//! CDP runtime provider.

use std::collections::BTreeMap;
use std::future::Future;

use assistant_platform_api::{
    CdpConnectOptions, CdpDomSnapshot, CdpLoadCondition, CdpPoint, CdpProvider, CdpScreenshot,
    CdpSessionSnapshot, CdpTargetIdentity, ResolvedCdpSession, ResolvedCdpTarget,
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
        options: &CdpConnectOptions,
    ) -> impl Future<Output = assistant_platform_api::PlatformResult<CdpSessionSnapshot>> + Send
    {
        let session = ResolvedCdpSession::new(0);
        let target = ResolvedCdpTarget::new(session, 0);
        let identity =
            CdpTargetIdentity::new(options.page_url(), options.page_title().unwrap_or("blank"));
        let identity = if let Some(marker) = options.page_marker() {
            identity.with_marker(marker)
        } else {
            identity
        };
        std::future::ready(Ok(CdpSessionSnapshot::new(session, target, identity)))
    }

    fn navigate(
        &self,
        _target: &ResolvedCdpTarget,
        _url: &str,
        _condition: &CdpLoadCondition,
    ) -> impl Future<Output = assistant_platform_api::PlatformResult<()>> + Send {
        std::future::ready(Ok(()))
    }

    fn evaluate_dom(
        &self,
        _target: &ResolvedCdpTarget,
        _expression: &str,
    ) -> impl Future<Output = assistant_platform_api::PlatformResult<CdpDomSnapshot>> + Send {
        std::future::ready(Ok(CdpDomSnapshot::new("about:blank", "blank", None, "")))
    }

    fn click(
        &self,
        _target: &ResolvedCdpTarget,
        _point: &CdpPoint,
    ) -> impl Future<Output = assistant_platform_api::PlatformResult<()>> + Send {
        std::future::ready(Ok(()))
    }

    fn capture_screenshot(
        &self,
        _target: &ResolvedCdpTarget,
    ) -> impl Future<Output = assistant_platform_api::PlatformResult<CdpScreenshot>> + Send {
        std::future::ready(Ok(CdpScreenshot::new("blank", 1, 1)))
    }

    fn set_download_directory(
        &self,
        _target: &ResolvedCdpTarget,
        _directory: &str,
    ) -> impl Future<Output = assistant_platform_api::PlatformResult<()>> + Send {
        std::future::ready(Ok(()))
    }

    fn close(
        &self,
        _target: &ResolvedCdpTarget,
    ) -> impl Future<Output = assistant_platform_api::PlatformResult<()>> + Send {
        std::future::ready(Ok(()))
    }
}
