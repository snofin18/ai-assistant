//! Windows `NamedPipe` transport.
//!
//! Every `unsafe` call is paired with a SAFETY comment. The pipe rejects remote
//! clients and inherits the process default DACL; the host then validates the
//! peer image and authentication token before serving the session.

use std::thread;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{
    ERROR_FILE_NOT_FOUND, ERROR_IO_PENDING, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, HANDLE,
    INVALID_HANDLE_VALUE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_OVERLAPPED, FILE_SHARE_NONE, OPEN_EXISTING,
    PIPE_ACCESS_DUPLEX,
};
use windows::Win32::System::IO::OVERLAPPED;
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeClientProcessId,
    PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
    SetNamedPipeHandleState,
};
use windows::core::PCWSTR;

use crate::windows_io::{
    OwnedHandle, abort_on_drop_failure, create_event, last_error_transport, read_exact,
    transport_failure, wait_overlapped, win32_code, write_all,
};
use crate::{IpcError, IpcResult, WireMessage, decode_frame, encode_frame, frame::decode_prefix};

const PIPE_BUFFER_SIZE: u32 = 64 * 1024;
const CONNECT_RETRY_MS: u64 = 20;

/// Windows `NamedPipe` transport.
pub struct NamedPipeTransport {
    pipe_name: String,
    handle: Option<OwnedHandle>,
    is_server: bool,
    is_connected: bool,
}

impl NamedPipeTransport {
    /// Construct a client transport for `pipe_name`.
    ///
    /// # Errors
    /// Returns [`IpcError::Serialization`] for an invalid pipe name.
    pub fn client(pipe_name: &str) -> IpcResult<Self> {
        validate_pipe_name(pipe_name)?;
        Ok(Self {
            pipe_name: pipe_name.to_string(),
            handle: None,
            is_server: false,
            is_connected: false,
        })
    }

    /// Construct a server transport and create its pipe instance.
    ///
    /// # Errors
    /// Returns a transport error when the pipe cannot be created.
    pub fn server(pipe_name: &str) -> IpcResult<Self> {
        validate_pipe_name(pipe_name)?;
        let full_name = wide_pipe_name(pipe_name);
        // SAFETY: `full_name` is NUL-terminated and remains alive for the call.
        // A null security descriptor deliberately uses the process default DACL;
        // PIPE_REJECT_REMOTE_CLIENTS prevents remote clients.
        let raw_handle = unsafe {
            CreateNamedPipeW(
                PCWSTR(full_name.as_ptr()),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                PIPE_BUFFER_SIZE,
                PIPE_BUFFER_SIZE,
                0,
                None,
            )
        };
        if raw_handle == INVALID_HANDLE_VALUE || raw_handle.is_invalid() {
            return Err(last_error_transport("CreateNamedPipeW"));
        }
        Ok(Self {
            pipe_name: pipe_name.to_string(),
            handle: Some(OwnedHandle::new(raw_handle)),
            is_server: true,
            is_connected: false,
        })
    }

    fn raw_handle(&self) -> IpcResult<HANDLE> {
        self.handle
            .as_ref()
            .map(OwnedHandle::raw)
            .ok_or_else(|| IpcError::Transport {
                message: "named pipe handle is not open".to_string(),
            })
    }
}

impl crate::Transport for NamedPipeTransport {
    fn connect(&mut self, timeout: Duration) -> IpcResult<()> {
        if self.is_server {
            return Err(IpcError::Transport {
                message: "server transport cannot call connect".to_string(),
            });
        }
        let full_name = wide_pipe_name(&self.pipe_name);
        let deadline = Instant::now() + timeout;
        loop {
            // SAFETY: `full_name` is a valid NUL-terminated path and the return value
            // is checked before ownership is stored.
            let opened = unsafe {
                CreateFileW(
                    PCWSTR(full_name.as_ptr()),
                    (windows::Win32::Foundation::GENERIC_READ
                        | windows::Win32::Foundation::GENERIC_WRITE)
                        .0,
                    FILE_SHARE_NONE,
                    None,
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OVERLAPPED,
                    None,
                )
            };
            match opened {
                Ok(handle) => {
                    self.handle = Some(OwnedHandle::new(handle));
                    self.is_connected = true;
                    let mode = PIPE_READMODE_BYTE;
                    // SAFETY: the handle is connected and valid for named-pipe state.
                    unsafe { SetNamedPipeHandleState(handle, Some(&raw const mode), None, None) }
                        .map_err(|failure| transport_failure("SetNamedPipeHandleState", &failure))?;
                    return Ok(());
                }
                Err(failure) => {
                    let code = win32_code(&failure);
                    if code == ERROR_FILE_NOT_FOUND.0 || code == ERROR_PIPE_BUSY.0 {
                        if Instant::now() >= deadline {
                            return Err(IpcError::timeout("named-pipe-connect", timeout));
                        }
                        thread::sleep(Duration::from_millis(CONNECT_RETRY_MS));
                    } else {
                        return Err(transport_failure("CreateFileW", &failure));
                    }
                }
            }
        }
    }

    fn accept(&mut self, timeout: Duration) -> IpcResult<()> {
        if !self.is_server {
            return Err(IpcError::Transport {
                message: "client transport cannot call accept".to_string(),
            });
        }
        let handle = self.raw_handle()?;
        let event = create_event()?;
        let mut overlapped = OVERLAPPED {
            hEvent: event.raw(),
            ..Default::default()
        };
        // SAFETY: `handle` is a valid overlapped server pipe, and `overlapped`
        // remains alive until the operation completes or is cancelled below.
        match unsafe { ConnectNamedPipe(handle, Some(&raw mut overlapped)) } {
            Ok(()) => {
                self.is_connected = true;
                Ok(())
            }
            Err(failure) if win32_code(&failure) == ERROR_PIPE_CONNECTED.0 => {
                self.is_connected = true;
                Ok(())
            }
            Err(failure) if win32_code(&failure) == ERROR_IO_PENDING.0 => {
                wait_overlapped(
                    handle,
                    &overlapped,
                    event.raw(),
                    timeout,
                    "named-pipe-accept",
                )?;
                self.is_connected = true;
                Ok(())
            }
            Err(failure) => Err(transport_failure("ConnectNamedPipe", &failure)),
        }
    }

    fn send(&mut self, message: &WireMessage, timeout: Duration) -> IpcResult<()> {
        let envelope = message.to_json()?;
        let frame = encode_frame(&envelope)?;
        let handle = self.raw_handle()?;
        write_all(handle, &frame, timeout, "named-pipe-send")
    }

    fn recv(&mut self, timeout: Duration) -> IpcResult<WireMessage> {
        let handle = self.raw_handle()?;
        let mut prefix_bytes = [0_u8; 8];
        read_exact(handle, &mut prefix_bytes, timeout, "named-pipe-prefix")?;
        let prefix = decode_prefix(&prefix_bytes)?;
        let remaining_size =
            prefix
                .envelope_size
                .checked_add(4)
                .ok_or(IpcError::FrameLengthMismatch {
                    expected: 12,
                    actual: prefix.envelope_size,
                })?;
        let mut remaining = vec![0_u8; remaining_size];
        read_exact(handle, &mut remaining, timeout, "named-pipe-envelope")?;
        let mut frame = prefix_bytes.to_vec();
        frame.extend_from_slice(&remaining);
        let envelope = decode_frame(&frame)?;
        WireMessage::from_json(&envelope)
    }

    fn close(&mut self) -> IpcResult<()> {
        if let Some(handle) = self.handle.as_ref()
            && self.is_server
            && self.is_connected
        {
            // SAFETY: this is the live server pipe instance.
            unsafe { DisconnectNamedPipe(handle.raw()) }
                .map_err(|failure| transport_failure("DisconnectNamedPipe", &failure))?;
        }
        if let Some(mut handle) = self.handle.take() {
            handle.close()?;
        }
        self.is_connected = false;
        Ok(())
    }

    fn peer_process_id(&self) -> IpcResult<u32> {
        if !self.is_server || !self.is_connected {
            return Err(IpcError::PeerIdentityUnavailable {
                message: "peer_process_id is only valid on an accepted server connection"
                    .to_string(),
            });
        }
        let handle = self.raw_handle()?;
        let mut process_id = 0_u32;
        // SAFETY: `handle` is a connected server pipe and `process_id` is writable.
        unsafe { GetNamedPipeClientProcessId(handle, &raw mut process_id) }.map_err(|failure| {
            IpcError::PeerIdentityUnavailable {
                message: transport_failure("GetNamedPipeClientProcessId", &failure).to_string(),
            }
        })?;
        if process_id == 0 {
            return Err(IpcError::PeerIdentityUnavailable {
                message: "GetNamedPipeClientProcessId returned zero".to_string(),
            });
        }
        Ok(process_id)
    }
}

impl Drop for NamedPipeTransport {
    fn drop(&mut self) {
        if let Some(mut handle) = self.handle.take()
            && let Err(error) = handle.close()
        {
            // SAFETY: abort is intentional. Dropping an open kernel handle
            // silently would violate the no-silent-failure invariant.
            abort_on_drop_failure(&error);
        }
    }
}

fn validate_pipe_name(pipe_name: &str) -> IpcResult<()> {
    if pipe_name.is_empty()
        || pipe_name.len() > 200
        || pipe_name.contains(['\\', '/', '\0'])
        || pipe_name.chars().any(char::is_whitespace)
    {
        Err(IpcError::Serialization {
            message: "pipe name must be a single non-empty path component".to_string(),
        })
    } else {
        Ok(())
    }
}

fn wide_pipe_name(pipe_name: &str) -> Vec<u16> {
    format!(r"\\.\pipe\{pipe_name}")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::validate_pipe_name;

    #[test]
    fn test_pipe_name_validation_rejects_paths_and_whitespace() {
        assert!(validate_pipe_name("assistant-task-019").is_ok());
        assert!(validate_pipe_name(r"..\escape").is_err());
        assert!(validate_pipe_name("two words").is_err());
        assert!(validate_pipe_name("").is_err());
    }
}
