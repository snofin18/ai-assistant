//! CDP provider contract 的公开 API 契约测试。
//!
//! 边界：只构造纯值对象并调用公开访问器；不启动浏览器、不连接 CDP、不执行
//! HTTP / WebSocket / 文件系统 IO。平台 provider 的真实传输由 Windows adapter
//! 与后续靶机卡覆盖。
//!
//! 不变量：
//!   1. normalised CDP 点使用左闭右开的页面矩形；
//!   2. 非正矩形与越界点必须显式返回 `ToolInvalidArgs`；
//!   3. 可序列化 contract 必须同时支持 `Serialize` 与反序列化；
//!   4. opaque session / target 句柄保持不可序列化。
//!
//! 相关：架构 v2 §13.2.7、TASK-048、`docs/spec/testing.md` §4.4。

mod common;

use assistant_platform_api::{
    CdpConnectOptions, CdpDomSnapshot, CdpLoadCondition, CdpPoint, CdpScreenshot,
    CdpSessionSnapshot, CdpTargetIdentity, ErrorCode, ResolvedCdpSession, ResolvedCdpTarget,
};
use common::ok_or_fail;
use serde::{Serialize, de::DeserializeOwned};

fn assert_serde_contract<T: Serialize + DeserializeOwned + 'static>() {
    assert_ne!(
        std::any::TypeId::of::<T>(),
        std::any::TypeId::of::<()>(),
        "CDP serializable contracts must not be unit",
    );
}

#[test]
fn test_cdp_point_bounds_use_closed_open_rectangle() {
    let point = CdpPoint::new(10, 20);
    assert_eq!(point.x(), 10);
    assert_eq!(point.y(), 20);
    assert!(point.check_in_bounds(10, 20, 11, 21).is_ok());

    let cases = [
        (10, 20, 10, 20),
        (10, 20, 11, 20),
        (10, 20, 10, 21),
        (0, 20, 10, 21),
        (10, 0, 11, 20),
        (11, 20, 10, 21),
        (10, 21, 11, 20),
    ];
    for (left, top, right, bottom) in cases {
        assert_eq!(
            point
                .check_in_bounds(left, top, right, bottom)
                .err()
                .map(|error| error.code()),
            Some(ErrorCode::ToolInvalidArgs)
        );
    }
}

#[test]
fn test_cdp_session_target_and_snapshot_accessors_preserve_identity() {
    let session = ResolvedCdpSession::new(7);
    assert_eq!(session.id(), 7);

    let target = ResolvedCdpTarget::new(session, 11);
    assert_eq!(target.session(), session);
    assert_eq!(target.target_id(), 11);

    let identity =
        CdpTargetIdentity::new("https://example.test/items", "Items").with_marker("data-task-327");
    assert_eq!(identity.marker(), Some("data-task-327"));

    let snapshot = CdpSessionSnapshot::new(session, target, identity.clone());
    assert_eq!(snapshot.session(), session);
    assert_eq!(snapshot.target(), target);
    assert_eq!(snapshot.target_identity(), &identity);

    let without_marker = CdpTargetIdentity::new("https://example.test/", "Home");
    assert_eq!(without_marker.marker(), None);
}

#[test]
fn test_cdp_dom_snapshot_preserves_target_identity_and_dom() {
    let snapshot = CdpDomSnapshot::new(
        "https://example.test/items",
        "Items",
        Some("data-task-327"),
        "<main>ready</main>",
    );
    assert_eq!(snapshot.target_url(), "https://example.test/items");
    assert_eq!(snapshot.target_title(), "Items");
    assert_eq!(snapshot.target_marker(), Some("data-task-327"));
    assert_eq!(snapshot.dom(), "<main>ready</main>");

    let without_marker =
        CdpDomSnapshot::new("https://example.test/", "Home", None, "<main>home</main>");
    assert_eq!(without_marker.target_marker(), None);
}

#[test]
fn test_cdp_load_condition_reports_stable_expected_state() {
    assert_eq!(CdpLoadCondition::Load.expected_state(), "load");
    assert_eq!(
        CdpLoadCondition::NetworkIdle { quiet_ms: 750 }.expected_state(),
        "network-idle"
    );
}

#[test]
fn test_cdp_screenshot_accessors_preserve_png_metadata() {
    let screenshot = CdpScreenshot::new("blob-sha256:abc", 1920, 1080);
    assert_eq!(screenshot.blob_id(), "blob-sha256:abc");
    assert_eq!(screenshot.width(), 1920);
    assert_eq!(screenshot.height(), 1080);
    assert_eq!(screenshot.format(), "png");
}

#[test]
fn test_cdp_connect_options_accept_complete_values_and_expose_them() {
    let options = ok_or_fail(
        CdpConnectOptions::new(
            "127.0.0.1",
            9222,
            "https://example.test/items",
            Some("Items".to_string()),
            Some("data-task-327".to_string()),
            5_000,
        ),
        "完整连接参数应被接受",
    );
    assert_eq!(options.host(), "127.0.0.1");
    assert_eq!(options.port(), 9222);
    assert_eq!(options.page_url(), "https://example.test/items");
    assert_eq!(options.page_title(), Some("Items"));
    assert_eq!(options.page_marker(), Some("data-task-327"));
    assert_eq!(options.session_timeout_ms(), 5_000);

    let minimal = ok_or_fail(
        CdpConnectOptions::new("localhost", 0, "about:blank", None, None, 0),
        "可选项省略和零超时应由 provider 继续解释",
    );
    assert_eq!(minimal.page_title(), None);
    assert_eq!(minimal.page_marker(), None);
}

#[test]
fn test_cdp_connect_options_reject_blank_host_or_page_url() {
    for (host, page_url) in [
        ("", "https://example.test/"),
        ("   ", "https://example.test/"),
        ("localhost", ""),
        ("localhost", "\t"),
    ] {
        assert_eq!(
            CdpConnectOptions::new(host, 9222, page_url, None, None, 1_000)
                .err()
                .map(|error| error.code()),
            Some(ErrorCode::ToolInvalidArgs),
            "host={host:?}, page_url={page_url:?}"
        );
    }
}

#[test]
fn test_cdp_serializable_contracts_keep_serde_bounds() {
    assert_serde_contract::<CdpPoint>();
    assert_serde_contract::<CdpTargetIdentity>();
    assert_serde_contract::<CdpDomSnapshot>();
    assert_serde_contract::<CdpLoadCondition>();
    assert_serde_contract::<CdpScreenshot>();
    assert_serde_contract::<CdpConnectOptions>();
}
