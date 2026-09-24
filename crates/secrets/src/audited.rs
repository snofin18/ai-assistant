//! 带审计的密钥存取装饰器（**fail-closed**）。
//!
//! 语义：包住任意 [`SecretStore`]，每次访问都先执行、再写一条 [`SecretAccessRecord`]；
//! **审计写不进去 → 该次访问按失败返回**，调用方拿不到密钥（读）也看不到成功（写 / 删）。
//!
//! 为什么"先执行再审计"而不是"先审计再执行"：结果只有执行完才知道，先写一条 `allowed` 会
//! 在操作随后失败时**说谎**。反过来，执行完再写审计仍然是 fail-closed —— 审计失败时我们
//! **丢弃结果**（读到的明文随之被 `zeroize` 清零）并返回 `AuditRejected`。
//!
//! 边界：**不做**策略判定（铁律 6，那是 policy）；**不**决定审计记录的持久化方式（那是注入的
//! [`SecretAccessAuditor`] 实现）。
//!
//! 不变量：
//!   1. 每个 `get` / `set` / `delete` / `contains` 都恰好产生一条记录（成功或失败都记）
//!   2. 审计失败 ⇒ 返回值是 `Err(AuditRejected)`；读到的明文**不外泄**
//!   3. 写 / 删在审计失败时**可能已经生效** —— 调用方看到的语义是"这次操作未被完整记录，
//!      故不视为成功"。`set` 幂等、`delete` 重试返回 `Ok(false)`，故重试安全
//!   4. 记录里不含明文（类型层保证：`SecretAccessRecord` 没有装明文的字段）
//!
//! 相关：架构 v2 §12.5（「密钥访问本身要审计」）、`crates/secrets/README.md`

use std::sync::Arc;

use assistant_storage::Clock;

use crate::access_audit::{
    SecretAccessAuditor, SecretAccessOperation, SecretAccessOutcome, SecretAccessRecord,
};
use crate::error::{SecretError, SecretResult};
use crate::{SecretName, SecretStore, SecretValue};

/// 装饰器：`S` 是真后端，`A` 是审计出口。
pub struct AuditedSecretStore<S, A> {
    inner: S,
    auditor: A,
    clock: Arc<dyn Clock>,
}

impl<S, A> AuditedSecretStore<S, A> {
    /// 装配：后端 + 审计出口 + 注入时钟。
    #[must_use]
    pub fn new(inner: S, auditor: A, clock: Arc<dyn Clock>) -> Self {
        Self {
            inner,
            auditor,
            clock,
        }
    }

    /// 拆回后端（测试与装配调试用）。
    #[must_use]
    pub fn into_inner(self) -> S {
        self.inner
    }

    /// 审计出口（测试断言用）。
    #[must_use]
    pub const fn auditor(&self) -> &A {
        &self.auditor
    }
}

impl<S: SecretStore, A: SecretAccessAuditor> AuditedSecretStore<S, A> {
    /// 写一条记录。
    fn record(
        &self,
        name: &SecretName,
        operation: SecretAccessOperation,
        outcome: SecretAccessOutcome,
    ) -> SecretResult<()> {
        let record =
            SecretAccessRecord::new(name.clone(), operation, outcome, self.clock.now_unix_ms());
        self.auditor.record_access(&record)
    }

    /// 收尾：审计成功 → 原样返回操作结果；审计失败 → 丢弃结果并报 [`SecretError::AuditRejected`]。
    ///
    /// 两个都失败时把**原始操作错误**也写进 `detail`：审计失败不能把真正的故障原因盖掉。
    fn finish<T>(
        &self,
        name: &SecretName,
        operation: SecretAccessOperation,
        outcome: SecretAccessOutcome,
        original: SecretResult<T>,
    ) -> SecretResult<T> {
        match self.record(name, operation, outcome) {
            Ok(()) => original,
            Err(audit_error) => Err(SecretError::AuditRejected {
                detail: match &original {
                    Ok(_) => format!("{operation} 已执行，但审计写入失败：{audit_error}"),
                    Err(original_error) => format!(
                        "{operation} 失败（{original_error}），且审计写入也失败：{audit_error}"
                    ),
                },
            }),
        }
    }
}

impl<S: SecretStore, A: SecretAccessAuditor> SecretStore for AuditedSecretStore<S, A> {
    fn get(&self, name: &SecretName) -> SecretResult<Option<SecretValue>> {
        let result = self.inner.get(name);
        let outcome = match &result {
            Ok(Some(_)) => SecretAccessOutcome::Allowed,
            Ok(None) => SecretAccessOutcome::NotFound,
            Err(_) => SecretAccessOutcome::Failed,
        };
        // 审计失败时 `result` 在这里被 drop → 读到的明文随之清零
        self.finish(name, SecretAccessOperation::Read, outcome, result)
    }

    fn set(&self, name: &SecretName, value: &SecretValue) -> SecretResult<()> {
        let result = self.inner.set(name, value);
        let outcome = match &result {
            Ok(()) => SecretAccessOutcome::Allowed,
            Err(_) => SecretAccessOutcome::Failed,
        };
        self.finish(name, SecretAccessOperation::Write, outcome, result)
    }

    fn delete(&self, name: &SecretName) -> SecretResult<bool> {
        let result = self.inner.delete(name);
        let outcome = match &result {
            Ok(true) => SecretAccessOutcome::Allowed,
            Ok(false) => SecretAccessOutcome::NotFound,
            Err(_) => SecretAccessOutcome::Failed,
        };
        self.finish(name, SecretAccessOperation::Delete, outcome, result)
    }

    fn contains(&self, name: &SecretName) -> SecretResult<bool> {
        let result = self.inner.contains(name);
        let outcome = match &result {
            Ok(true) => SecretAccessOutcome::Allowed,
            Ok(false) => SecretAccessOutcome::NotFound,
            Err(_) => SecretAccessOutcome::Failed,
        };
        // 存在性探测按 `read` 记：它同样泄露"某个 key 有没有配"
        self.finish(name, SecretAccessOperation::Read, outcome, result)
    }
}
