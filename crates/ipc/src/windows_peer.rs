//! Windows peer process identity and cryptographic random helpers.

use windows::Win32::Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::core::PWSTR;

use crate::windows_io::{OwnedHandle, transport_failure};
use crate::{IpcError, IpcResult};

/// Fill a byte slice from the OS cryptographic random source.
///
/// # Errors
/// Returns [`IpcError::Transport`] when `BCryptGenRandom` reports failure.
pub fn fill_random(bytes: &mut [u8]) -> IpcResult<()> {
    // SAFETY: `BCryptGenRandom` writes exactly bytes.len() bytes into `bytes`,
    // and the null algorithm handle selects the system-preferred RNG.
    unsafe { BCryptGenRandom(None, bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }
        .ok()
        .map_err(|failure| IpcError::Transport {
            message: format!("BCryptGenRandom failed: {failure}"),
        })
}

/// Resolve a process image path through Win32.
///
/// # Errors
/// Fails closed when the process cannot be opened or queried.
pub fn process_image_path(process_id: u32) -> IpcResult<String> {
    // SAFETY: the process id came from the connected NamedPipe. The returned
    // handle is immediately wrapped and closed by `OwnedHandle`.
    let raw_process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }
        .map_err(|failure| IpcError::PeerIdentityUnavailable {
            message: transport_failure("OpenProcess", &failure).to_string(),
        })?;
    let process = OwnedHandle::new(raw_process);
    let mut buffer = vec![0_u16; 32_768].into_boxed_slice();
    let mut length = u32::try_from(buffer.len()).unwrap_or(u32::MAX);
    // SAFETY: `buffer` is writable for `length` UTF-16 code units, and
    // `process` owns a live query-limited process handle.
    unsafe {
        QueryFullProcessImageNameW(
            process.raw(),
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &raw mut length,
        )
    }
    .map_err(|failure| IpcError::PeerIdentityUnavailable {
        message: transport_failure("QueryFullProcessImageNameW", &failure).to_string(),
    })?;
    let used = usize::try_from(length).map_err(|_| IpcError::PeerIdentityUnavailable {
        message: "process image path length overflow".to_string(),
    })?;
    let path_units = buffer
        .get(..used)
        .ok_or_else(|| IpcError::PeerIdentityUnavailable {
            message: "process image path length exceeded buffer".to_string(),
        })?;
    Ok(String::from_utf16_lossy(path_units))
}
