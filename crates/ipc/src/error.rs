//! IPC error taxonomy.
//!
//! Every failure maps to the protocol-wide [`ErrorCategory`]; no variant silently
//! falls back to an empty frame, a fresh session, or a success value.

use std::fmt;

use assistant_protocol::ErrorCategory;

/// Result type used by all IPC operations.
pub type IpcResult<T> = Result<T, IpcError>;

/// Stable IPC failure categories.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum IpcError {
    /// The fixed four-byte frame magic was not present.
    MagicMismatch {
        /// Magic value observed on the wire.
        received: u32,
    },
    /// The frame payload exceeded the protocol's 16 MiB ceiling.
    EnvelopeTooLarge {
        /// Observed payload size.
        size: usize,
        /// Contract maximum.
        maximum: usize,
    },
    /// The encoded frame length did not match its declared envelope size.
    FrameLengthMismatch {
        /// Length required by the declared envelope size.
        expected: usize,
        /// Length actually available.
        actual: usize,
    },
    /// The CRC32 checksum did not match the payload.
    ChecksumMismatch {
        /// Checksum carried by the frame.
        expected: u32,
        /// Checksum computed from the payload.
        actual: u32,
    },
    /// The peer advertised a protocol version this build cannot serve.
    ProtocolVersionMismatch {
        /// Version supported by this build.
        expected: u32,
        /// Version advertised by the peer.
        actual: u32,
    },
    /// Authentication material was missing or outside the accepted bounds.
    InvalidAuthenticationToken,
    /// The peer supplied a token that did not match the server's token.
    AuthenticationFailed,
    /// The server could not obtain the connecting process identity.
    PeerIdentityUnavailable {
        /// Platform-specific failure detail without secret material.
        message: String,
    },
    /// The peer image was not present in the configured allow-list.
    PeerIdentityRejected {
        /// Image path returned by the operating system.
        image_path: String,
    },
    /// The message kind was not valid for the current protocol phase.
    UnexpectedMessage {
        /// Message kind required by the current phase.
        expected: &'static str,
        /// Message kind actually received.
        actual: &'static str,
    },
    /// JSON serialization or decoding failed.
    Serialization {
        /// Serde error text.
        message: String,
    },
    /// A bounded operation exceeded its deadline.
    Timeout {
        /// Stable operation label.
        operation: &'static str,
        /// Deadline in milliseconds.
        timeout_ms: u64,
    },
    /// The peer closed the pipe or the underlying transport ended.
    Disconnected {
        /// Transport context for the disconnect.
        message: String,
    },
    /// The current platform does not provide this transport.
    UnsupportedPlatform,
    /// A Win32 or transport failure that does not have a narrower category.
    Transport {
        /// Platform failure detail without secret material.
        message: String,
    },
}

impl IpcError {
    /// The protocol-wide category callers and audit records should persist.
    #[must_use]
    pub const fn code(&self) -> ErrorCategory {
        match self {
            Self::MagicMismatch { .. }
            | Self::EnvelopeTooLarge { .. }
            | Self::FrameLengthMismatch { .. }
            | Self::ProtocolVersionMismatch { .. }
            | Self::UnexpectedMessage { .. }
            | Self::Serialization { .. } => ErrorCategory::ToolInvalidArgs,
            Self::ChecksumMismatch { .. } => ErrorCategory::VerifyFailed,
            Self::InvalidAuthenticationToken
            | Self::AuthenticationFailed
            | Self::PeerIdentityUnavailable { .. }
            | Self::PeerIdentityRejected { .. } => ErrorCategory::PlatformPermission,
            Self::Timeout { .. } | Self::Disconnected { .. } => ErrorCategory::TargetUnresponsive,
            Self::UnsupportedPlatform => ErrorCategory::CapabilityMissing,
            Self::Transport { .. } => ErrorCategory::Transient,
        }
    }

    /// Construct a timeout error with a millisecond payload.
    #[must_use]
    pub fn timeout(operation: &'static str, timeout: std::time::Duration) -> Self {
        let timeout_ms = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);
        Self::Timeout {
            operation,
            timeout_ms,
        }
    }
}

impl fmt::Display for IpcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MagicMismatch { received } => {
                write!(formatter, "invalid IPC frame magic 0x{received:08X}")
            }
            Self::EnvelopeTooLarge { size, maximum } => {
                write!(
                    formatter,
                    "IPC envelope is {size} bytes; maximum is {maximum}"
                )
            }
            Self::FrameLengthMismatch { expected, actual } => {
                write!(
                    formatter,
                    "IPC frame is {actual} bytes; expected {expected}"
                )
            }
            Self::ChecksumMismatch { expected, actual } => {
                write!(
                    formatter,
                    "IPC CRC32 mismatch: expected 0x{expected:08X}, got 0x{actual:08X}"
                )
            }
            Self::ProtocolVersionMismatch { expected, actual } => {
                write!(
                    formatter,
                    "IPC protocol version mismatch: expected {expected}, got {actual}"
                )
            }
            Self::InvalidAuthenticationToken => {
                formatter.write_str("IPC authentication token is missing or invalid")
            }
            Self::AuthenticationFailed => formatter.write_str("IPC authentication failed"),
            Self::PeerIdentityUnavailable { message } => {
                write!(formatter, "IPC peer identity unavailable: {message}")
            }
            Self::PeerIdentityRejected { image_path } => {
                write!(formatter, "IPC peer image is not allowed: {image_path}")
            }
            Self::UnexpectedMessage { expected, actual } => {
                write!(
                    formatter,
                    "unexpected IPC message {actual}; expected {expected}"
                )
            }
            Self::Serialization { message } => {
                write!(formatter, "IPC serialization failed: {message}")
            }
            Self::Timeout {
                operation,
                timeout_ms,
            } => write!(
                formatter,
                "IPC operation {operation} timed out after {timeout_ms} ms"
            ),
            Self::Disconnected { message } => {
                write!(formatter, "IPC peer disconnected: {message}")
            }
            Self::UnsupportedPlatform => {
                formatter.write_str("this IPC transport is not available on this platform")
            }
            Self::Transport { message } => write!(formatter, "IPC transport failure: {message}"),
        }
    }
}

impl std::error::Error for IpcError {}

#[cfg(test)]
mod tests {
    use assistant_protocol::ErrorCategory;

    use super::IpcError;

    #[test]
    fn test_error_categories_cover_protocol_boundaries() {
        assert_eq!(
            IpcError::MagicMismatch { received: 0 }.code(),
            ErrorCategory::ToolInvalidArgs
        );
        assert_eq!(
            IpcError::ChecksumMismatch {
                expected: 1,
                actual: 2,
            }
            .code(),
            ErrorCategory::VerifyFailed
        );
        assert_eq!(
            IpcError::AuthenticationFailed.code(),
            ErrorCategory::PlatformPermission
        );
        assert_eq!(
            IpcError::UnsupportedPlatform.code(),
            ErrorCategory::CapabilityMissing
        );
    }
}
