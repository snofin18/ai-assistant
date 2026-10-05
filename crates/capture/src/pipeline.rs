//! Platform-agnostic screenshot orchestration.
//!
//! The pipeline validates redaction decisions before capture, delegates the
//! only platform primitive to the injected `WindowProvider`, and applies the
//! selected privacy retention mode to the returned `ImageRef`.

use assistant_dlp::{ImageDimensions, OcclusionRectangle, RedactionRule, resolve_occlusions};
use assistant_platform_api::{CaptureOptions, ImageRef, ResolvedWindow, WindowProvider};

use crate::error::CaptureError;
use crate::privacy::PrivacyMode;

/// Redaction policy for one capture call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RedactionPolicy<'rules> {
    /// Capture without applying redaction decisions.
    Disabled,
    /// Require at least one already classified rule and compute occlusions.
    Required(&'rules [RedactionRule]),
}

/// Capture result without pixels or platform handles.
#[derive(Debug)]
pub struct CaptureOutcome {
    retained_image: Option<ImageRef>,
    image_dimensions: ImageDimensions,
    occlusions: Vec<OcclusionRectangle>,
}

impl CaptureOutcome {
    /// Blob reference retained only when [`PrivacyMode::PersistBlob`] is active.
    #[must_use]
    pub const fn retained_image(&self) -> Option<&ImageRef> {
        self.retained_image.as_ref()
    }

    /// Platform-reported image dimensions.
    #[must_use]
    pub const fn image_dimensions(&self) -> ImageDimensions {
        self.image_dimensions
    }

    /// Validated redaction rectangles; empty when redaction is disabled.
    #[must_use]
    pub fn occlusions(&self) -> &[OcclusionRectangle] {
        &self.occlusions
    }
}

/// Platform-agnostic capture pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapturePipeline {
    privacy_mode: PrivacyMode,
}

impl CapturePipeline {
    /// Construct a pipeline with one privacy retention mode.
    #[must_use]
    pub const fn new(privacy_mode: PrivacyMode) -> Self {
        Self { privacy_mode }
    }

    /// Configured privacy mode.
    #[must_use]
    pub const fn privacy_mode(self) -> PrivacyMode {
        self.privacy_mode
    }

    /// Capture one window through the injected provider.
    ///
    /// # Errors
    /// Redaction rules are validated before the provider call. Provider errors
    /// preserve their original `ErrorCode`; invalid provider dimensions are
    /// mapped to a redaction validation error.
    pub async fn capture<P: WindowProvider>(
        self,
        provider: &P,
        window: &ResolvedWindow,
        bounds: ImageDimensions,
        redaction: RedactionPolicy<'_>,
    ) -> Result<CaptureOutcome, CaptureError> {
        let (redact_pixels, occlusions) = match redaction {
            RedactionPolicy::Disabled => (false, Vec::new()),
            RedactionPolicy::Required(rules) => (
                true,
                resolve_occlusions(bounds, rules).map_err(CaptureError::Redaction)?,
            ),
        };

        let image_ref = provider
            .capture(window, &CaptureOptions::new(redact_pixels))
            .await
            .map_err(CaptureError::Platform)?;
        let image_dimensions = ImageDimensions::new(image_ref.width(), image_ref.height())
            .map_err(CaptureError::Redaction)?;
        let retained_image = self.privacy_mode.retains_image_ref().then_some(image_ref);

        Ok(CaptureOutcome {
            retained_image,
            image_dimensions,
            occlusions,
        })
    }
}
