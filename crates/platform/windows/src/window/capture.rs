//! Windows 单窗口截图与像素脱敏（ADR-0074）。
//!
//! 职责：把一个已解析 `ResolvedWindow` 的 HWND 渲染到有界 BGRA 缓冲；按
//! `CaptureOptions.redact` 查询 UIA 密码框并做不透明黑色遮挡；把 Win32 / GDI 失败映射成
//! 既有 `ErrorCode`。
//! 边界：**不做**全屏截图、**不做**桌面 DC、**不做**图像编解码、**不做**视觉验证
//! （TASK-042）、**不做** blob 持久化装配（storage sink 未接入时保持 WIP）。
//!
//! ## 不变量
//! 1. 主通道是 `PrintWindow(PW_RENDERFULLCONTENT)`；`BitBlt` 只在该调用失败且窗口**未被遮挡**
//!    时回退。遮挡场景拒绝 BitBlt，避免截到其它窗口。
//! 2. 缓冲尺寸来自 `GetWindowRect`，必须先做正数、`i32` 无损和总像素硬上限校验。
//! 3. `redact=true` 时任一步 UIA 查询失败都 fail-closed；不得返回未遮挡图像。
//! 4. GDI DC / bitmap / 选中对象在成功、错误和提前返回路径都释放；清理失败不伪造成功。
//!
//! 相关：ADR-0074、ADR-0073、`crates/platform/api/src/traits/window.rs`。

use std::ffi::c_void;
use std::ptr::null_mut;

use assistant_platform_api::{
    CaptureOptions, ErrorCode, ImageBlobSink, ImageRef, PlatformError, PlatformResult,
    ResolvedWindow,
};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleDC, CreateDIBSection,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetWindowDC, HBITMAP, HDC, HGDIOBJ, ReleaseDC, SRCCOPY,
    SelectObject,
};
use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{TreeScope_Descendants, UIA_IsPasswordPropertyId};
use windows::Win32::UI::WindowsAndMessaging::PW_RENDERFULLCONTENT;

use crate::{com, digest, error, handles, uia, win32};

/// 单次截图的像素硬上限（7680×4320；覆盖当前 8K 工作面上限）。
const MAX_CAPTURE_PIXELS: u64 = 7_680 * 4_320;
/// 单窗口密码框遮挡矩形的硬上限。
const MAX_REDACTION_RECTANGLES: usize = 64;
/// BGRA 每个像素的字节数。
const BYTES_PER_PIXEL: usize = 4;
/// 遮挡后的不透明黑色（BGRA）。
const BLACK_BGRA: [u8; 4] = [0, 0, 0, 255];

/// 截图前置状态失败（纯值，便于单测映射）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureFailure {
    /// 句柄无效 / 指针宽度不匹配。
    InvalidHandle,
    /// 窗口最小化。
    Minimized,
    /// 窗口被遮挡且 `BitBlt` 不安全。
    OccludedFallbackUnavailable,
    /// 窗口矩形为零 / 负数。
    ZeroSized,
    /// 宽高超出 i32 / 像素上限。
    TooLarge,
}

/// 已验证的截图尺寸。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CaptureDimensions {
    width: u32,
    height: u32,
}

impl CaptureDimensions {
    /// 该尺寸对应的 BGRA 字节数。
    fn byte_len(self) -> PlatformResult<usize> {
        let pixels = u64::from(self.width)
            .checked_mul(u64::from(self.height))
            .ok_or_else(|| capture_failure_error(CaptureFailure::TooLarge))?;
        let bytes = pixels
            .checked_mul(BYTES_PER_PIXEL as u64)
            .ok_or_else(|| capture_failure_error(CaptureFailure::TooLarge))?;
        usize::try_from(bytes).map_err(|_| capture_failure_error(CaptureFailure::TooLarge))
    }

    /// 每行 BGRA 字节数。
    fn row_bytes(self) -> PlatformResult<usize> {
        usize::try_from(self.width)
            .ok()
            .and_then(|width| width.checked_mul(BYTES_PER_PIXEL))
            .ok_or_else(|| capture_failure_error(CaptureFailure::TooLarge))
    }
}

/// 已裁剪到图像坐标系的遮挡矩形。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PixelRectangle {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

/// GDI 资源组：确保 DC / bitmap / 选中对象在每条返回路径后释放。
struct GdiCaptureResources {
    device_context: HDC,
    bitmap: HBITMAP,
    previous_object: Option<HGDIOBJ>,
}

impl GdiCaptureResources {
    const fn new(device_context: HDC, bitmap: HBITMAP) -> Self {
        Self {
            device_context,
            bitmap,
            previous_object: None,
        }
    }

    const fn record_selected(&mut self, previous_object: HGDIOBJ) {
        self.previous_object = Some(previous_object);
    }

    /// 恢复选中对象并释放 bitmap / DC；返回第一个清理失败。
    fn release(&mut self) -> PlatformResult<()> {
        let mut first_failure: Option<PlatformError> = None;
        if let Some(previous_object) = self.previous_object.take() {
            // SAFETY: `device_context` 与 `previous_object` 都来自本组持有的 GDI 资源。
            let restored = unsafe { SelectObject(self.device_context, previous_object) };
            if restored.0.is_null() && first_failure.is_none() {
                first_failure = Some(capture_failure_error(CaptureFailure::TooLarge));
            }
        }
        if !self.bitmap.0.is_null() {
            let bitmap = self.bitmap;
            self.bitmap = HBITMAP::default();
            // SAFETY: `bitmap` 是 CreateDIBSection 返回且尚未删除的 GDI 对象。
            let deleted = unsafe { DeleteObject(bitmap.into()) };
            if !deleted.as_bool() && first_failure.is_none() {
                first_failure = Some(error_from_thread("DeleteObject"));
            }
        }
        if !self.device_context.0.is_null() {
            let device_context = self.device_context;
            self.device_context = HDC::default();
            // SAFETY: `device_context` 是 CreateCompatibleDC 返回且尚未删除的 DC。
            let deleted = unsafe { DeleteDC(device_context) };
            if !deleted.as_bool() && first_failure.is_none() {
                first_failure = Some(error_from_thread("DeleteDC"));
            }
        }
        first_failure.map_or(Ok(()), Err)
    }
}

impl Drop for GdiCaptureResources {
    fn drop(&mut self) {
        // Drop 不能返回错误；显式绑定并保留原因，正常路径已经在返回前调用 release()。
        let _cleanup_outcome = self.release();
    }
}

/// 截取一个已解析窗口（ADR-0074 的平台入口）。
pub(super) fn capture_window(
    window: &ResolvedWindow,
    options: CaptureOptions,
    blob_sink: Option<&dyn ImageBlobSink>,
) -> PlatformResult<ImageRef> {
    let hwnd = handles::hwnd_from_window_handle(window.id())
        .map_err(|_| capture_failure_error(CaptureFailure::InvalidHandle))?;
    if !win32::is_window(hwnd) {
        return Err(capture_failure_error(CaptureFailure::InvalidHandle));
    }
    if win32::is_minimized(hwnd) {
        return Err(capture_failure_error(CaptureFailure::Minimized));
    }
    let window_rect =
        win32::window_rect(hwnd).ok_or_else(|| capture_failure_error(CaptureFailure::ZeroSized))?;
    let dimensions = dimensions_from_rect(window_rect)?;
    let occlusions = if options.redact() {
        collect_password_rectangles(hwnd, window_rect, dimensions)?
    } else {
        Vec::new()
    };
    let mut pixels = capture_bgra(hwnd, dimensions, win32::is_occluded(hwnd))?;
    apply_pixel_occlusions(&mut pixels, dimensions, &occlusions)?;
    // 内容地址由平台层算（ADR-0076）：先算出地址，再把它与像素一起交给注入的写入端。
    let content_address = digest::sha256_hex_bytes(&pixels);
    let Some(blob_sink) = blob_sink else {
        // 未注入写入端 = 装配缺口。显式失败，绝不返回一个谎称已持久化的 `ImageRef`（铁律 1）。
        return Err(PlatformError::new(
            ErrorCode::Fatal,
            "capture: no ImageBlobSink was injected, so the frame cannot be persisted; \
             assemble the platform with `WindowsPlatform::with_blob_sink`",
        ));
    };
    // 写入端的错误（IO / 元数据不一致）原样透传，不降级成空图。
    blob_sink.store_bgra(dimensions.width, dimensions.height, &content_address, &pixels)?;
    Ok(ImageRef::new(
        content_address,
        dimensions.width,
        dimensions.height,
    ))
}

/// 把状态失败映射成既有 `ErrorCode`（ADR-0074 D3）。
fn capture_failure_error(failure: CaptureFailure) -> PlatformError {
    match failure {
        CaptureFailure::InvalidHandle => {
            error::target_not_found("capture: the resolved window handle is no longer a valid HWND")
        }
        CaptureFailure::Minimized => error::target_unresponsive(
            "capture: the target window is minimized; no reliable window surface is available",
        ),
        CaptureFailure::OccludedFallbackUnavailable => error::target_unresponsive(
            "capture: PrintWindow failed while the window is occluded; BitBlt fallback is refused \
             because it could capture pixels from other windows",
        ),
        CaptureFailure::ZeroSized => error::target_unresponsive(
            "capture: the target window has a zero or negative rectangle",
        ),
        CaptureFailure::TooLarge => PlatformError::new(
            ErrorCode::Fatal,
            "capture: window dimensions overflow or exceed MAX_CAPTURE_PIXELS",
        ),
    }
}

/// 从窗口矩形推导有界尺寸。
fn dimensions_from_rect(rect: RECT) -> PlatformResult<CaptureDimensions> {
    let width = i64::from(rect.right) - i64::from(rect.left);
    let height = i64::from(rect.bottom) - i64::from(rect.top);
    validate_capture_dimensions(width, height)
}

/// 校验宽高并转换为 `u32`。
fn validate_capture_dimensions(width: i64, height: i64) -> PlatformResult<CaptureDimensions> {
    if width <= 0 || height <= 0 {
        return Err(capture_failure_error(CaptureFailure::ZeroSized));
    }
    let width =
        u32::try_from(width).map_err(|_| capture_failure_error(CaptureFailure::TooLarge))?;
    let height =
        u32::try_from(height).map_err(|_| capture_failure_error(CaptureFailure::TooLarge))?;
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| capture_failure_error(CaptureFailure::TooLarge))?;
    if pixels > MAX_CAPTURE_PIXELS {
        return Err(capture_failure_error(CaptureFailure::TooLarge));
    }
    Ok(CaptureDimensions { width, height })
}

/// 查询 UIA 已判定为密码框的元素，并转换为图像局部矩形。
fn collect_password_rectangles(
    hwnd: HWND,
    window_rect: RECT,
    dimensions: CaptureDimensions,
) -> PlatformResult<Vec<PixelRectangle>> {
    let root = uia::element_for_window(hwnd)?;
    com::with_automation(|automation| {
        let value = VARIANT::from(true);
        // SAFETY: `value` 在调用期间存活，UIA 会复制它。
        let condition =
            unsafe { automation.CreatePropertyCondition(UIA_IsPasswordPropertyId, &value) }
                .map_err(|failure| {
                    error::error_from_hresult(
                        failure.code().0,
                        "CreatePropertyCondition(IsPassword)",
                    )
                })?;
        // SAFETY: `condition` 在调用期间存活；返回数组由本进程持有。
        let array =
            unsafe { root.FindAll(TreeScope_Descendants, &condition) }.map_err(|failure| {
                error::error_from_hresult(failure.code().0, "FindAll(IsPassword)")
            })?;
        // SAFETY: 只读属性。
        let length = unsafe { array.Length() }.map_err(|failure| {
            error::error_from_hresult(failure.code().0, "ElementArray::Length")
        })?;
        let count =
            usize::try_from(length).map_err(|_| capture_failure_error(CaptureFailure::TooLarge))?;
        if count > MAX_REDACTION_RECTANGLES {
            return Err(capture_failure_error(CaptureFailure::TooLarge));
        }
        let mut rectangles = Vec::with_capacity(count);
        for index in 0..length {
            // SAFETY: `index` 由 Length() 界定在合法区间内。
            let element = unsafe { array.GetElement(index) }.map_err(|failure| {
                error::error_from_hresult(failure.code().0, "ElementArray::GetElement")
            })?;
            // SAFETY: 只读属性查询。
            let bounding_rectangle =
                unsafe { element.CurrentBoundingRectangle() }.map_err(|failure| {
                    error::error_from_hresult(
                        failure.code().0,
                        "CurrentBoundingRectangle(IsPassword)",
                    )
                })?;
            if let Some(rectangle) = screen_rect_to_pixel_rectangle(
                bounding_rectangle,
                window_rect.left,
                window_rect.top,
                dimensions,
            )? {
                rectangles.push(rectangle);
            }
        }
        Ok(rectangles)
    })
}

/// 把屏幕坐标的 UIA 矩形转换为窗口图像局部坐标；完全不可见时返回 `None`。
fn screen_rect_to_pixel_rectangle(
    rect: RECT,
    origin_x: i32,
    origin_y: i32,
    dimensions: CaptureDimensions,
) -> PlatformResult<Option<PixelRectangle>> {
    let left = i64::from(rect.left) - i64::from(origin_x);
    let top = i64::from(rect.top) - i64::from(origin_y);
    let right = i64::from(rect.right) - i64::from(origin_x);
    let bottom = i64::from(rect.bottom) - i64::from(origin_y);
    if right <= left || bottom <= top {
        return Ok(None);
    }
    let image_right = i64::from(dimensions.width);
    let image_bottom = i64::from(dimensions.height);
    let clipped_left = left.clamp(0, image_right);
    let clipped_top = top.clamp(0, image_bottom);
    let clipped_right = right.clamp(0, image_right);
    let clipped_bottom = bottom.clamp(0, image_bottom);
    if clipped_right <= clipped_left || clipped_bottom <= clipped_top {
        return Ok(None);
    }
    Ok(Some(PixelRectangle {
        left: u32::try_from(clipped_left)
            .map_err(|_| capture_failure_error(CaptureFailure::TooLarge))?,
        top: u32::try_from(clipped_top)
            .map_err(|_| capture_failure_error(CaptureFailure::TooLarge))?,
        right: u32::try_from(clipped_right)
            .map_err(|_| capture_failure_error(CaptureFailure::TooLarge))?,
        bottom: u32::try_from(clipped_bottom)
            .map_err(|_| capture_failure_error(CaptureFailure::TooLarge))?,
    }))
}

/// 在 BGRA 缓冲上写不透明黑色。
fn apply_pixel_occlusions(
    pixels: &mut [u8],
    dimensions: CaptureDimensions,
    rectangles: &[PixelRectangle],
) -> PlatformResult<()> {
    let expected_bytes = dimensions.byte_len()?;
    if pixels.len() != expected_bytes {
        return Err(error::verify_failed(format!(
            "capture: BGRA buffer length {} does not match dimensions {}x{} ({} bytes)",
            pixels.len(),
            dimensions.width,
            dimensions.height,
            expected_bytes
        )));
    }
    let row_bytes = dimensions.row_bytes()?;
    for rectangle in rectangles {
        mask_rectangle(pixels, row_bytes, *rectangle)?;
    }
    Ok(())
}

/// 在 `row_bytes` 行跨度的 BGRA 缓冲里填充一个已裁剪矩形。
fn mask_rectangle(
    pixels: &mut [u8],
    row_bytes: usize,
    rectangle: PixelRectangle,
) -> PlatformResult<()> {
    for row_index in rectangle.top..rectangle.bottom {
        let row_start = usize::try_from(row_index)
            .ok()
            .and_then(|row| row.checked_mul(row_bytes))
            .ok_or_else(|| capture_failure_error(CaptureFailure::TooLarge))?;
        let row_end = row_start
            .checked_add(row_bytes)
            .ok_or_else(|| capture_failure_error(CaptureFailure::TooLarge))?;
        let row = pixels
            .get_mut(row_start..row_end)
            .ok_or_else(|| capture_failure_error(CaptureFailure::TooLarge))?;
        let left = usize::try_from(rectangle.left)
            .ok()
            .and_then(|value| value.checked_mul(BYTES_PER_PIXEL))
            .ok_or_else(|| capture_failure_error(CaptureFailure::TooLarge))?;
        let right = usize::try_from(rectangle.right)
            .ok()
            .and_then(|value| value.checked_mul(BYTES_PER_PIXEL))
            .ok_or_else(|| capture_failure_error(CaptureFailure::TooLarge))?;
        let span = row
            .get_mut(left..right)
            .ok_or_else(|| capture_failure_error(CaptureFailure::TooLarge))?;
        for pixel in span.as_chunks_mut::<BYTES_PER_PIXEL>().0 {
            pixel.copy_from_slice(&BLACK_BGRA);
        }
    }
    Ok(())
}

/// 捕获窗口 BGRA；`PrintWindow` 成功后不碰 `BitBlt`。
fn capture_bgra(
    hwnd: HWND,
    dimensions: CaptureDimensions,
    occluded: bool,
) -> PlatformResult<Vec<u8>> {
    let (device_context, bitmap, bits) = create_capture_target(dimensions)?;
    let mut resources = GdiCaptureResources::new(device_context, bitmap);

    // SAFETY: `bitmap` 是本组持有且尚未删除的 GDI 对象。
    let previous_object = unsafe { SelectObject(device_context, bitmap.into()) };
    if previous_object.0.is_null() {
        return Err(error_from_thread("SelectObject"));
    }
    resources.record_selected(previous_object);

    render_window_into(hwnd, device_context, dimensions, occluded)?;

    if bits.is_null() {
        return Err(error_from_thread("CreateDIBSection(bits)"));
    }
    let byte_len = dimensions.byte_len()?;
    // SAFETY: CreateDIBSection 保证 `bits` 指向 byte_len 字节的可读写 DIB 区域；
    // 本次 to_vec 在资源删除前完成。
    let pixels = unsafe { std::slice::from_raw_parts(bits.cast::<u8>(), byte_len) }.to_vec();
    resources.release()?;
    Ok(pixels)
}

/// 创建一个匹配窗口尺寸的 top-down 32-bit DIB 与内存 DC。
fn create_capture_target(
    dimensions: CaptureDimensions,
) -> PlatformResult<(HDC, HBITMAP, *mut c_void)> {
    let width = i32::try_from(dimensions.width)
        .map_err(|_| capture_failure_error(CaptureFailure::TooLarge))?;
    let height = i32::try_from(dimensions.height)
        .map_err(|_| capture_failure_error(CaptureFailure::TooLarge))?;
    let header_size = u32::try_from(std::mem::size_of::<BITMAPINFOHEADER>())
        .map_err(|_| capture_failure_error(CaptureFailure::TooLarge))?;
    let image_bytes = u32::try_from(dimensions.byte_len()?)
        .map_err(|_| capture_failure_error(CaptureFailure::TooLarge))?;
    let mut bitmap_info = BITMAPINFO::default();
    bitmap_info.bmiHeader.biSize = header_size;
    bitmap_info.bmiHeader.biWidth = width;
    // 负高度 = top-down DIB，行序与 Rust 缓冲一致。
    bitmap_info.bmiHeader.biHeight = -height;
    bitmap_info.bmiHeader.biPlanes = 1;
    bitmap_info.bmiHeader.biBitCount = 32;
    bitmap_info.bmiHeader.biCompression = BI_RGB.0;
    bitmap_info.bmiHeader.biSizeImage = image_bytes;

    // SAFETY: 创建与屏幕兼容的内存 DC；失败返回空句柄。
    let device_context = unsafe { CreateCompatibleDC(None) };
    if device_context.0.is_null() {
        return Err(error_from_thread("CreateCompatibleDC"));
    }
    let mut bits: *mut c_void = null_mut();
    // SAFETY: `bitmap_info` 是合法 BITMAPINFO；`bits` 是本栈帧 out-pointer。
    let bitmap = match unsafe {
        CreateDIBSection(
            None,
            &raw const bitmap_info,
            DIB_RGB_COLORS,
            &raw mut bits,
            None,
            0,
        )
    } {
        Ok(bitmap) => bitmap,
        Err(failure) => {
            // SAFETY: `device_context` 是本函数刚创建且尚未删除的 DC。
            let _cleanup_outcome = unsafe { DeleteDC(device_context) };
            return Err(error::error_from_win32_failure(
                &failure,
                "CreateDIBSection",
            ));
        }
    };
    Ok((device_context, bitmap, bits))
}

/// 优先 `PrintWindow`，只在窗口未遮挡时允许 `BitBlt` 回退。
fn render_window_into(
    hwnd: HWND,
    device_context: HDC,
    dimensions: CaptureDimensions,
    occluded: bool,
) -> PlatformResult<()> {
    // SAFETY: HWND 已通过 IsWindow；memory DC 已选中匹配尺寸的 DIB。
    let printed = unsafe {
        PrintWindow(
            hwnd,
            device_context,
            PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT),
        )
    };
    if !printed.as_bool() {
        let print_failure = windows::core::Error::from_thread();
        let print_error = error::error_from_win32_failure(&print_failure, "PrintWindow");
        if occluded {
            return Err(capture_failure_error(
                CaptureFailure::OccludedFallbackUnavailable,
            ));
        }
        // SAFETY: 内存 DC、尺寸和 HWND 都已校验；BitBlt 只在窗口未被遮挡时回退。
        bit_blt_window(hwnd, device_context, dimensions).map_err(|bitblt_error| {
            PlatformError::new(
                bitblt_error.code(),
                format!(
                    "capture: PrintWindow failed ({}) and BitBlt fallback failed ({})",
                    print_error.message(),
                    bitblt_error.message()
                ),
            )
        })?;
    }
    Ok(())
}

/// 从窗口 DC 做受限 `BitBlt` 回退。
fn bit_blt_window(
    hwnd: HWND,
    destination: HDC,
    dimensions: CaptureDimensions,
) -> PlatformResult<()> {
    let width = i32::try_from(dimensions.width)
        .map_err(|_| capture_failure_error(CaptureFailure::TooLarge))?;
    let height = i32::try_from(dimensions.height)
        .map_err(|_| capture_failure_error(CaptureFailure::TooLarge))?;
    // SAFETY: 只读获取目标窗口 DC；调用方保证窗口未被遮挡。
    let source = unsafe { GetWindowDC(Some(hwnd)) };
    if source.0.is_null() {
        return Err(error_from_thread("GetWindowDC"));
    }
    // SAFETY: source / destination 都是本函数生命周期内的合法 DC，尺寸来自窗口矩形。
    let copied = unsafe {
        BitBlt(
            destination,
            0,
            0,
            width,
            height,
            Some(source),
            0,
            0,
            SRCCOPY,
        )
    }
    .map_err(|failure| error::error_from_win32_failure(&failure, "BitBlt"));
    // SAFETY: 与 GetWindowDC 严格配对释放；释放前不丢弃结果。
    let released = unsafe { ReleaseDC(Some(hwnd), source) };
    copied?;
    if released == 0 {
        return Err(error_from_thread("ReleaseDC"));
    }
    Ok(())
}

/// 把 `GetLastError` 形态转成平台错误。
fn error_from_thread(context: &str) -> PlatformError {
    error::error_from_win32_failure(&windows::core::Error::from_thread(), context)
}

#[cfg(test)]
mod tests;
