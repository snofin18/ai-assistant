//! Transport abstraction and platform selection.

use std::time::Duration;

use crate::{IpcResult, WireMessage};

#[cfg(not(windows))]
use crate::IpcError;

/// A framed, reliable, ordered IPC transport.
pub trait Transport {
    /// Connect as the client.
    ///
    /// # Errors
    /// Returns timeout, permission, disconnected, or unsupported-platform errors.
    fn connect(&mut self, timeout: Duration) -> IpcResult<()>;

    /// Accept one client as the server.
    ///
    /// # Errors
    /// Returns timeout, disconnected, or unsupported-platform errors.
    fn accept(&mut self, timeout: Duration) -> IpcResult<()>;

    /// Send one complete wire message.
    ///
    /// # Errors
    /// Returns serialization, timeout, disconnected, or transport errors.
    fn send(&mut self, message: &WireMessage, timeout: Duration) -> IpcResult<()>;

    /// Receive one complete wire message or fail on timeout/disconnect.
    ///
    /// # Errors
    /// Returns timeout, disconnected, serialization, or protocol errors.
    fn recv(&mut self, timeout: Duration) -> IpcResult<WireMessage>;

    /// Close the connection and release its OS resources.
    ///
    /// # Errors
    /// Returns a transport error if the OS reports a close failure.
    fn close(&mut self) -> IpcResult<()>;

    /// Process identifier of the connected peer.
    ///
    /// # Errors
    /// Returns [`IpcError::PeerIdentityUnavailable`] when the platform cannot
    /// prove the peer identity.
    fn peer_process_id(&self) -> IpcResult<u32>;
}

#[cfg(windows)]
pub use crate::windows_transport::NamedPipeTransport;

/// Query the image path for a process identifier.
///
/// # Errors
/// Fails closed when the identity cannot be read.
#[cfg(windows)]
pub fn peer_process_image_path(process_id: u32) -> IpcResult<String> {
    crate::windows_peer::process_image_path(process_id)
}

/// Query the image path for a process identifier.
///
/// # Errors
/// Always returns [`IpcError::UnsupportedPlatform`] outside Windows.
#[cfg(not(windows))]
pub const fn peer_process_image_path(_process_id: u32) -> IpcResult<String> {
    Err(IpcError::UnsupportedPlatform)
}

/// Non-Windows transport placeholder that fails explicitly.
#[cfg(not(windows))]
#[derive(Debug, Default)]
pub struct NamedPipeTransport;

#[cfg(not(windows))]
impl NamedPipeTransport {
    /// Construct the placeholder for a pipe name.
    ///
    /// # Errors
    /// Always fails with [`IpcError::UnsupportedPlatform`].
    pub const fn client(_pipe_name: &str) -> IpcResult<Self> {
        Err(IpcError::UnsupportedPlatform)
    }

    /// Construct the placeholder for a pipe name.
    ///
    /// # Errors
    /// Always fails with [`IpcError::UnsupportedPlatform`].
    pub const fn server(_pipe_name: &str) -> IpcResult<Self> {
        Err(IpcError::UnsupportedPlatform)
    }
}

#[cfg(not(windows))]
impl Transport for NamedPipeTransport {
    fn connect(&mut self, _timeout: Duration) -> IpcResult<()> {
        Err(IpcError::UnsupportedPlatform)
    }

    fn accept(&mut self, _timeout: Duration) -> IpcResult<()> {
        Err(IpcError::UnsupportedPlatform)
    }

    fn send(&mut self, _message: &WireMessage, _timeout: Duration) -> IpcResult<()> {
        Err(IpcError::UnsupportedPlatform)
    }

    fn recv(&mut self, _timeout: Duration) -> IpcResult<WireMessage> {
        Err(IpcError::UnsupportedPlatform)
    }

    fn close(&mut self) -> IpcResult<()> {
        Err(IpcError::UnsupportedPlatform)
    }

    fn peer_process_id(&self) -> IpcResult<u32> {
        Err(IpcError::UnsupportedPlatform)
    }
}
