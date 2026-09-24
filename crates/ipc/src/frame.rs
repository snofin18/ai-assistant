//! Wire-frame encoding and decoding.
//!
//! The layout is fixed by `docs/spec/ipc-protocol.md` section 3:
//! `magic (u32 LE) | envelope_size (u32 LE) | envelope | crc32 (u32 LE)`.
//! This module is deliberately pure: it performs no I/O and can be tested on
//! every CI target.

use crate::{IpcError, IpcResult};

/// Fixed frame magic from the IPC contract.
pub const FRAME_MAGIC: u32 = 0xC0DE_CAFE;
/// Maximum serialized envelope size accepted by the transport.
pub const MAX_ENVELOPE_SIZE: usize = 16 * 1024 * 1024;

const PREFIX_SIZE: usize = 8;
const CHECKSUM_SIZE: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FramePrefix {
    pub envelope_size: usize,
}

/// Compute the standard CRC32 (IEEE) checksum used by the frame contract.
#[must_use]
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Encode one serialized envelope into the fixed IPC frame.
///
/// # Errors
/// - [`IpcError::EnvelopeTooLarge`] when `envelope` exceeds 16 MiB.
/// - [`IpcError::FrameLengthMismatch`] if the resulting length cannot be represented.
pub fn encode_frame(envelope: &[u8]) -> IpcResult<Vec<u8>> {
    if envelope.len() > MAX_ENVELOPE_SIZE {
        return Err(IpcError::EnvelopeTooLarge {
            size: envelope.len(),
            maximum: MAX_ENVELOPE_SIZE,
        });
    }
    let envelope_size =
        u32::try_from(envelope.len()).map_err(|_| IpcError::FrameLengthMismatch {
            expected: MAX_ENVELOPE_SIZE,
            actual: envelope.len(),
        })?;

    let mut encoded = Vec::with_capacity(PREFIX_SIZE + envelope.len() + CHECKSUM_SIZE);
    encoded.extend_from_slice(&FRAME_MAGIC.to_le_bytes());
    encoded.extend_from_slice(&envelope_size.to_le_bytes());
    encoded.extend_from_slice(envelope);
    encoded.extend_from_slice(&crc32(envelope).to_le_bytes());
    Ok(encoded)
}

/// Decode a complete IPC frame and return the serialized envelope bytes.
///
/// # Errors
/// The three protocol-level failures are intentionally distinct:
/// magic mismatch, declared length mismatch, and CRC32 mismatch. Oversized
/// frames are rejected before allocation by [`decode_prefix`].
pub fn decode_frame(encoded: &[u8]) -> IpcResult<Vec<u8>> {
    let prefix_bytes = encoded
        .get(..PREFIX_SIZE)
        .ok_or(IpcError::FrameLengthMismatch {
            expected: PREFIX_SIZE + CHECKSUM_SIZE,
            actual: encoded.len(),
        })?;
    let prefix = decode_prefix(prefix_bytes)?;
    let expected_length = PREFIX_SIZE
        .checked_add(prefix.envelope_size)
        .and_then(|size| size.checked_add(CHECKSUM_SIZE))
        .ok_or(IpcError::EnvelopeTooLarge {
            size: prefix.envelope_size,
            maximum: MAX_ENVELOPE_SIZE,
        })?;
    if encoded.len() != expected_length {
        return Err(IpcError::FrameLengthMismatch {
            expected: expected_length,
            actual: encoded.len(),
        });
    }

    let envelope_start = PREFIX_SIZE;
    let envelope_end = PREFIX_SIZE + prefix.envelope_size;
    let envelope =
        encoded
            .get(envelope_start..envelope_end)
            .ok_or(IpcError::FrameLengthMismatch {
                expected: expected_length,
                actual: encoded.len(),
            })?;
    let checksum_bytes =
        encoded
            .get(envelope_end..expected_length)
            .ok_or(IpcError::FrameLengthMismatch {
                expected: expected_length,
                actual: encoded.len(),
            })?;
    let checksum_array = <[u8; CHECKSUM_SIZE]>::try_from(checksum_bytes).map_err(|_| {
        IpcError::FrameLengthMismatch {
            expected: expected_length,
            actual: encoded.len(),
        }
    })?;
    let expected_checksum = u32::from_le_bytes(checksum_array);
    let actual_checksum = crc32(envelope);
    if expected_checksum != actual_checksum {
        return Err(IpcError::ChecksumMismatch {
            expected: expected_checksum,
            actual: actual_checksum,
        });
    }
    Ok(envelope.to_vec())
}

/// Decode the fixed prefix so a streaming transport can bound its next read.
pub fn decode_prefix(prefix: &[u8]) -> IpcResult<FramePrefix> {
    let magic_bytes = prefix.get(..4).ok_or(IpcError::FrameLengthMismatch {
        expected: PREFIX_SIZE,
        actual: prefix.len(),
    })?;
    let size_bytes = prefix
        .get(4..PREFIX_SIZE)
        .ok_or(IpcError::FrameLengthMismatch {
            expected: PREFIX_SIZE,
            actual: prefix.len(),
        })?;
    let magic_array =
        <[u8; 4]>::try_from(magic_bytes).map_err(|_| IpcError::FrameLengthMismatch {
            expected: PREFIX_SIZE,
            actual: prefix.len(),
        })?;
    let size_array =
        <[u8; 4]>::try_from(size_bytes).map_err(|_| IpcError::FrameLengthMismatch {
            expected: PREFIX_SIZE,
            actual: prefix.len(),
        })?;
    let received_magic = u32::from_le_bytes(magic_array);
    if received_magic != FRAME_MAGIC {
        return Err(IpcError::MagicMismatch {
            received: received_magic,
        });
    }
    let envelope_size = usize::try_from(u32::from_le_bytes(size_array)).map_err(|_| {
        IpcError::EnvelopeTooLarge {
            size: usize::MAX,
            maximum: MAX_ENVELOPE_SIZE,
        }
    })?;
    if envelope_size > MAX_ENVELOPE_SIZE {
        return Err(IpcError::EnvelopeTooLarge {
            size: envelope_size,
            maximum: MAX_ENVELOPE_SIZE,
        });
    }
    Ok(FramePrefix { envelope_size })
}

#[cfg(test)]
mod tests {
    use super::{FRAME_MAGIC, MAX_ENVELOPE_SIZE, crc32, decode_frame, encode_frame};
    use crate::IpcError;

    #[test]
    fn test_crc32_known_vector() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn test_frame_round_trip_preserves_payload() {
        let encoded = encode_frame(b"{\"kind\":\"heartbeat\"}");
        assert!(encoded.is_ok());
        let Ok(encoded) = encoded else {
            return;
        };
        assert_eq!(encoded.get(..4), Some(FRAME_MAGIC.to_le_bytes().as_slice()));
        let decoded = decode_frame(&encoded);
        assert_eq!(decoded, Ok(b"{\"kind\":\"heartbeat\"}".to_vec()));
    }

    #[test]
    fn test_decode_frame_rejects_bad_magic() {
        let result = decode_frame(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(matches!(result, Err(IpcError::MagicMismatch { .. })));
    }

    #[test]
    fn test_decode_frame_rejects_bad_crc() {
        let Ok(mut encoded) = encode_frame(b"payload") else {
            return;
        };
        if let Some(last) = encoded.last_mut() {
            *last ^= 0xFF;
        }
        assert!(matches!(
            decode_frame(&encoded),
            Err(IpcError::ChecksumMismatch { .. })
        ));
    }

    #[test]
    fn test_decode_frame_rejects_oversized_envelope() {
        let mut prefix = FRAME_MAGIC.to_le_bytes().to_vec();
        let oversized = u32::try_from(MAX_ENVELOPE_SIZE + 1).unwrap_or(u32::MAX);
        prefix.extend_from_slice(&oversized.to_le_bytes());
        assert!(matches!(
            decode_frame(&prefix),
            Err(IpcError::EnvelopeTooLarge { .. })
        ));
    }

    #[test]
    fn test_decode_frame_rejects_length_mismatch() {
        let Ok(mut encoded) = encode_frame(b"payload") else {
            return;
        };
        encoded.push(0);
        assert!(matches!(
            decode_frame(&encoded),
            Err(IpcError::FrameLengthMismatch { .. })
        ));
    }
}
