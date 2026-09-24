//! Windows end-to-end acceptance for the real host process.

#![cfg(windows)]

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use assistant_ipc::{
    Heartbeat, IpcError, NamedPipeTransport, Transport, WireMessage, client_handshake,
};

const TOKEN_ENVIRONMENT_VARIABLE: &str = "ASSISTANT_TASK_019_TOKEN";
const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct HostProcess {
    child: Child,
}

impl HostProcess {
    fn spawn(pipe_name: &str, allowed_peer: &Path, status_file: &Path) -> std::io::Result<Self> {
        let child = Command::new(env!("CARGO_BIN_EXE_assistant-automation-host"))
            .arg("--pipe-name")
            .arg(pipe_name)
            .arg("--token-env")
            .arg(TOKEN_ENVIRONMENT_VARIABLE)
            .arg("--allow-peer")
            .arg(allowed_peer)
            .arg("--status-file")
            .arg(status_file)
            .arg("--handshake-timeout-ms")
            .arg("5000")
            .arg("--heartbeat-timeout-ms")
            .arg("2000")
            .env(TOKEN_ENVIRONMENT_VARIABLE, TOKEN)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(Self { child })
    }

    fn kill_and_wait(&mut self) -> std::io::Result<()> {
        match self.child.kill() {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {}
            Err(error) => return Err(error),
        }
        self.child.wait().map(|_status| ())
    }
}

impl Drop for HostProcess {
    fn drop(&mut self) {
        match self.child.kill() {
            Ok(()) => {
                if let Err(error) = self.child.wait() {
                    drop(error);
                }
            }
            Err(error) => drop(error),
        }
    }
}

#[test]
fn test_kill_host_client_detects_disconnect() -> Result<(), Box<dyn std::error::Error>> {
    let unique = unique_suffix()?;
    let pipe_name = format!("assistant-task-019-{unique}");
    let temp_directory = std::env::temp_dir();
    let status_file = temp_directory.join(format!("assistant-task-019-{unique}.status"));
    let peer_image = std::env::current_exe()?;
    let mut host = HostProcess::spawn(&pipe_name, &peer_image, &status_file)?;

    let mut transport = NamedPipeTransport::client(&pipe_name)?;
    transport.connect(Duration::from_secs(5))?;
    let server_hello = client_handshake(&mut transport, TOKEN, Vec::new(), Duration::from_secs(5))?;
    assert_eq!(server_hello.version, assistant_ipc::IPC_PROTOCOL_VERSION);

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let timestamp_unix_ms = u64::try_from(now)?;
    transport.send(
        &WireMessage::Heartbeat(Heartbeat {
            session_id: server_hello.session_id,
            timestamp_unix_ms,
            alive: true,
        }),
        Duration::from_secs(2),
    )?;
    let heartbeat = transport.recv(Duration::from_secs(2))?;
    assert!(matches!(heartbeat, WireMessage::Heartbeat(_)));

    host.kill_and_wait()?;
    let failure = transport.recv(Duration::from_secs(2));
    assert!(
        matches!(
            failure,
            Err(IpcError::Disconnected { .. } | IpcError::Timeout { .. })
        ),
        "expected disconnect or timeout after host kill"
    );
    remove_status_file(&status_file)?;
    Ok(())
}

fn unique_suffix() -> Result<String, Box<dyn std::error::Error>> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(format!("{}-{nanos}", std::process::id()))
}

fn remove_status_file(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if let Err(error) = std::fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        return Err(error.into());
    }
    Ok(())
}
