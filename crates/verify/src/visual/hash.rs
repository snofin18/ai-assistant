//! Perceptual hashes (pHash / dHash) and Hamming distance.
//!
//! Responsibility: reduce a validated grayscale image to a 64-bit fingerprint that survives small
//! brightness / noise perturbations but separates clearly different images. These are deliberately
//! **not** state fingerprints (architecture section 7.3, `crate::fingerprint.ts`): a state
//! fingerprint answers "did this exact state change", while a perceptual hash answers "does this
//! image still look like the reference".
//!
//! Boundary: no OCR, no template matching, no region selection, no image codec. Those need the
//! platform screenshot layer and are explicitly out of this card (ADR-0074 D2).
//!
//! Algorithm口径 is frozen by ADR-0074 D5 / D6:
//! - **pHash**: 32x32 box downsample, raw 2D DCT-II (no alpha normalization) over the top-left
//!   8x8 coefficients including DC, then bit = coefficient strictly greater than the median of
//!   those 64 coefficients.
//! - **dHash**: 9x8 box downsample, then bit = "left pixel strictly greater than right pixel" for
//!   each of the 8 comparisons in each of the 8 rows.
//!
//! Both hashes are 64-bit; the bit order is part of the contract, not an implementation detail.
//!
//! Invariants:
//! 1. Equal images give bit-identical hashes; there is no randomness, clock, or platform branch.
//! 2. Hamming distance is symmetric and zero exactly for equal hashes.
//! 3. The accepted threshold never exceeds [`MAX_HAMMING_DISTANCE`]; larger values are rejected.

use crate::visual::error::VisualError;
use crate::visual::image::{GrayImage, downsample};

/// Side length of the pHash working grid.
pub const PHASH_SIDE: usize = 32;
/// Number of low-frequency DCT coefficients kept per axis.
pub const PHASH_LOW_FREQUENCY_SIDE: usize = 8;
/// Width of the dHash working grid (one extra column for the horizontal differences).
pub const DHASH_WIDTH: usize = 9;
/// Height of the dHash working grid.
pub const DHASH_HEIGHT: usize = 8;
/// Number of bits in both perceptual hashes.
pub const PERCEPTUAL_HASH_BITS: u32 = 64;

/// Hard upper bound on `max_hamming_distance`.
///
/// Under the null hypothesis that two unrelated images produce independent uniform 64-bit hashes,
/// `P(distance <= 24) = sum_{k=0..24} C(64, k) / 2^64 = 2.997%`, below the stage-1 target of a
/// false-match rate under 5%. A threshold above 24 would make "different images" match too often,
/// so parsing rejects it (ADR-0074 D7).
pub const MAX_HAMMING_DISTANCE: u32 = 24;

/// A 64-bit perceptual hash.
///
/// Wraps the raw bits so the hash cannot be confused with a state fingerprint (a `sha256:` string)
/// or with a plain integer in a signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PerceptualHash {
    bits: u64,
}

impl PerceptualHash {
    /// Builds a hash from its raw 64-bit representation.
    #[must_use]
    pub const fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    /// The raw 64-bit representation.
    #[must_use]
    pub const fn bits(self) -> u64 {
        self.bits
    }

    /// Renders the hash as exactly 16 lowercase hexadecimal characters.
    #[must_use]
    pub fn to_hex(self) -> String {
        format!("{:016x}", self.bits)
    }
}

/// Computes the pHash of a grayscale image.
///
/// # Errors
///
/// Returns a [`VisualError`] when the image cannot be reduced to the fixed 32x32 grid or when the
/// internal DCT arithmetic hits an invariant break. Malformed image input is rejected by
/// [`GrayImage::new`] before this function is called.
pub fn compute_phash(image: &GrayImage) -> Result<PerceptualHash, VisualError> {
    let small = downsample(image, PHASH_SIDE, PHASH_SIDE)?;
    let cosine_table = build_cosine_table()?;
    let mut coefficients = [0.0_f64; PHASH_LOW_FREQUENCY_SIDE * PHASH_LOW_FREQUENCY_SIDE];

    for vertical in 0..PHASH_LOW_FREQUENCY_SIDE {
        for horizontal in 0..PHASH_LOW_FREQUENCY_SIDE {
            let mut sum = 0.0_f64;
            for y in 0..PHASH_SIDE {
                let vertical_term = cosine_term_at(&cosine_table, y, vertical)?;
                for x in 0..PHASH_SIDE {
                    let value = sample(&small, y * PHASH_SIDE + x)?;
                    let horizontal_term = cosine_term_at(&cosine_table, x, horizontal)?;
                    sum = (f64::from(value) * horizontal_term).mul_add(vertical_term, sum);
                }
            }
            let slot = coefficients
                .get_mut(vertical * PHASH_LOW_FREQUENCY_SIDE + horizontal)
                .ok_or_else(|| VisualError::Inconsistent {
                    operation: "compute phash",
                    reason: format!("coefficient slot ({horizontal}, {vertical}) is missing"),
                })?;
            *slot = sum;
        }
    }

    let mut sorted = coefficients;
    sorted.sort_by(f64::total_cmp);
    let lower = sorted
        .get(31)
        .copied()
        .ok_or_else(|| VisualError::Inconsistent {
            operation: "compute phash",
            reason: "the 64-coefficient array has no lower median element".to_owned(),
        })?;
    let upper = sorted
        .get(32)
        .copied()
        .ok_or_else(|| VisualError::Inconsistent {
            operation: "compute phash",
            reason: "the 64-coefficient array has no upper median element".to_owned(),
        })?;
    let median = f64::midpoint(lower, upper);

    let mut bits = 0_u64;
    for (index, coefficient) in coefficients.iter().enumerate() {
        if *coefficient > median {
            bits |= 1_u64 << index;
        }
    }
    Ok(PerceptualHash::from_bits(bits))
}

/// Computes the dHash of a grayscale image.
///
/// # Errors
///
/// Returns a [`VisualError`] when the image cannot be reduced to the fixed 9x8 grid or when an
/// internal grid lookup breaks. Malformed image input is rejected by [`GrayImage::new`].
pub fn compute_dhash(image: &GrayImage) -> Result<PerceptualHash, VisualError> {
    let small = downsample(image, DHASH_WIDTH, DHASH_HEIGHT)?;
    let mut bits = 0_u64;
    for row in 0..DHASH_HEIGHT {
        for column in 0..(DHASH_WIDTH - 1) {
            let left = sample(&small, row * DHASH_WIDTH + column)?;
            let right = sample(&small, row * DHASH_WIDTH + column + 1)?;
            if left > right {
                bits |= 1_u64 << (row * (DHASH_WIDTH - 1) + column);
            }
        }
    }
    Ok(PerceptualHash::from_bits(bits))
}

/// Hamming distance between two perceptual hashes.
#[must_use]
pub const fn hamming_distance(left: PerceptualHash, right: PerceptualHash) -> u32 {
    (left.bits ^ right.bits).count_ones()
}

/// Reads one element from a downsampled grid.
fn sample(grid: &[u8], index: usize) -> Result<u8, VisualError> {
    grid.get(index)
        .copied()
        .ok_or_else(|| VisualError::Inconsistent {
            operation: "sample downsampled grid",
            reason: format!("index {index} is outside a {}-byte grid", grid.len()),
        })
}

/// One DCT-II cosine term, computed with `u8` inputs so no lossy integer cast is needed.
fn cosine_term(offset: usize, frequency: usize, size: usize) -> Result<f64, VisualError> {
    let offset_byte = u8::try_from(offset).map_err(|_| VisualError::Inconsistent {
        operation: "build dct cosine table",
        reason: format!("offset {offset} does not fit in u8"),
    })?;
    let frequency_byte = u8::try_from(frequency).map_err(|_| VisualError::Inconsistent {
        operation: "build dct cosine table",
        reason: format!("frequency {frequency} does not fit in u8"),
    })?;
    let size_byte = u8::try_from(size).map_err(|_| VisualError::Inconsistent {
        operation: "build dct cosine table",
        reason: format!("size {size} does not fit in u8"),
    })?;
    let numerator = std::f64::consts::PI
        * 2.0f64.mul_add(f64::from(offset_byte), 1.0)
        * f64::from(frequency_byte);
    let denominator = 2.0 * f64::from(size_byte);
    Ok((numerator / denominator).cos())
}

/// Precomputes the cosine terms shared by every DCT coefficient.
fn build_cosine_table() -> Result<Vec<f64>, VisualError> {
    let mut table = Vec::with_capacity(PHASH_SIDE * PHASH_LOW_FREQUENCY_SIDE);
    for offset in 0..PHASH_SIDE {
        for frequency in 0..PHASH_LOW_FREQUENCY_SIDE {
            table.push(cosine_term(offset, frequency, PHASH_SIDE)?);
        }
    }
    Ok(table)
}

/// Looks up a cosine term; index layout matches [`build_cosine_table`].
fn cosine_term_at(table: &[f64], offset: usize, frequency: usize) -> Result<f64, VisualError> {
    table
        .get(frequency * PHASH_SIDE + offset)
        .copied()
        .ok_or_else(|| VisualError::Inconsistent {
            operation: "lookup dct cosine term",
            reason: format!("cosine term (offset={offset}, frequency={frequency}) is missing"),
        })
}
