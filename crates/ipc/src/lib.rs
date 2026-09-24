//! # assistant-ipc
//!
//! Host IPC 传输层：帧编解码、认证握手、心跳状态机与平台传输。
//!
//! ## 职责
//!
//! - 把 `docs/spec/ipc-protocol.md` §3 / §4 的线上格式落成可测试 Rust API。
//! - 只在协议边界接受 `assistant_protocol` 的可序列化类型。
//! - Windows 实现 NamedPipe；其它平台返回明确的 `CapabilityMissing`。
//!
//! ## 边界（不做什么）
//!
//! - 不做工具语义、策略判定、元素定位或真实应用自动化。
//! - 不复制 `docs/spec/envelope.md` 的 `Header` / `Payload` 类型。
//! - 不做加密、压缩、分片、远端管道或会话恢复。
//!
//! ## 不变量
//!
//! 1. 所有失败返回 [`IpcError`]，并映射到 `assistant_protocol::ErrorCategory`。
//! 2. `magic` / CRC32 / 16 MiB 上限 / 协议版本 / token 全部在进入消息流前校验。
//! 3. 内核句柄仅存在于 Windows 传输实现内，绝不进入消息载荷。
//! 4. 超时不会返回空消息或伪造心跳；它返回带错误码的失败。
//!
//! ## 典型用法
//!
//! ```no_run
//! use std::time::Duration;
//!
//! use assistant_ipc::{NamedPipeTransport, Transport, client_handshake};
//!
//! # fn main() -> Result<(), assistant_ipc::IpcError> {
//! let mut transport = NamedPipeTransport::client("assistant-task-019")?;
//! transport.connect(Duration::from_secs(5))?;
//! let hello = client_handshake(&mut transport, "a-secret", Vec::new(), Duration::from_secs(5))?;
//! assert_eq!(hello.version, assistant_ipc::IPC_PROTOCOL_VERSION);
//! # Ok(())
//! # }
//! ```

#![deny(unsafe_code)]

mod error;
mod frame;
mod handshake;
mod heartbeat;
mod transport;

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows_io;
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows_peer;
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows_transport;

pub use error::{IpcError, IpcResult};
pub use frame::{FRAME_MAGIC, MAX_ENVELOPE_SIZE, crc32, decode_frame, encode_frame};
pub use handshake::{
    AuthenticatedClientHello, ClientHello, HandshakeResult, Heartbeat, IPC_PROTOCOL_VERSION,
    RequestMessage, ResponseMessage, ServerHello, SessionId, WireMessage, client_handshake,
    constant_time_eq, generate_session_id, server_handshake, validate_authentication_token,
};
pub use heartbeat::HeartbeatMonitor;
pub use transport::{NamedPipeTransport, Transport, peer_process_image_path};
