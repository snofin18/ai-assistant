//! CDP runtime provider.
//!
//! The real HTTP/WebSocket transport and production handler assembly are not part
//! of this module yet. Until they exist, every trait operation fails closed with
//! `ErrorCode::CapabilityMissing`; returning synthetic page data would turn an
//! unimplemented browser path into a false success.

use std::collections::BTreeMap;
use std::future::Future;

use assistant_platform_api::{
    CdpConnectOptions, CdpDomSnapshot, CdpLoadCondition, CdpPoint, CdpProvider, CdpScreenshot,
    CdpSessionSnapshot, CdpTargetIdentity, ErrorCode, PlatformError, PlatformResult,
    ResolvedCdpTarget,
};

/// Runtime provider state for one CDP target.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CdpRuntimeTarget {
    id: u64,
    session_id: u64,
    identity: CdpTargetIdentity,
}

/// CDP provider backed by the browser runtime.
#[derive(Debug, Default)]
pub struct RuntimeCdpProvider {
    targets: BTreeMap<u64, CdpRuntimeTarget>,
}

impl RuntimeCdpProvider {
    /// 创建一个空 provider。
    #[must_use]
    pub const fn new() -> Self {
        Self {
            targets: BTreeMap::new(),
        }
    }

    /// 当前 targets。
    #[must_use]
    pub const fn targets(&self) -> &BTreeMap<u64, CdpRuntimeTarget> {
        &self.targets
    }

    /// 读取一个 target。
    #[must_use]
    pub fn target(&self, target_id: u64) -> Option<&CdpRuntimeTarget> {
        self.targets.get(&target_id)
    }

    /// 打开一个 target。
    pub fn open_target(
        &mut self,
        session_id: u64,
        target_id: u64,
        identity: CdpTargetIdentity,
    ) -> Option<&CdpRuntimeTarget> {
        self.targets.insert(
            target_id,
            CdpRuntimeTarget {
                id: target_id,
                session_id,
                identity,
            },
        );
        self.targets.get(&target_id)
    }

    /// 关闭一个 target。
    pub fn close_target(&mut self, target_id: u64) -> bool {
        self.targets.remove(&target_id).is_some()
    }
}

impl CdpProvider for RuntimeCdpProvider {
    fn connect(
        &self,
        _options: &CdpConnectOptions,
    ) -> impl Future<Output = PlatformResult<CdpSessionSnapshot>> + Send {
        std::future::ready(Err(capability_missing("connect")))
    }

    fn navigate(
        &self,
        _target: &ResolvedCdpTarget,
        _url: &str,
        _condition: &CdpLoadCondition,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        std::future::ready(Err(capability_missing("navigate")))
    }

    fn evaluate_dom(
        &self,
        _target: &ResolvedCdpTarget,
        _expression: &str,
    ) -> impl Future<Output = PlatformResult<CdpDomSnapshot>> + Send {
        std::future::ready(Err(capability_missing("evaluate_dom")))
    }

    fn click(
        &self,
        _target: &ResolvedCdpTarget,
        _point: &CdpPoint,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        std::future::ready(Err(capability_missing("click")))
    }

    fn capture_screenshot(
        &self,
        _target: &ResolvedCdpTarget,
    ) -> impl Future<Output = PlatformResult<CdpScreenshot>> + Send {
        std::future::ready(Err(capability_missing("capture_screenshot")))
    }

    fn set_download_directory(
        &self,
        _target: &ResolvedCdpTarget,
        _directory: &str,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        std::future::ready(Err(capability_missing("set_download_directory")))
    }

    fn close(
        &self,
        _target: &ResolvedCdpTarget,
    ) -> impl Future<Output = PlatformResult<()>> + Send {
        std::future::ready(Err(capability_missing("close")))
    }
}

fn capability_missing(operation: &str) -> PlatformError {
    PlatformError::new(
        ErrorCode::CapabilityMissing,
        format!(
            "real CDP {operation} is unavailable until HTTP/WebSocket transport and production \
             browser handlers are assembled"
        ),
    )
}
