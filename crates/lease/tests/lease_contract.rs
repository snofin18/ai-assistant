//! Contract tests for target lease modes, TTL behavior, preemption, and deadlock avoidance.

use assistant_lease::{
    Lease, LeaseError, LeaseKey, LeaseManager, LeaseMode, LeaseOwner, LeaseRequest,
};
use assistant_protocol::ErrorCode;

fn key(app_id: &str) -> Result<LeaseKey, Box<dyn std::error::Error>> {
    Ok(LeaseKey::new(app_id)?)
}

fn owner(value: &str) -> Result<LeaseOwner, Box<dyn std::error::Error>> {
    Ok(LeaseOwner::parse(value)?)
}

fn expect_error<T, E>(
    result: Result<T, E>,
    message: &'static str,
) -> Result<E, Box<dyn std::error::Error>> {
    match result {
        Ok(_) => Err(message.into()),
        Err(error) => Ok(error),
    }
}

#[test]
fn test_lease_key_validation_rejects_empty_owner_and_nul() {
    assert_eq!(
        LeaseKey::new("").map_err(|error| error.error_code()),
        Err(ErrorCode::ToolInvalidArgs)
    );
    assert_eq!(
        LeaseKey::new("app\0suffix").map_err(|error| error.error_code()),
        Err(ErrorCode::ToolInvalidArgs)
    );
    assert_eq!(
        LeaseOwner::parse("").map_err(|error| error.error_code()),
        Err(ErrorCode::ToolInvalidArgs)
    );
    assert_eq!(
        LeaseKey::new("   ").map_err(|error| error.error_code()),
        Err(ErrorCode::ToolInvalidArgs)
    );
}

#[test]
fn test_lease_key_display_contains_all_components() -> Result<(), Box<dyn std::error::Error>> {
    let key = LeaseKey::new("com.example.app")?
        .with_window_id("window_1")?
        .with_document_id("doc_1")?
        .with_resource("selection")?;
    let rendered = key.to_string();
    assert_eq!(
        rendered,
        "app=com.example.app window=window_1 document=doc_1 resource=selection"
    );
    Ok(())
}

#[test]
fn test_mode_compatibility_matrix_is_conservative() {
    let modes = [LeaseMode::Shared, LeaseMode::Intent, LeaseMode::Exclusive];
    for requested in modes {
        for existing in modes {
            let expected = requested == LeaseMode::Exclusive || existing == LeaseMode::Exclusive;
            assert_eq!(
                requested.conflicts_with(existing),
                expected,
                "unexpected compatibility for {} vs {}",
                requested.as_str(),
                existing.as_str()
            );
        }
    }
    assert!(!LeaseMode::Shared.is_write());
    assert!(!LeaseMode::Intent.is_write());
    assert!(LeaseMode::Exclusive.is_write());
}

#[test]
fn test_shared_and_intent_leases_coexist_for_different_owners()
-> Result<(), Box<dyn std::error::Error>> {
    let target = key("app")?;
    let owner_a = owner("owner_a")?;
    let owner_b = owner("owner_b")?;
    let mut manager = LeaseManager::new();

    let shared = manager.acquire(target.clone(), LeaseMode::Shared, &owner_a, 1_000, 100)?;
    let intent = manager.acquire(target, LeaseMode::Intent, &owner_b, 1_000, 101)?;

    assert_eq!(shared.mode(), LeaseMode::Shared);
    assert_eq!(intent.mode(), LeaseMode::Intent);
    assert_eq!(manager.stored_lease_count(), 2);
    Ok(())
}

#[test]
fn test_intent_blocks_another_owners_exclusive_lease() -> Result<(), Box<dyn std::error::Error>> {
    let target = key("app")?;
    let owner_a = owner("owner_a")?;
    let owner_b = owner("owner_b")?;
    let mut manager = LeaseManager::new();
    let intent = manager.acquire(target.clone(), LeaseMode::Intent, &owner_a, 1_000, 100)?;

    let error = expect_error(
        manager.acquire(target.clone(), LeaseMode::Exclusive, &owner_b, 1_000, 101),
        "intent must reserve the target against another writer",
    )?;
    let LeaseError::Conflict(conflict) = error else {
        return Err("expected a lease conflict".into());
    };
    assert_eq!(conflict.key(), &target);
    assert_eq!(conflict.requested_mode(), LeaseMode::Exclusive);
    assert_eq!(conflict.requested_owner(), "owner_b");
    assert_eq!(conflict.blockers(), &[intent]);
    assert_eq!(manager.stored_lease_count(), 1);
    Ok(())
}

#[test]
fn test_exclusive_blocks_all_other_owners() -> Result<(), Box<dyn std::error::Error>> {
    let target = key("app")?;
    let owner_a = owner("owner_a")?;
    let owner_b = owner("owner_b")?;
    let mut manager = LeaseManager::new();
    let exclusive = manager.acquire(target.clone(), LeaseMode::Exclusive, &owner_a, 1_000, 100)?;

    for requested_mode in [LeaseMode::Shared, LeaseMode::Intent, LeaseMode::Exclusive] {
        let error = expect_error(
            manager.acquire(target.clone(), requested_mode, &owner_b, 1_000, 101),
            "exclusive must block every other owner",
        )?;
        let LeaseError::Conflict(conflict) = error else {
            return Err("expected a lease conflict".into());
        };
        assert_eq!(conflict.blockers(), std::slice::from_ref(&exclusive));
    }
    Ok(())
}

#[test]
fn test_same_owner_may_add_stronger_mode_without_self_conflict()
-> Result<(), Box<dyn std::error::Error>> {
    let target = key("app")?;
    let task = owner("task_1")?;
    let mut manager = LeaseManager::new();
    let shared = manager.acquire(target.clone(), LeaseMode::Shared, &task, 1_000, 100)?;
    let exclusive = manager.acquire(target, LeaseMode::Exclusive, &task, 1_000, 101)?;

    assert_eq!(shared.mode(), LeaseMode::Shared);
    assert_eq!(exclusive.mode(), LeaseMode::Exclusive);
    assert_eq!(manager.stored_lease_count(), 2);
    Ok(())
}

#[test]
fn test_ttl_boundary_expires_lease_and_frees_target() -> Result<(), Box<dyn std::error::Error>> {
    let target = key("app")?;
    let owner_a = owner("owner_a")?;
    let owner_b = owner("owner_b")?;
    let mut manager = LeaseManager::new();
    let lease = manager.acquire(target.clone(), LeaseMode::Exclusive, &owner_a, 100, 1_000)?;

    assert!(lease.is_active_at(1_099));
    assert!(!lease.is_active_at(1_100));
    let later = manager.acquire(target, LeaseMode::Exclusive, &owner_b, 100, 1_100)?;
    assert_eq!(later.owner(), &owner_b);
    assert_eq!(manager.stored_lease_count(), 1);
    Ok(())
}

#[test]
fn test_renew_extends_lease_and_rejects_expired_lease() -> Result<(), Box<dyn std::error::Error>> {
    let target = key("app")?;
    let task = owner("task_1")?;
    let mut manager = LeaseManager::new();
    let lease = manager.acquire(target, LeaseMode::Exclusive, &task, 100, 1_000)?;

    let renewed = manager.renew(lease.id(), &task, 200, 1_050)?;
    assert_eq!(renewed.acquired_at_ms(), 1_000);
    assert_eq!(renewed.renewed_at_ms(), 1_050);
    assert_eq!(renewed.expires_at_ms(), 1_250);

    let error = expect_error(
        manager.renew(lease.id(), &task, 200, 1_250),
        "the lease is expired at the exact expiration boundary",
    )?;
    assert_eq!(error.error_code(), ErrorCode::Transient);
    Ok(())
}

#[test]
fn test_renew_and_release_require_original_owner() -> Result<(), Box<dyn std::error::Error>> {
    let target = key("app")?;
    let owner_a = owner("owner_a")?;
    let owner_b = owner("owner_b")?;
    let mut manager = LeaseManager::new();
    let lease = manager.acquire(target, LeaseMode::Shared, &owner_a, 1_000, 100)?;

    let renew_error = expect_error(
        manager.renew(lease.id(), &owner_b, 1_000, 101),
        "another owner cannot renew",
    )?;
    assert!(matches!(renew_error, LeaseError::OwnerMismatch { .. }));
    let release_error = expect_error(
        manager.release(lease.id(), &owner_b),
        "another owner cannot release",
    )?;
    assert!(matches!(release_error, LeaseError::OwnerMismatch { .. }));
    assert_eq!(manager.release(lease.id(), &owner_a)?, lease);
    Ok(())
}

#[test]
fn test_release_reports_unknown_lease() -> Result<(), Box<dyn std::error::Error>> {
    let task = owner("task_1")?;
    let mut manager = LeaseManager::new();
    let lease = manager.acquire(key("app")?, LeaseMode::Shared, &task, 1_000, 100)?;
    manager.release(lease.id(), &task)?;
    let error = expect_error(
        manager.release(lease.id(), &task),
        "release is explicit and not idempotent",
    )?;
    assert_eq!(error.error_code(), ErrorCode::ToolInvalidArgs);
    Ok(())
}

#[test]
fn test_reap_expired_reports_removed_leases() -> Result<(), Box<dyn std::error::Error>> {
    let task = owner("task_1")?;
    let key_a = key("app_a")?;
    let key_b = key("app_b")?;
    let mut manager = LeaseManager::new();
    let lease_a = manager.acquire(key_a, LeaseMode::Shared, &task, 100, 1_000)?;
    let lease_b = manager.acquire(key_b, LeaseMode::Shared, &task, 1_000, 1_001)?;
    let lease_a_id = lease_a.id();

    let expired = manager.reap_expired(1_100)?;
    assert_eq!(expired, vec![lease_a]);
    assert_eq!(manager.stored_lease_count(), 1);
    assert!(manager.get(lease_a_id).is_none());
    assert_eq!(manager.get(lease_b.id()), Some(lease_b));
    Ok(())
}

#[test]
fn test_user_preemption_releases_all_matching_active_leases()
-> Result<(), Box<dyn std::error::Error>> {
    let target = key("app")?;
    let other_target = key("other_app")?;
    let owner_a = owner("owner_a")?;
    let owner_b = owner("owner_b")?;
    let mut manager = LeaseManager::new();
    let shared = manager.acquire(target.clone(), LeaseMode::Shared, &owner_a, 1_000, 100)?;
    let intent = manager.acquire(target.clone(), LeaseMode::Intent, &owner_b, 1_000, 101)?;
    let unrelated = manager.acquire(other_target, LeaseMode::Exclusive, &owner_a, 1_000, 102)?;

    let report = manager.preempt_for_user(&target, 200)?;
    assert_eq!(report.preempted(), &[shared, intent]);
    assert_eq!(report.expired().len(), 0);
    assert_eq!(manager.stored_lease_count(), 1);
    assert_eq!(manager.get(unrelated.id()), Some(unrelated));
    Ok(())
}

#[test]
fn test_user_preemption_reports_expired_without_counting_them_as_preempted()
-> Result<(), Box<dyn std::error::Error>> {
    let target = key("app")?;
    let task = owner("task_1")?;
    let mut manager = LeaseManager::new();
    let expired = manager.acquire(target.clone(), LeaseMode::Exclusive, &task, 100, 1_000)?;
    let active = manager.acquire(target.clone(), LeaseMode::Intent, &task, 1_000, 1_001)?;

    let report = manager.preempt_for_user(&target, 1_100)?;
    assert_eq!(report.expired(), &[expired]);
    assert_eq!(report.preempted(), &[active]);
    assert_eq!(manager.stored_lease_count(), 0);
    Ok(())
}

#[test]
fn test_acquire_many_sorts_by_canonical_key_before_allocating_ids()
-> Result<(), Box<dyn std::error::Error>> {
    let task = owner("task_1")?;
    let key_a = key("app_a")?;
    let key_b = key("app_b")?;
    let key_c = key("app_c")?;
    let mut manager = LeaseManager::new();

    let leases = manager.acquire_many(
        &task,
        &[
            LeaseRequest::new(key_c.clone(), LeaseMode::Shared, 1_000),
            LeaseRequest::new(key_a.clone(), LeaseMode::Shared, 1_000),
            LeaseRequest::new(key_b.clone(), LeaseMode::Shared, 1_000),
        ],
        100,
    )?;

    assert_eq!(
        leases.iter().map(Lease::key).collect::<Vec<_>>(),
        vec![&key_a, &key_b, &key_c]
    );
    assert_eq!(
        leases
            .iter()
            .map(|lease| lease.id().value())
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    Ok(())
}

#[test]
fn test_acquire_many_conflict_commits_zero_requested_leases()
-> Result<(), Box<dyn std::error::Error>> {
    let owner_a = owner("owner_a")?;
    let owner_b = owner("owner_b")?;
    let key_a = key("app_a")?;
    let key_b = key("app_b")?;
    let mut manager = LeaseManager::new();
    let existing = manager.acquire(key_a.clone(), LeaseMode::Exclusive, &owner_a, 1_000, 100)?;

    let error = expect_error(
        manager.acquire_many(
            &owner_b,
            &[
                LeaseRequest::new(key_b, LeaseMode::Shared, 1_000),
                LeaseRequest::new(key_a.clone(), LeaseMode::Shared, 1_000),
            ],
            101,
        ),
        "the second requested key conflicts",
    )?;
    let LeaseError::Conflict(conflict) = error else {
        return Err("expected a lease conflict".into());
    };
    assert_eq!(conflict.key(), &key_a);
    assert_eq!(conflict.blockers(), std::slice::from_ref(&existing));
    assert_eq!(manager.stored_lease_count(), 1);
    assert_eq!(manager.get(existing.id()), Some(existing));
    Ok(())
}

#[test]
fn test_acquire_many_rejects_duplicate_keys_before_commit() -> Result<(), Box<dyn std::error::Error>>
{
    let task = owner("task_1")?;
    let target = key("app")?;
    let mut manager = LeaseManager::new();
    let error = expect_error(
        manager.acquire_many(
            &task,
            &[
                LeaseRequest::new(target.clone(), LeaseMode::Shared, 1_000),
                LeaseRequest::new(target, LeaseMode::Intent, 1_000),
            ],
            100,
        ),
        "duplicate keys are ambiguous",
    )?;
    assert!(matches!(error, LeaseError::DuplicateRequestKey { .. }));
    assert_eq!(manager.stored_lease_count(), 0);
    Ok(())
}

#[test]
fn test_manager_rejects_invalid_ttl_timestamp_and_backward_clock()
-> Result<(), Box<dyn std::error::Error>> {
    let task = owner("task_1")?;
    let mut manager = LeaseManager::new();
    assert_eq!(
        manager
            .acquire(key("app")?, LeaseMode::Shared, &task, 0, 100)
            .map_err(|error| error.error_code()),
        Err(ErrorCode::ToolInvalidArgs)
    );
    assert_eq!(
        manager
            .acquire(key("app")?, LeaseMode::Shared, &task, 100, -1)
            .map_err(|error| error.error_code()),
        Err(ErrorCode::ToolInvalidArgs)
    );
    let _lease = manager.acquire(key("app")?, LeaseMode::Shared, &task, 100, 100)?;
    assert_eq!(
        manager
            .acquire(key("other")?, LeaseMode::Shared, &task, 1, i64::MAX)
            .map_err(|error| error.error_code()),
        Err(ErrorCode::Fatal)
    );
    assert_eq!(
        manager.reap_expired(99).map_err(|error| error.error_code()),
        Err(ErrorCode::Fatal)
    );
    Ok(())
}

#[test]
fn test_conflict_error_is_readable_and_retryable() -> Result<(), Box<dyn std::error::Error>> {
    let target = key("app")?;
    let owner_a = owner("owner_a")?;
    let owner_b = owner("owner_b")?;
    let mut manager = LeaseManager::new();
    let lease = manager.acquire(target.clone(), LeaseMode::Exclusive, &owner_a, 500, 100)?;

    let error = expect_error(
        manager.acquire(target, LeaseMode::Shared, &owner_b, 500, 101),
        "exclusive lease blocks shared acquisition",
    )?;
    assert_eq!(error.error_code(), ErrorCode::Transient);
    let rendered = error.to_string();
    assert!(rendered.contains("lease conflict"));
    assert!(rendered.contains("owner_b"));
    assert!(rendered.contains(&lease.id().to_string()));
    Ok(())
}

#[test]
fn test_default_manager_starts_lease_ids_at_one() -> Result<(), Box<dyn std::error::Error>> {
    let task = owner("task_1")?;
    let mut manager = LeaseManager::default();
    let lease = manager.acquire(key("app")?, LeaseMode::Shared, &task, 100, 1)?;
    assert_eq!(lease.id().value(), 1);
    Ok(())
}

fn assert_lease_error(error: &LeaseError, expected: ErrorCode, fragment: &str) {
    assert_eq!(error.error_code(), expected);
    let rendered = error.to_string();
    assert!(
        rendered.contains(fragment),
        "expected `{rendered}` to contain `{fragment}`"
    );
}

#[test]
fn test_lease_key_and_owner_accessors_are_stable() -> Result<(), Box<dyn std::error::Error>> {
    let task = owner("task_1")?;
    assert_eq!(task.as_str(), "task_1");
    assert_eq!(task.to_string(), "task_1");

    let bare = key("app")?;
    assert_eq!(bare.app_id(), "app");
    assert_eq!(bare.window_id(), None);
    assert_eq!(bare.document_id(), None);
    assert_eq!(bare.resource(), None);
    assert_eq!(bare.to_string(), "app=app");

    let full = bare
        .with_window_id("window_1")?
        .with_document_id("document_1")?
        .with_resource("selection")?;
    assert_eq!(full.app_id(), "app");
    assert_eq!(full.window_id(), Some("window_1"));
    assert_eq!(full.document_id(), Some("document_1"));
    assert_eq!(full.resource(), Some("selection"));
    assert_eq!(
        full.to_string(),
        "app=app window=window_1 document=document_1 resource=selection"
    );
    Ok(())
}

#[test]
fn test_lease_identifier_boundaries_are_explicit() -> Result<(), Box<dyn std::error::Error>> {
    let maximum = "a".repeat(256);
    assert_eq!(LeaseOwner::parse(maximum.clone())?.as_str(), maximum);
    assert_eq!(LeaseKey::new(maximum)?.app_id(), "a".repeat(256));

    let too_long = "a".repeat(257);
    assert_eq!(
        LeaseOwner::parse(too_long.clone()).map_err(|error| error.error_code()),
        Err(ErrorCode::ToolInvalidArgs)
    );
    assert_eq!(
        LeaseKey::new(too_long).map_err(|error| error.error_code()),
        Err(ErrorCode::ToolInvalidArgs)
    );
    assert_eq!(
        LeaseKey::new("app\0suffix").map_err(|error| error.error_code()),
        Err(ErrorCode::ToolInvalidArgs)
    );
    assert_eq!(
        key("app")?
            .with_window_id("   ")
            .map_err(|error| error.error_code()),
        Err(ErrorCode::ToolInvalidArgs)
    );
    assert_eq!(
        key("app")?
            .with_document_id("doc\0suffix")
            .map_err(|error| error.error_code()),
        Err(ErrorCode::ToolInvalidArgs)
    );
    assert_eq!(
        key("app")?
            .with_resource("a".repeat(257))
            .map_err(|error| error.error_code()),
        Err(ErrorCode::ToolInvalidArgs)
    );
    Ok(())
}

#[test]
fn test_lease_input_error_display_and_codes_are_stable() -> Result<(), Box<dyn std::error::Error>> {
    let task = owner("task_1")?;
    let mut manager = LeaseManager::new();
    let invalid_owner = expect_error(LeaseOwner::parse(""), "empty owner")?;
    assert_lease_error(
        &invalid_owner,
        ErrorCode::ToolInvalidArgs,
        "invalid lease_owner",
    );
    let invalid_ttl = expect_error(
        manager.acquire(key("app")?, LeaseMode::Shared, &task, 0, 100),
        "zero ttl",
    )?;
    assert_lease_error(&invalid_ttl, ErrorCode::ToolInvalidArgs, "lease TTL");
    let invalid_timestamp = expect_error(
        manager.acquire(key("app")?, LeaseMode::Shared, &task, 100, -1),
        "negative timestamp",
    )?;
    assert_lease_error(
        &invalid_timestamp,
        ErrorCode::ToolInvalidArgs,
        "lease timestamp must be non-negative",
    );
    let _lease = manager.acquire(key("app")?, LeaseMode::Shared, &task, 100, 100)?;
    let backward_clock = expect_error(manager.reap_expired(99), "backward clock")?;
    assert_lease_error(
        &backward_clock,
        ErrorCode::Fatal,
        "lease clock moved backwards",
    );
    let overflow = expect_error(
        manager.acquire(key("other")?, LeaseMode::Shared, &task, 1, i64::MAX),
        "expiration overflow",
    )?;
    assert_lease_error(&overflow, ErrorCode::Fatal, "numeric overflow");
    Ok(())
}

#[test]
fn test_lease_lookup_error_display_is_stable() -> Result<(), Box<dyn std::error::Error>> {
    let task = owner("task_1")?;
    let mut manager = LeaseManager::new();
    let lease = manager.acquire(key("app")?, LeaseMode::Shared, &task, 1_000, 100)?;
    manager.release(lease.id(), &task)?;
    let missing = expect_error(
        manager.release(lease.id(), &task),
        "released lease is missing",
    )?;
    assert_lease_error(&missing, ErrorCode::ToolInvalidArgs, "not found");
    let expired = manager.acquire(key("app")?, LeaseMode::Shared, &task, 100, 1_000)?;
    let expired_error = expect_error(
        manager.renew(expired.id(), &task, 200, 1_100),
        "expired renew",
    )?;
    assert_lease_error(&expired_error, ErrorCode::Transient, "expired at");
    let other_owner = owner("task_2")?;
    let mismatch = expect_error(
        manager.release(expired.id(), &other_owner),
        "owner mismatch",
    )?;
    assert_lease_error(&mismatch, ErrorCode::ToolInvalidArgs, "is owned by");
    Ok(())
}

#[test]
fn test_lease_duplicate_error_display_is_stable() -> Result<(), Box<dyn std::error::Error>> {
    let task = owner("task_1")?;
    let mut manager = LeaseManager::new();
    let duplicate = expect_error(
        manager.acquire_many(
            &task,
            &[
                LeaseRequest::new(key("duplicate")?, LeaseMode::Shared, 100),
                LeaseRequest::new(key("duplicate")?, LeaseMode::Intent, 100),
            ],
            1_100,
        ),
        "duplicate key",
    )?;
    assert_lease_error(
        &duplicate,
        ErrorCode::ToolInvalidArgs,
        "duplicate target key",
    );
    let blocker_owner = owner("task_3")?;
    let blocker = manager.acquire(key("app")?, LeaseMode::Exclusive, &task, 1_000, 1_100)?;
    let conflict = expect_error(
        manager.acquire(key("app")?, LeaseMode::Shared, &blocker_owner, 1_000, 1_101),
        "exclusive blocks shared",
    )?;
    assert_lease_error(&conflict, ErrorCode::Transient, "lease conflict");
    assert!(conflict.to_string().contains(&blocker.id().to_string()));
    Ok(())
}
