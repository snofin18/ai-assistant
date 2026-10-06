//! Grayscale image buffer and deterministic box downsampling.
//!
//! Responsibility: own the only input shape the visual hashes accept — a validated, row-major,
//! 8-bit grayscale buffer — and reduce it to the small grids the hashes need.
//!
//! Boundary: no image codec, no color conversion, no file IO. The caller (platform screenshot
//! layer) is responsible for turning an [`assistant_platform_api::ImageRef`] into grayscale; this
//! module only validates the declared width / height / buffer triple and does integer arithmetic
//! on it. Keeping the codec out is what makes the whole module zero-dependency and replayable.
//!
//! Invariants:
//! 1. `width * height == pixels.len()`, both dimensions are non-zero, and the pixel count never
//!    exceeds [`MAX_IMAGE_PIXELS`]; anything else is an explicit [`VisualError`].
//! 2. Downsampling is a pure function of the input: box averages use integer arithmetic with a
//!    fixed floor rounding, so equal inputs give bit-identical outputs on every platform.
//! 3. No allocation grows with anything but the declared image size, and the declared image size
//!    is capped (ADR-0063's bounded-state discipline).

use crate::visual::error::VisualError;

/// Hard upper bound on the number of pixels the visual layer will accept.
///
/// The bound keeps every later accumulator inside `u32`: `16_777_216 * 255` is
/// `4_278_190_080`, still below `u32::MAX`, so the sum of absolute differences cannot overflow
/// and no `u64` to `f64` cast is needed.
pub const MAX_IMAGE_PIXELS: usize = 16_777_216;

/// One grayscale image: width, height, and a row-major 8-bit pixel buffer.
///
/// This is deliberately not an image-file abstraction. It is the smallest structure a perceptual
/// hash can be computed from, and it carries no codec or platform metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrayImage {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl GrayImage {
    /// Validates a declared width / height / pixel buffer triple.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::EmptyDimensions`] for a zero dimension,
    /// [`VisualError::EmptyBuffer`] for a non-empty image with an empty buffer,
    /// [`VisualError::BufferSizeMismatch`] when `pixels.len() != width * height`,
    /// [`VisualError::ImageTooLarge`] above [`MAX_IMAGE_PIXELS`],
    /// [`VisualError::DimensionsOverflow`] when `width * height` overflows `usize`, and
    /// [`VisualError::DimensionNotRepresentable`] when a `u32` dimension cannot become `usize`.
    /// All of these map to `ErrorCode::ToolInvalidArgs`; none of them is repaired.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, VisualError> {
        let width_usize = usize::try_from(width)
            .map_err(|_| VisualError::DimensionNotRepresentable { value: width })?;
        let height_usize = usize::try_from(height)
            .map_err(|_| VisualError::DimensionNotRepresentable { value: height })?;

        if width_usize == 0 || height_usize == 0 {
            return Err(VisualError::EmptyDimensions { width, height });
        }

        let expected = width_usize
            .checked_mul(height_usize)
            .ok_or(VisualError::DimensionsOverflow { width, height })?;
        if expected > MAX_IMAGE_PIXELS {
            return Err(VisualError::ImageTooLarge {
                width,
                height,
                max_pixels: MAX_IMAGE_PIXELS,
            });
        }
        if pixels.is_empty() {
            return Err(VisualError::EmptyBuffer { width, height });
        }
        if pixels.len() != expected {
            return Err(VisualError::BufferSizeMismatch {
                width,
                height,
                expected,
                actual: pixels.len(),
            });
        }

        Ok(Self {
            width: width_usize,
            height: height_usize,
            pixels,
        })
    }

    /// Image width in pixels.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Image height in pixels.
    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    /// The row-major 8-bit pixel buffer.
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Reads one pixel, with an explicit error instead of a panic when out of bounds.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::PixelOutOfBounds`] when `(x, y)` is outside the image, and
    /// [`VisualError::Inconsistent`] if the validated buffer cannot be addressed (a crate defect,
    /// reported as `ErrorCode::Fatal`).
    pub fn pixel_at(&self, x: usize, y: usize) -> Result<u8, VisualError> {
        let row = self.row(y)?;
        row.get(x).copied().ok_or(VisualError::PixelOutOfBounds {
            x,
            y,
            width: self.width,
            height: self.height,
        })
    }

    /// Returns one row of the buffer, or an internal inconsistency error.
    pub(crate) fn row(&self, row_index: usize) -> Result<&[u8], VisualError> {
        let start = row_index
            .checked_mul(self.width)
            .ok_or_else(|| VisualError::Inconsistent {
                operation: "read image row",
                reason: format!("row {row_index} * width {} overflowed", self.width),
            })?;
        let end = start
            .checked_add(self.width)
            .ok_or_else(|| VisualError::Inconsistent {
                operation: "read image row",
                reason: format!("row {row_index} end offset overflowed"),
            })?;
        self.pixels
            .get(start..end)
            .ok_or_else(|| VisualError::Inconsistent {
                operation: "read image row",
                reason: format!("row {row_index} range {start}..{end} is outside the buffer"),
            })
    }
}

/// Box-average downsampling to an exact `target_width x target_height` grid.
///
/// The algorithm is the standard integer box filter: each target cell covers a contiguous source
/// rectangle, sums its pixels, and floors the average. When the source is smaller than the target,
/// the rectangle collapses to the nearest single source pixel, which repeats it instead of
/// inventing a value. Example: a 4x4 source reduced to 2x2 averages each 2x2 quadrant.
///
/// # Errors
///
/// Returns [`VisualError::Inconsistent`] when a target dimension is zero, a row or cell cannot be
/// addressed, or an accumulator overflows. Those are crate-defect paths (`ErrorCode::Fatal`); the
/// caller's dimensions and buffer are validated by [`GrayImage::new`] first.
pub fn downsample(
    image: &GrayImage,
    target_width: usize,
    target_height: usize,
) -> Result<Vec<u8>, VisualError> {
    if target_width == 0 || target_height == 0 {
        return Err(VisualError::Inconsistent {
            operation: "downsample",
            reason: "target dimensions must be non-zero".to_owned(),
        });
    }

    let source_width = image.width();
    let source_height = image.height();
    let mut output = Vec::with_capacity(target_width * target_height);

    for target_y in 0..target_height {
        let y_start = target_y * source_height / target_height;
        let y_end = (((target_y + 1) * source_height) / target_height)
            .max(y_start + 1)
            .min(source_height);
        for target_x in 0..target_width {
            let x_start = target_x * source_width / target_width;
            let x_end = (((target_x + 1) * source_width) / target_width)
                .max(x_start + 1)
                .min(source_width);

            let mut sum: u32 = 0;
            let mut count: u32 = 0;
            for source_y in y_start..y_end {
                let row = image.row(source_y)?;
                let block = row
                    .get(x_start..x_end)
                    .ok_or_else(|| VisualError::Inconsistent {
                        operation: "downsample",
                        reason: format!("source row {source_y} is shorter than {x_end}"),
                    })?;
                for &value in block {
                    sum = sum.checked_add(u32::from(value)).ok_or_else(|| {
                        VisualError::Inconsistent {
                            operation: "downsample",
                            reason: "source cell sum overflowed u32".to_owned(),
                        }
                    })?;
                    count += 1;
                }
            }
            if count == 0 {
                return Err(VisualError::Inconsistent {
                    operation: "downsample",
                    reason: format!("target cell ({target_x}, {target_y}) covered no source pixel"),
                });
            }
            let average = sum / count;
            let average_byte = u8::try_from(average).map_err(|_| VisualError::Inconsistent {
                operation: "downsample",
                reason: format!("average {average} does not fit in one byte"),
            })?;
            output.push(average_byte);
        }
    }

    Ok(output)
}
