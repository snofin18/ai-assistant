//! Contract and process-tree tests for the dedicated browser launcher.
//!
//! The helper test is intentionally ignored during normal runs and is spawned
//! only by the cleanup test. No test starts a real browser or opens a profile.

use super::*;

const HELPER_MODE_ENV: &str = "ASSISTANT_BROWSER_LAUNCHER_HELPER_MODE";
const HELPER_PID_FILE_ENV: &str = "ASSISTANT_BROWSER_LAUNCHER_HELPER_PID_FILE";
const HELPER_ROOT: &str = "root";
const HELPER_LEAF: &str = "leaf";

#[test]
fn test_fixed_arguments_pin_profile_and_dynamic_cdp_port() -> PlatformResult<()> {
    let profile = BrowserProfile::new(
        "launcher-test",
        BrowserKind::Edge,
        r"C:\assistant\profiles\launcher-test",
    )
    .map_err(|error| invalid_args(error.to_string()))?;
    let arguments = fixed_launch_arguments(&profile);

    assert!(
        arguments
            .iter()
            .any(|argument| { argument == r"--user-data-dir=C:\assistant\profiles\launcher-test" })
    );
    assert!(
        arguments
            .iter()
            .any(|argument| argument == "--remote-debugging-port=0")
    );
    assert_eq!(arguments.last().map(String::as_str), Some("about:blank"));
    assert!(!arguments.iter().any(|argument| argument.contains("origin")));
    Ok(())
}

#[test]
fn test_active_port_parser_accepts_complete_endpoint() -> PlatformResult<()> {
    let endpoint =
        parse_active_port(b"9222\n/devtools/browser/0f5f8f1e-41f2-4a54-9c28-f20f187b789a\n")?
            .ok_or_else(|| invalid_ready_file("expected a complete endpoint"))?;

    assert_eq!(endpoint.port, 9222);
    assert_eq!(
        endpoint.browser_path,
        "/devtools/browser/0f5f8f1e-41f2-4a54-9c28-f20f187b789a"
    );
    Ok(())
}

#[test]
fn test_active_port_parser_waits_for_partial_file() -> PlatformResult<()> {
    assert_eq!(parse_active_port(b"")?, None);
    assert_eq!(parse_active_port(b"9222\n")?, None);
    Ok(())
}

#[test]
fn test_active_port_parser_rejects_invalid_or_oversized_contents() {
    for contents in [
        b"0\n/devtools/browser/id".as_slice(),
        b"9222\nhttp://example.test/id".as_slice(),
        b"9222\n/devtools/browser/id\nextra".as_slice(),
        b"not-a-port\n/devtools/browser/id".as_slice(),
    ] {
        assert!(parse_active_port(contents).is_err());
    }
    assert!(parse_active_port(&vec![b'a'; MAX_ACTIVE_PORT_BYTES + 1]).is_err());
}

#[test]
fn test_user_profile_tree_is_rejected_without_touching_it() -> PlatformResult<()> {
    let Some(base) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
        return Ok(());
    };
    let edge_user_data = base
        .join("Microsoft")
        .join("Edge")
        .join("User Data")
        .join("Assistant");
    let error = match reject_user_profile_tree(&edge_user_data) {
        Ok(()) => return Err(invalid_args("user profile tree must be rejected")),
        Err(error) => error,
    };
    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    Ok(())
}

#[test]
fn test_launcher_rejects_relative_profile_root() -> PlatformResult<()> {
    let Err(error) = BrowserLauncher::new(
        std::env::current_exe().unwrap_or_default(),
        "relative-profile",
        "launcher-test",
        BrowserKind::Edge,
    ) else {
        return Err(invalid_args("relative root must be rejected"));
    };
    assert_eq!(error.code(), ErrorCode::ToolInvalidArgs);
    Ok(())
}

#[test]
fn test_process_tree_termination_reaps_root_and_child() -> PlatformResult<()> {
    let temp_root = std::env::temp_dir().join(format!(
        "assistant-browser-launcher-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| {
                PlatformError::new(ErrorCode::Fatal, format!("read system clock: {error}"))
            })?
            .as_nanos()
    ));
    fs::create_dir_all(&temp_root).map_err(|error| process_error("create test root", &error))?;
    let pid_file = temp_root.join("pids.txt");
    let executable = std::env::current_exe()
        .map_err(|error| process_error("resolve current test executable", &error))?;
    let mut child = Command::new(executable)
        .args([
            "--ignored",
            "--exact",
            "browser_launcher::tests::helper_process_tree",
            "--nocapture",
        ])
        .env(HELPER_MODE_ENV, HELPER_ROOT)
        .env(HELPER_PID_FILE_ENV, &pid_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| process_error("spawn helper process tree", &error))?;

    let pids = wait_for_pid_file(&pid_file)?;
    terminate_child_tree(&mut child, TERMINATE_TIMEOUT)?;
    wait_until_process_gone(pids.0)?;
    wait_until_process_gone(pids.1)?;

    fs::remove_dir_all(&temp_root).map_err(|error| process_error("remove test root", &error))?;
    Ok(())
}

#[test]
#[ignore = "TASK-110: spawned only by the process-tree cleanup test"]
fn helper_process_tree() {
    match std::env::var(HELPER_MODE_ENV).as_deref() {
        Ok(HELPER_LEAF) => thread::sleep(Duration::from_secs(60)),
        Ok(HELPER_ROOT) => {
            let Ok(executable) = std::env::current_exe() else {
                return;
            };
            let Ok(mut leaf) = Command::new(executable)
                .args([
                    "--ignored",
                    "--exact",
                    "browser_launcher::tests::helper_process_tree",
                    "--nocapture",
                ])
                .env(HELPER_MODE_ENV, HELPER_LEAF)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            else {
                return;
            };
            let Some(pid_path) = std::env::var_os(HELPER_PID_FILE_ENV) else {
                return;
            };
            let pid_file = PathBuf::from(pid_path);
            if fs::write(pid_file, format!("{} {}", std::process::id(), leaf.id())).is_err() {
                return;
            }
            if leaf.wait().is_err() {
                std::process::exit(2);
            }
        }
        _ => {}
    }
}

fn wait_for_pid_file(path: &Path) -> PlatformResult<(u32, u32)> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(contents) = fs::read_to_string(path) {
            let mut values = contents.split_whitespace();
            if let (Some(root), Some(leaf)) = (values.next(), values.next())
                && let (Ok(root), Ok(leaf)) = (root.parse::<u32>(), leaf.parse::<u32>())
            {
                return Ok((root, leaf));
            }
        }
        if Instant::now() >= deadline {
            return Err(PlatformError::new(
                ErrorCode::TargetUnresponsive,
                "timed out waiting for helper process ids",
            ));
        }
        thread::sleep(PROCESS_POLL_INTERVAL);
    }
}

fn test_process_exists(process_id: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    // SAFETY: OpenProcess only queries existence and does not transfer memory.
    unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }.is_ok_and(
        |handle| {
            let mut exit_code = 0_u32;
            // SAFETY: the process handle is valid and exit_code points to writable storage.
            let is_running = unsafe { GetExitCodeProcess(handle, &raw mut exit_code) }.is_ok()
                && exit_code == STILL_ACTIVE.0 as u32;
            // SAFETY: handle was returned by OpenProcess and is closed exactly once.
            let _close_outcome = unsafe { CloseHandle(handle) };
            is_running
        },
    )
}

fn wait_until_process_gone(process_id: u32) -> PlatformResult<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if !test_process_exists(process_id) {
            return Ok(());
        }
        thread::sleep(PROCESS_POLL_INTERVAL);
    }
    Err(PlatformError::new(
        ErrorCode::TargetUnresponsive,
        format!("process {process_id} remained visible after tree termination"),
    ))
}
