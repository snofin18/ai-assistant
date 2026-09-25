//! 工具集治理：指纹、> 40 挂载告警、两个元工具（TASK-020 交付物 7~8 / 不变量 4~5）。
//!
//! 这些断言都走**同一个公共入口**（`ToolBus::start` → 挂载报告 / `tools/call`），因此它们
//! 证明的是「模型真的看到的东西」，而不是本 crate 内部数据结构的自述。
//!
//! `unwrap` / `expect` / `panic` 的例外由本文件顶部一行显式放开（AGENTS.md §5.3）。

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

mod common;

use assistant_protocol::ErrorCode;
use assistant_tool_bus::{CallContext, MountSelection, ToolBus, ToolBusConfig, ToolRegistry};
use common::{
    arguments, assert_rejected, clock, demo_definition, echo_definition, echo_handler,
    listed_names, mount_fingerprint, payload, register,
};
use serde_json::{Value, json};

/// 不变量 4：同集合不同插入顺序 → 同指纹；描述变化 / 增删工具 → 指纹变化。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_toolset_fingerprint_is_order_independent_and_change_sensitive() {
    let mut forward = ToolRegistry::new();
    register(
        &mut forward,
        demo_definition("alpha", "第一个"),
        echo_handler(),
    );
    register(
        &mut forward,
        demo_definition("beta", "第二个"),
        echo_handler(),
    );
    let forward = mount_fingerprint(forward).await;

    // 反向插入：指纹必须逐字节相同（顺序无关）。
    let mut backward = ToolRegistry::new();
    register(
        &mut backward,
        demo_definition("beta", "第二个"),
        echo_handler(),
    );
    register(
        &mut backward,
        demo_definition("alpha", "第一个"),
        echo_handler(),
    );
    let backward = mount_fingerprint(backward).await;
    assert_eq!(forward, backward);
    assert_eq!(forward.as_str().len(), 64, "SHA-256 hex must be 64 chars");

    // 只改一个工具的描述：模型看到的东西变了 → 指纹必须变。
    let mut reworded = ToolRegistry::new();
    register(
        &mut reworded,
        demo_definition("alpha", "第一个（改过）"),
        echo_handler(),
    );
    register(
        &mut reworded,
        demo_definition("beta", "第二个"),
        echo_handler(),
    );
    let reworded = mount_fingerprint(reworded).await;
    assert_ne!(forward, reworded);

    // 多挂一个工具：集合变了 → 指纹必须变。
    let mut extended = ToolRegistry::new();
    register(
        &mut extended,
        demo_definition("alpha", "第一个"),
        echo_handler(),
    );
    register(
        &mut extended,
        demo_definition("beta", "第二个"),
        echo_handler(),
    );
    register(
        &mut extended,
        demo_definition("gamma", "第三个"),
        echo_handler(),
    );
    let extended = mount_fingerprint(extended).await;
    assert_ne!(forward, extended);

    // 同一注册表重复挂载：指纹必须稳定（幂等，不掺时间 / 随机）。
    let mut repeated = ToolRegistry::new();
    register(
        &mut repeated,
        demo_definition("alpha", "第一个"),
        echo_handler(),
    );
    register(
        &mut repeated,
        demo_definition("beta", "第二个"),
        echo_handler(),
    );
    assert_eq!(forward, mount_fingerprint(repeated).await);
}

/// 不变量 5 / 铁律 1：挂载 > 40 个工具必须留下**结构化**告警 + 审计事件（不是一行日志）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_mounting_more_than_forty_tools_warns() {
    let mut registry = ToolRegistry::new();
    for index in 0..41 {
        register(
            &mut registry,
            demo_definition(&format!("tool{index:02}"), "批量工具（测试用）"),
            echo_handler(),
        );
    }
    assert_eq!(registry.len(), 41);

    let (bus, report) = ToolBus::start(
        registry,
        MountSelection::all(),
        ToolBusConfig::new("session-oversize"),
        clock(),
    )
    .await
    .expect("in-process bus must start");

    // 计数口径 = 最终挂载集（41 个工具 + 2 个元工具）。
    assert_eq!(report.mounted.len(), 43);
    let warning = report
        .warning
        .expect("mounting 43 tools must raise a structured warning");
    assert_eq!(warning.mounted_count, 43);
    assert_eq!(warning.limit, 40);

    let audit_event = report
        .audit_event
        .as_ref()
        .expect("the oversize warning must also produce an audit event");
    assert_eq!(audit_event.event_type, "incident.reported");
    assert_eq!(audit_event.session_id, "session-oversize");
    let reason = audit_event
        .args
        .as_ref()
        .and_then(|args| args.get("reason"))
        .and_then(Value::as_str);
    assert_eq!(reason, Some("toolset_oversize"));

    // 告警不影响可用性：超限的工具集仍然能正常列出来。
    let exposed = bus
        .list_tool_names()
        .await
        .expect("tools/list must succeed");
    assert_eq!(exposed.len(), 43);

    bus.shutdown().await.expect("bus must shut down");
}

/// 交付物 8：两个元工具出现在 `tools/list`，`toolset.search` 的 `query` 必填（有负向用例）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_meta_tools_list_and_search_the_current_toolset() {
    let mut registry = ToolRegistry::new();
    register(
        &mut registry,
        echo_definition("把入参原样返回"),
        echo_handler(),
    );

    let (bus, _report) = ToolBus::start(
        registry,
        MountSelection::all(),
        ToolBusConfig::new("session-meta"),
        clock(),
    )
    .await
    .expect("in-process bus must start");

    let call = CallContext::new("task-1", "step-1");

    // toolset.list：不带过滤 → 全部 3 条（含元工具自己）。
    let listed = bus
        .call_tool("toolset.list", arguments(json!({})), &call)
        .await
        .expect("toolset.list must return an envelope");
    assert!(listed.ok);
    assert_eq!(payload(&listed).get("count"), Some(&json!(3)));
    let mut names = listed_names(payload(&listed));
    names.sort();
    assert_eq!(
        names,
        vec![
            "demo.echo.echo".to_owned(),
            "toolset.list".to_owned(),
            "toolset.search".to_owned(),
        ]
    );

    // toolset.list + app_id 过滤：只回该应用的工具。
    let filtered = bus
        .call_tool(
            "toolset.list",
            arguments(json!({ "app_id": "toolset" })),
            &call,
        )
        .await
        .expect("toolset.list must return an envelope");
    assert_eq!(payload(&filtered).get("count"), Some(&json!(2)));

    // toolset.search：命中 `echo` 的只有那一个工具。
    let found = bus
        .call_tool(
            "toolset.search",
            arguments(json!({ "query": "echo" })),
            &call,
        )
        .await
        .expect("toolset.search must return an envelope");
    assert!(found.ok);
    assert_eq!(payload(&found).get("count"), Some(&json!(1)));
    assert_eq!(
        listed_names(payload(&found)),
        vec!["demo.echo.echo".to_owned()]
    );

    // 负向：缺必填 `query` → 直接拒（不进 handler）。
    let rejected = bus
        .call_tool("toolset.search", arguments(json!({})), &call)
        .await
        .expect("a rejected call is still an envelope");
    assert_rejected(&rejected, "missing required property `query`");

    // 未挂载的工具名 → `TargetNotFound` 信封（可读原因，模型能自我纠正）。
    let missing = bus
        .call_tool("demo.echo.nope", arguments(json!({})), &call)
        .await
        .expect("an unmounted tool is still an envelope");
    assert!(!missing.ok);
    assert_eq!(
        missing.error.as_ref().map(|error| error.code),
        Some(ErrorCode::TargetNotFound)
    );

    bus.shutdown().await.expect("bus must shut down");
}
