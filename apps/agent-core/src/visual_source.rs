//! Host-side `visual_assert` evidence source.
//!
//! Responsibility: read the reference and observed screenshot blob descriptors
//! from a successful tool envelope, load the stored BGRA bytes through the
//! assembly-owned database handle, convert them to validated grayscale images,
//! and hand the result to the runtime's additive visual-observation hook.
//!
//! Boundary: this module does not capture windows, choose regions, decode image
//! files, or evaluate assertions. It also never puts pixels or hashes into the
//! tool envelope, IPC, or `Observation`.
//!
//! Invariants:
//! 1. A missing `visual_observation` field means "no visual evidence"; a
//!    partial or malformed field is an explicit error, never `None`.
//! 2. Every blob descriptor is validated before any storage read.
//! 3. Stored BGRA bytes must be exactly `width * height * 4`; the storage
//!    read path already verifies the content address.
//! 4. The grayscale result is bounded by the verify crate's image limit.
//!
//! Related: ADR-0074, ADR-0076, ADR-0077, ADR-0079,
//! `apps/agent-core/src/runtime.rs`, `crates/verify/src/visual/image.rs`.

use assistant_protocol::ToolEnvelope;
use assistant_protocol::serde_json::{Map, Value as JsonValue};
use assistant_storage::BlobId;
use assistant_verify::{GrayImage, VisualError, VisualObservation};

use crate::adapters::DatabaseHandle;
use crate::runtime::{EnvelopeObservationCollector, ObservationCollector, RuntimeExecutionError};

/// Top-level envelope field carrying the two screenshot descriptors.
pub const VISUAL_OBSERVATION_FIELD: &str = "visual_observation";

/// Host-side collector that adds stored visual evidence to the ordinary
/// envelope observation.
#[derive(Debug, Clone)]
pub struct StorageVisualObservationCollector {
    database: DatabaseHandle,
}

impl StorageVisualObservationCollector {
    /// Creates a collector over the assembly-owned database.
    #[must_use]
    pub const fn new(database: DatabaseHandle) -> Self {
        Self { database }
    }
}

impl ObservationCollector for StorageVisualObservationCollector {
    fn observe(
        &self,
        step: &assistant_task_engine::PlanStep,
        envelope: &ToolEnvelope,
    ) -> Result<assistant_verify::Observation, RuntimeExecutionError> {
        EnvelopeObservationCollector.observe(step, envelope)
    }

    fn observe_visual(
        &self,
        _step: &assistant_task_engine::PlanStep,
        envelope: &ToolEnvelope,
    ) -> Result<Option<VisualObservation>, RuntimeExecutionError> {
        load_visual_observation(&self.database, envelope).map_err(|reason| {
            RuntimeExecutionError::Observation {
                reason: format!("visual observation source failed: {reason}"),
            }
        })
    }
}

/// One content-addressed BGRA image descriptor from the tool envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ImageDescriptor {
    blob_id: BlobId,
    width: u32,
    height: u32,
}

/// Loads and validates the optional visual observation carried by an envelope.
fn load_visual_observation(
    database: &DatabaseHandle,
    envelope: &ToolEnvelope,
) -> Result<Option<VisualObservation>, String> {
    let Some(data) = envelope.data.as_ref() else {
        return Ok(None);
    };
    let Some(value) = data.0.get(VISUAL_OBSERVATION_FIELD) else {
        return Ok(None);
    };
    let object = value
        .as_object()
        .ok_or_else(|| format!("`{VISUAL_OBSERVATION_FIELD}` must be an object"))?;
    ensure_allowed_keys(
        object,
        &["reference", "observed", "confidence"],
        VISUAL_OBSERVATION_FIELD,
    )?;
    let reference = descriptor_from_object(object, "reference")?;
    let observed = descriptor_from_object(object, "observed")?;
    let confidence = object
        .get("confidence")
        .and_then(JsonValue::as_f64)
        .ok_or_else(|| format!("`{VISUAL_OBSERVATION_FIELD}.confidence` must be a JSON number"))?;

    // Read the two images before conversion so a broken reference cannot leave
    // a partially built observation behind.
    let reference_bgra = read_bgra(database, &reference)?;
    let observed_bgra = read_bgra(database, &observed)?;
    let expected = bgra_to_gray(reference.width, reference.height, &reference_bgra)?;
    let actual = bgra_to_gray(observed.width, observed.height, &observed_bgra)?;
    VisualObservation::new(expected, actual, confidence)
        .map(Some)
        .map_err(|error| format!("visual observation is invalid: {error}"))
}

/// Parses one nested image descriptor from the envelope object.
fn descriptor_from_object(
    object: &Map<String, JsonValue>,
    field: &str,
) -> Result<ImageDescriptor, String> {
    let value = object
        .get(field)
        .ok_or_else(|| format!("`{VISUAL_OBSERVATION_FIELD}.{field}` is missing"))?;
    let descriptor = value
        .as_object()
        .ok_or_else(|| format!("`{VISUAL_OBSERVATION_FIELD}.{field}` must be an object"))?;
    ensure_allowed_keys(
        descriptor,
        &["blob_id", "width", "height"],
        &format!("{VISUAL_OBSERVATION_FIELD}.{field}"),
    )?;
    let blob_text = descriptor
        .get("blob_id")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| format!("`{VISUAL_OBSERVATION_FIELD}.{field}.blob_id` must be a string"))?;
    let blob_id = BlobId::parse(blob_text).map_err(|error| {
        format!("`{VISUAL_OBSERVATION_FIELD}.{field}.blob_id` is invalid: {error}")
    })?;
    let width = positive_u32(descriptor, field, "width")?;
    let height = positive_u32(descriptor, field, "height")?;
    Ok(ImageDescriptor {
        blob_id,
        width,
        height,
    })
}

/// Rejects unknown keys so an envelope cannot smuggle extra image or policy
/// fields past a reader that only understands the fixed ADR-0079 shape.
fn ensure_allowed_keys(
    object: &Map<String, JsonValue>,
    allowed: &[&str],
    context: &str,
) -> Result<(), String> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("`{context}` contains unknown field `{key}`"));
        }
    }
    Ok(())
}

/// Reads one positive `u32` from a nested image descriptor.
fn positive_u32(
    descriptor: &Map<String, JsonValue>,
    field: &str,
    key: &str,
) -> Result<u32, String> {
    let raw = descriptor
        .get(key)
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| {
            format!("`{VISUAL_OBSERVATION_FIELD}.{field}.{key}` must be a positive integer")
        })?;
    if raw == 0 {
        return Err(format!(
            "`{VISUAL_OBSERVATION_FIELD}.{field}.{key}` must be greater than zero"
        ));
    }
    u32::try_from(raw).map_err(|_| {
        format!("`{VISUAL_OBSERVATION_FIELD}.{field}.{key}` exceeds the u32 image dimension range")
    })
}

/// Loads one stored BGRA frame and validates the descriptor's byte count.
fn read_bgra(database: &DatabaseHandle, descriptor: &ImageDescriptor) -> Result<Vec<u8>, String> {
    let bytes = {
        let database = database.lock().map_err(|_| {
            "the assembly-owned visual source database mutex is poisoned".to_owned()
        })?;
        database
            .blob_store()
            .get(database.connection(), &descriptor.blob_id)
            .map_err(|error| format!("blob `{}` could not be read: {error}", descriptor.blob_id))
    }?;
    validate_bgra_length(descriptor, bytes)
}

/// Validates `width * height * 4 == bytes.len()` with explicit overflow checks.
fn validate_bgra_length(descriptor: &ImageDescriptor, bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    let pixels = u64::from(descriptor.width)
        .checked_mul(u64::from(descriptor.height))
        .ok_or_else(|| "image dimensions overflow the BGRA byte count".to_owned())?;
    let expected = pixels
        .checked_mul(4)
        .ok_or_else(|| "image dimensions overflow the BGRA byte count".to_owned())?;
    if u64::try_from(bytes.len()).ok() != Some(expected) {
        return Err(format!(
            "blob `{}` is {} bytes, but {}x{} BGRA needs {} bytes",
            descriptor.blob_id,
            bytes.len(),
            descriptor.width,
            descriptor.height,
            expected
        ));
    }
    Ok(bytes)
}

/// Converts one row-major BGRA buffer to the verify crate's grayscale shape.
///
/// The integer Rec.709 luma formula is fixed by ADR-0079 so the conversion is
/// deterministic across platforms. Alpha is intentionally ignored: the
/// Windows capture channel releases opaque window surfaces.
fn bgra_to_gray(width: u32, height: u32, bgra: &[u8]) -> Result<GrayImage, String> {
    let mut pixels = Vec::with_capacity(bgra.len() / 4);
    let (pixels_bgra, remainder) = bgra.as_chunks::<4>();
    if !remainder.is_empty() {
        return Err("BGRA buffer length is not divisible by four".to_owned());
    }
    for &[blue, green, red, _alpha] in pixels_bgra {
        let luma = (77 * u32::from(red) + 150 * u32::from(green) + 29 * u32::from(blue) + 128) >> 8;
        let gray = u8::try_from(luma)
            .map_err(|_| format!("Rec.709 luma {luma} does not fit in one byte"))?;
        pixels.push(gray);
    }
    GrayImage::new(width, height, pixels)
        .map_err(|error: VisualError| format!("grayscale image is invalid: {error}"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use super::bgra_to_gray;

    #[test]
    fn test_bgra_to_gray_uses_the_rec709_integer_formula() {
        let gray = bgra_to_gray(2, 1, &[0, 0, 255, 255, 255, 0, 0, 255]).expect("valid BGRA image");
        assert_eq!(gray.pixels(), &[77, 29]);
        assert_eq!(gray.width(), 2);
        assert_eq!(gray.height(), 1);
    }

    #[test]
    fn test_bgra_to_gray_rejects_non_pixel_buffer() {
        let error = bgra_to_gray(1, 1, &[0, 0, 255]).expect_err("partial pixel must fail");
        assert!(error.contains("divisible"));
    }
}
