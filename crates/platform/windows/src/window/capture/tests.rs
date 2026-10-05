use super::*;

fn valid_dimensions(width: i64, height: i64) -> CaptureDimensions {
    match validate_capture_dimensions(width, height) {
        Ok(dimensions) => dimensions,
        Err(error) => unreachable!("test fixture dimensions must be valid: {error}"),
    }
}

fn dimensions_error(width: i64, height: i64) -> PlatformError {
    match validate_capture_dimensions(width, height) {
        Ok(dimensions) => unreachable!("dimensions unexpectedly valid: {dimensions:?}"),
        Err(error) => error,
    }
}

fn clipped_rectangle(
    rect: RECT,
    origin_x: i32,
    origin_y: i32,
    dimensions: CaptureDimensions,
) -> Option<PixelRectangle> {
    match screen_rect_to_pixel_rectangle(rect, origin_x, origin_y, dimensions) {
        Ok(rectangle) => rectangle,
        Err(error) => unreachable!("test fixture rectangle must be valid: {error}"),
    }
}

fn assert_occlusions_succeed(
    pixels: &mut [u8],
    dimensions: CaptureDimensions,
    rectangles: &[PixelRectangle],
) {
    if let Err(error) = apply_pixel_occlusions(pixels, dimensions, rectangles) {
        unreachable!("test fixture masking must succeed: {error}");
    }
}

fn rectangle(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
    RECT {
        left,
        top,
        right,
        bottom,
    }
}

#[test]
fn test_dimensions_reject_zero_and_negative() {
    for (width, height) in [(0, 1), (1, 0), (-1, 1), (1, -1)] {
        let error = dimensions_error(width, height);
        assert_eq!(error.code(), ErrorCode::TargetUnresponsive);
    }
}

#[test]
fn test_dimensions_reject_over_limit() {
    let error = dimensions_error(i64::from(u32::MAX), i64::from(u32::MAX));
    assert_eq!(error.code(), ErrorCode::Fatal);
}

#[test]
fn test_screen_rectangle_is_clipped_to_image_space() {
    let dimensions = valid_dimensions(4, 4);
    let clipped = clipped_rectangle(rectangle(-2, -1, 2, 3), -2, -1, dimensions);
    assert_eq!(
        clipped,
        Some(PixelRectangle {
            left: 0,
            top: 0,
            right: 4,
            bottom: 4
        })
    );
}

#[test]
fn test_fully_offscreen_rectangle_is_ignored() {
    let dimensions = valid_dimensions(4, 4);
    let clipped = clipped_rectangle(rectangle(-10, -10, -1, -1), 0, 0, dimensions);
    assert_eq!(clipped, None);
}

#[test]
fn test_apply_occlusions_masks_only_requested_pixels() {
    let dimensions = valid_dimensions(3, 2);
    let byte_len = match dimensions.byte_len() {
        Ok(byte_len) => byte_len,
        Err(error) => unreachable!("test fixture byte length must be valid: {error}"),
    };
    let mut pixels = vec![9_u8; byte_len];
    assert_occlusions_succeed(
        &mut pixels,
        dimensions,
        &[PixelRectangle {
            left: 1,
            top: 0,
            right: 2,
            bottom: 1,
        }],
    );
    assert_eq!(pixels.get(0..4), Some(&[9, 9, 9, 9][..]));
    assert_eq!(pixels.get(4..8), Some(&BLACK_BGRA[..]));
    assert_eq!(pixels.get(8..12), Some(&[9, 9, 9, 9][..]));
}

#[test]
fn test_apply_occlusions_rejects_wrong_buffer_size() {
    let dimensions = valid_dimensions(3, 2);
    let mut pixels = vec![0_u8; 3];
    let error = match apply_pixel_occlusions(&mut pixels, dimensions, &[]) {
        Ok(()) => unreachable!("wrong buffer length must fail"),
        Err(error) => error,
    };
    assert_eq!(error.code(), ErrorCode::VerifyFailed);
}

#[test]
fn test_capture_failure_codes_are_stable() {
    assert_eq!(
        capture_failure_error(CaptureFailure::InvalidHandle).code(),
        ErrorCode::TargetNotFound
    );
    assert_eq!(
        capture_failure_error(CaptureFailure::Minimized).code(),
        ErrorCode::TargetUnresponsive
    );
    assert_eq!(
        capture_failure_error(CaptureFailure::OccludedFallbackUnavailable).code(),
        ErrorCode::TargetUnresponsive
    );
    assert_eq!(
        capture_failure_error(CaptureFailure::TooLarge).code(),
        ErrorCode::Fatal
    );
}
