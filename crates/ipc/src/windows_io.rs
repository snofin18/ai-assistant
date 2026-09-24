//! Shared Windows overlapped-I/O primitives for the named-pipe transport.

use std::time::{Duration, Instant};

use windows::Win32::Foundation::{
    CloseHandle, ERROR_BROKEN_PIPE, ERROR_IO_PENDING, ERROR_OPERATION_ABORTED, GetLastError,
    HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows::Win32::Storage::FileSystem::{ReadFile, WriteFile};
use windows::Win32::System::IO::{
    CancelIoEx, GetOverlappedResult, GetOverlappedResultEx, OVERLAPPED,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::core::PCWSTR;

use crate::{IpcError, IpcResult};

/// Owned Win32 handle that makes close failures explicit during normal control flow.
pub struct OwnedHandle(HANDLE);

impl OwnedHandle {
    /// Wrap a raw handle returned by Win32.
    #[must_use]
    pub const fn new(handle: HANDLE) -> Self {
        Self(handle)
    }

    /// Borrow the raw handle for a Win32 call.
    #[must_use]
    pub const fn raw(&self) -> HANDLE {
        self.0
    }

    /// Close the handle exactly once, returning a transport error on failure.
    ///
    /// # Errors
    /// Returns [`IpcError::Transport`] when `CloseHandle` fails.
    pub fn close(&mut self) -> IpcResult<()> {
        if self.0 == INVALID_HANDLE_VALUE || self.0.is_invalid() {
            return Ok(());
        }
        // SAFETY: this type owns the handle exactly once and marks it invalid
        // before returning.
        unsafe { CloseHandle(self.0) }.map_err(|failure| IpcError::Transport {
            message: format!("CloseHandle failed: {failure}"),
        })?;
        self.0 = INVALID_HANDLE_VALUE;
        Ok(())
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if self.0 != INVALID_HANDLE_VALUE && !self.0.is_invalid() {
            // SAFETY: dropping the last owner must close the kernel handle.
            if let Err(error) = unsafe { CloseHandle(self.0) } {
                abort_on_drop_failure(&IpcError::Transport {
                    message: format!("CloseHandle failed in Drop: {error}"),
                });
            }
            self.0 = INVALID_HANDLE_VALUE;
        }
    }
}

/// Read exactly `buffer.len()` bytes or return timeout/disconnect.
///
/// # Errors
/// Returns a transport, timeout, or disconnect error from the overlapped read.
pub fn read_exact(
    handle: HANDLE,
    buffer: &mut [u8],
    timeout: Duration,
    operation: &'static str,
) -> IpcResult<()> {
    let deadline = Instant::now() + timeout;
    let mut offset = 0_usize;
    let buffer_length = buffer.len();
    while offset < buffer_length {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(IpcError::timeout(operation, timeout));
        }
        let destination = buffer
            .get_mut(offset..)
            .ok_or(IpcError::FrameLengthMismatch {
                expected: buffer_length,
                actual: offset,
            })?;
        let read = read_once(handle, destination, remaining, operation)?;
        if read == 0 {
            return Err(IpcError::Disconnected {
                message: "named pipe returned zero bytes".to_string(),
            });
        }
        offset = offset
            .checked_add(read)
            .ok_or(IpcError::FrameLengthMismatch {
                expected: buffer_length,
                actual: usize::MAX,
            })?;
    }
    Ok(())
}

/// Write the entire buffer or return timeout/disconnect.
///
/// # Errors
/// Returns a transport, timeout, or disconnect error from the overlapped write.
pub fn write_all(
    handle: HANDLE,
    buffer: &[u8],
    timeout: Duration,
    operation: &'static str,
) -> IpcResult<()> {
    let deadline = Instant::now() + timeout;
    let mut offset = 0_usize;
    let buffer_length = buffer.len();
    while offset < buffer_length {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(IpcError::timeout(operation, timeout));
        }
        let source = buffer.get(offset..).ok_or(IpcError::FrameLengthMismatch {
            expected: buffer_length,
            actual: offset,
        })?;
        let written = write_once(handle, source, remaining, operation)?;
        if written == 0 {
            return Err(IpcError::Disconnected {
                message: "named pipe accepted zero bytes".to_string(),
            });
        }
        offset = offset
            .checked_add(written)
            .ok_or(IpcError::FrameLengthMismatch {
                expected: buffer_length,
                actual: usize::MAX,
            })?;
    }
    Ok(())
}

/// Wait for an already-started overlapped operation.
///
/// # Errors
/// Returns timeout, disconnect, or transport errors.
pub fn wait_overlapped(
    handle: HANDLE,
    overlapped: &OVERLAPPED,
    event: HANDLE,
    timeout: Duration,
    operation: &'static str,
) -> IpcResult<()> {
    let timeout_ms = duration_millis(timeout);
    let mut transferred = 0_u32;
    // SAFETY: `overlapped` belongs to a pending operation on `handle`.
    match unsafe {
        GetOverlappedResultEx(handle, overlapped, &raw mut transferred, timeout_ms, false)
    } {
        Ok(()) => Ok(()),
        Err(failure)
            if win32_code(&failure) == ERROR_IO_PENDING.0
                || win32_code(&failure) == windows::Win32::Foundation::WAIT_TIMEOUT.0 =>
        {
            cancel_and_reap(handle, overlapped, event);
            Err(IpcError::timeout(operation, timeout))
        }
        Err(failure) if is_disconnect_code(win32_code(&failure)) => Err(IpcError::Disconnected {
            message: format!("{operation}: {failure}"),
        }),
        Err(failure) => Err(transport_failure(operation, &failure)),
    }
}

/// Create one manual-reset event for an overlapped operation.
///
/// # Errors
/// Returns [`IpcError::Transport`] if the event cannot be created.
pub fn create_event() -> IpcResult<OwnedHandle> {
    // SAFETY: a null name creates an unnamed manual-reset event. Manual reset is
    // required so completion cannot pulse-reset before GetOverlappedResultEx observes it.
    let event = unsafe { CreateEventW(None, true, false, PCWSTR::null()) }
        .map_err(|failure| transport_failure("CreateEventW", &failure))?;
    Ok(OwnedHandle::new(event))
}

/// Convert a Win32 error observed after a direct API call.
///
/// # Errors
/// Never returns; the returned value is always an error category.
#[must_use]
pub fn last_error_transport(operation: &str) -> IpcError {
    // SAFETY: GetLastError has no preconditions and is thread-local.
    let code = unsafe { GetLastError().0 };
    transport_error_code(operation, code)
}

/// Classify a `windows` crate failure from a Win32 API.
///
/// # Errors
/// Never returns; the returned value is always an error category.
#[must_use]
pub fn transport_failure(operation: &str, failure: &windows::core::Error) -> IpcError {
    transport_error_code(operation, win32_code(failure))
}

/// Decode the low 16 bits of `HRESULT_FROM_WIN32`.
#[must_use]
pub const fn win32_code(failure: &windows::core::Error) -> u32 {
    u32::from_ne_bytes(failure.code().0.to_ne_bytes()) & 0x0000_FFFF
}

/// Abort from `Drop` rather than silently leaking an open kernel handle.
pub fn abort_on_drop_failure(error: &IpcError) -> ! {
    let _diagnostic = error.to_string();
    std::process::abort()
}

fn read_once(
    handle: HANDLE,
    buffer: &mut [u8],
    timeout: Duration,
    operation: &'static str,
) -> IpcResult<usize> {
    let event = create_event()?;
    let mut overlapped = OVERLAPPED {
        hEvent: event.raw(),
        ..Default::default()
    };
    let mut transferred = 0_u32;
    // SAFETY: `buffer` is writable for its full length and `overlapped` is
    // kept alive until the operation completes or is cancelled.
    let start = unsafe {
        ReadFile(
            handle,
            Some(buffer),
            Some(&raw mut transferred),
            Some(&raw mut overlapped),
        )
    };
    match start {
        Ok(()) => transfer_count(transferred),
        Err(failure) if win32_code(&failure) == ERROR_IO_PENDING.0 => {
            wait_overlapped(handle, &overlapped, event.raw(), timeout, operation)?;
            // SAFETY: the overlapped operation is complete.
            unsafe {
                GetOverlappedResult(handle, &raw const overlapped, &raw mut transferred, true)
            }
            .map_err(|failure| transport_failure("GetOverlappedResult", &failure))?;
            transfer_count(transferred)
        }
        Err(failure) if is_disconnect_code(win32_code(&failure)) => Err(IpcError::Disconnected {
            message: format!("ReadFile: {failure}"),
        }),
        Err(failure) => Err(transport_failure("ReadFile", &failure)),
    }
}

fn write_once(
    handle: HANDLE,
    buffer: &[u8],
    timeout: Duration,
    operation: &'static str,
) -> IpcResult<usize> {
    let event = create_event()?;
    let mut overlapped = OVERLAPPED {
        hEvent: event.raw(),
        ..Default::default()
    };
    let mut transferred = 0_u32;
    // SAFETY: `buffer` is readable for its full length and `overlapped` remains
    // alive until the operation completes or is cancelled.
    let start = unsafe {
        WriteFile(
            handle,
            Some(buffer),
            Some(&raw mut transferred),
            Some(&raw mut overlapped),
        )
    };
    match start {
        Ok(()) => transfer_count(transferred),
        Err(failure) if win32_code(&failure) == ERROR_IO_PENDING.0 => {
            wait_overlapped(handle, &overlapped, event.raw(), timeout, operation)?;
            // SAFETY: the overlapped operation is complete.
            unsafe {
                GetOverlappedResult(handle, &raw const overlapped, &raw mut transferred, true)
            }
            .map_err(|failure| transport_failure("GetOverlappedResult", &failure))?;
            transfer_count(transferred)
        }
        Err(failure) if is_disconnect_code(win32_code(&failure)) => Err(IpcError::Disconnected {
            message: format!("WriteFile: {failure}"),
        }),
        Err(failure) => Err(transport_failure("WriteFile", &failure)),
    }
}

fn cancel_and_reap(handle: HANDLE, overlapped: &OVERLAPPED, event: HANDLE) {
    // SAFETY: the pointer refers to the still-live overlapped operation.
    match unsafe { CancelIoEx(handle, Some(overlapped)) } {
        Ok(()) | Err(_) => {
            // The operation may have completed between timeout and cancellation.
        }
    }
    // SAFETY: `event` belongs to this overlapped operation.
    if unsafe { WaitForSingleObject(event, 5_000) } != WAIT_OBJECT_0 {
        // Returning with an in-flight overlapped buffer would be use-after-free.
        std::process::abort();
    }
    let mut transferred = 0_u32;
    // SAFETY: the event is signalled, so reaping cannot block indefinitely.
    if let Err(failure) =
        unsafe { GetOverlappedResult(handle, overlapped, &raw mut transferred, true) }
        && win32_code(&failure) != ERROR_OPERATION_ABORTED.0
        && !is_disconnect_code(win32_code(&failure))
    {
        // The operation is complete, but an unexpected status means the
        // caller must not continue with an unknown transport state.
        std::process::abort();
    }
}

fn transfer_count(count: u32) -> IpcResult<usize> {
    usize::try_from(count).map_err(|_| IpcError::Transport {
        message: format!("transfer count {count} does not fit usize"),
    })
}

fn duration_millis(timeout: Duration) -> u32 {
    u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX)
}

fn transport_error_code(operation: &str, code: u32) -> IpcError {
    if is_disconnect_code(code) {
        IpcError::Disconnected {
            message: format!("{operation} failed with Win32 error {code}"),
        }
    } else if code == windows::Win32::Foundation::WAIT_TIMEOUT.0 {
        IpcError::timeout("named-pipe", Duration::ZERO)
    } else if code == 5 {
        IpcError::PeerIdentityUnavailable {
            message: format!("{operation} was denied by the OS (Win32 error 5)"),
        }
    } else {
        IpcError::Transport {
            message: format!("{operation} failed with Win32 error {code}"),
        }
    }
}

const fn is_disconnect_code(code: u32) -> bool {
    code == ERROR_BROKEN_PIPE.0 || code == windows::Win32::Foundation::ERROR_NO_DATA.0
}

#[cfg(test)]
mod tests {
    use super::{duration_millis, win32_code};

    #[test]
    fn test_duration_millis_saturates() {
        assert_eq!(duration_millis(std::time::Duration::from_millis(42)), 42);
    }

    #[test]
    fn test_win32_code_decodes_hresult_from_win32() {
        let hresult = windows::core::HRESULT::from_win32(109);
        let failure = windows::core::Error::from_hresult(hresult);
        assert_eq!(win32_code(&failure), 109);
    }
}
