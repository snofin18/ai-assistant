//! selector 链的**纯函数**排序与歧义判定（架构 v2 §6.2 / §6.3、ADR-0022 D4）。
//!
//! 职责：把「怎么找」的决策从平台调用里**剥出来**，使它可以被单测（不需要真机、不需要 COM）。
//! 边界：**不碰** UIA / Win32；只吃 `SelectorChain` / `OnAmbiguous`，吐「排序结果 / 判定结果 / 错误」。
//!
//! ## 不变量
//! 1. **稳定排序**：有效分相同的候选**保持链内原顺序** → 同样的输入必得同样的顺序（可复现、可回放）。
//! 2. **`locale_dependent` 自动降权**（§6.3：`score *= 0.5`）—— 降权只影响**顺序**，
//!    **绝不**新增 / 删除候选（自愈不得产生不可解释的行为）。
//! 3. **多命中必须报歧义**（ADR-0044）：平台层**没有**「取第一个 / 取最高分」这类策略 ——
//!    那是「点偏了却看起来成功」的静默失败形态（铁律 1）。
//! 4. 判定是**纯函数**：同样的 `matched_scores` + 策略必得同样的结论。
//!
//! 相关：架构 v2 §6.2 / §6.3 / §6.4、ADR-0022 D4、`docs/spec/naming.md` §7。

use std::cmp::Ordering;

use assistant_platform_api::{
    ErrorCode, OnAmbiguous, PlatformError, SelectorCandidate, SelectorChain,
};

/// `locale_dependent` 候选的降权系数（§6.3）。
pub const LOCALE_PENALTY: f64 = 0.5;

/// 一个已按有效分排序的候选（借用原链，不复制）。
#[derive(Debug, Clone, Copy)]
pub struct RankedCandidate<'a> {
    candidate: &'a SelectorCandidate,
    effective_score: f64,
}

impl<'a> RankedCandidate<'a> {
    /// 原候选。
    pub const fn candidate(&self) -> &'a SelectorCandidate {
        self.candidate
    }

    /// 有效分（已含 `locale_dependent` 降权）。
    pub const fn effective_score(&self) -> f64 {
        self.effective_score
    }
}

/// 候选的有效分：`locale_dependent` 的候选乘 `LOCALE_PENALTY`（§6.3）。
#[must_use]
pub fn effective_score(candidate: &SelectorCandidate) -> f64 {
    if candidate.is_locale_dependent() {
        candidate.score() * LOCALE_PENALTY
    } else {
        candidate.score()
    }
}

/// 按有效分降序排序，并丢掉低于 `min_score_to_try` 的候选。
///
/// 排序是**稳定**的：同分候选保持链内原顺序（不变量 1）。
/// 用 `f64::total_cmp` 而不是 `partial_cmp` / `==`：前者是全序，既不 panic 也不触发 `float_cmp`，
/// 且 `score` 已在 `SelectorCandidate::new` 校验为有限值（§6.2 不变量 2）。
#[must_use]
pub fn rank_candidates<'a>(
    chain: &'a SelectorChain,
    min_score_to_try: f64,
) -> Vec<RankedCandidate<'a>> {
    let mut ranked: Vec<RankedCandidate<'a>> = chain
        .candidates()
        .iter()
        .map(|candidate| RankedCandidate {
            candidate,
            effective_score: effective_score(candidate),
        })
        .collect();
    ranked.sort_by(|left, right| right.effective_score.total_cmp(&left.effective_score));
    ranked.retain(|entry| entry.effective_score.total_cmp(&min_score_to_try) != Ordering::Less);
    ranked
}

/// 候选链的判定结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionOutcome {
    /// 一个都没匹配上 → `TargetNotFound`。
    NotFound,
    /// 多个匹配且无法安全择一 → `TargetAmbiguous`。
    Ambiguous {
        /// 匹配个数（进 message，便于定位）。
        matches: usize,
    },
    /// 唯一匹配 → 可安全使用。
    Unique,
}

/// 根据匹配数与歧义策略决定结论（**纯函数**）。
///
/// 平台层只有**一种**歧义语义（ADR-0044 D1）：多命中 → `Ambiguous`（fail-closed，铁律 1 不猜）。
/// `policy` 仍在签名里 —— 它是跨 crate 的 `#[non_exhaustive]` 枚举，**将来**新增策略时
/// 唯一的判定落点就是这里（改它要 ADR）。
#[must_use]
pub const fn decide_selection(matched_scores: &[f64], policy: OnAmbiguous) -> SelectionOutcome {
    // 用 `let … else` 而不是 `match`：两个分支若同体，`clippy::match_same_arms`（pedantic）
    // 会在 `-D warnings` 下报错，而这里「未来变体」与 `ErrorAndAsk` 确实映射到同一结论。
    let OnAmbiguous::ErrorAndAsk = policy else {
        // 未知策略一律按**最保守**的「报歧义、升级给人」处理（铁律 1）。
        return SelectionOutcome::Ambiguous {
            matches: matched_scores.len(),
        };
    };
    match matched_scores.len() {
        0 => SelectionOutcome::NotFound,
        1 => SelectionOutcome::Unique,
        matches => SelectionOutcome::Ambiguous { matches },
    }
}

/// 把判定结论翻译成 `PlatformError`（**纯函数**）。
///
/// `reasons` 是「每个候选为什么没匹配上」的人类可读说明 —— 带上它，
/// 时间线上才能回答「到底试过什么」（阶段 1 DoD「所有失败可从时间线定位原因」）。
#[must_use]
pub fn resolution_error(
    app_id: &str,
    outcome: SelectionOutcome,
    reasons: &[String],
) -> PlatformError {
    let tried = if reasons.is_empty() {
        "no candidate was eligible (all below min_score_to_try)".to_string()
    } else {
        reasons.join("; ")
    };
    match outcome {
        SelectionOutcome::NotFound => PlatformError::new(
            ErrorCode::TargetNotFound,
            format!("no window matched app_id `{app_id}`; tried: {tried}"),
        ),
        SelectionOutcome::Ambiguous { matches } => PlatformError::new(
            ErrorCode::TargetAmbiguous,
            format!(
                "{matches} windows matched app_id `{app_id}` and the top score is tied; refused to guess; tried: {tried}"
            ),
        ),
        SelectionOutcome::Unique => PlatformError::new(
            ErrorCode::Fatal,
            "internal error: resolution_error called for a unique match",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assistant_platform_api::{SelectorKind, SelectorValue};

    fn candidate(id: &str, score: f64, locale_dependent: bool) -> SelectorCandidate {
        match SelectorCandidate::new(
            id,
            SelectorKind::AutomationId,
            SelectorValue::Text(id.to_string()),
            score,
            locale_dependent,
        ) {
            Ok(candidate) => candidate,
            Err(error) => unreachable!("test fixture must be valid: {error}"),
        }
    }

    #[test]
    fn test_rank_orders_by_effective_score_descending() {
        let chain = SelectorChain::new(vec![
            candidate("low", 0.2, false),
            candidate("high", 0.9, false),
        ]);
        let ranked = rank_candidates(&chain, 0.0);
        assert_eq!(ranked.len(), 2);
        assert_eq!(
            ranked.first().map(|entry| entry.candidate().id()),
            Some("high")
        );
    }

    #[test]
    fn test_locale_dependent_candidate_is_penalized_not_dropped() {
        // 0.9 的本地化候选 → 有效分 0.45，排在 0.5 的非本地化候选之后，但**不被删除**（不变量 2）。
        let chain = SelectorChain::new(vec![
            candidate("localized", 0.9, true),
            candidate("stable", 0.5, false),
        ]);
        let ranked = rank_candidates(&chain, 0.0);
        assert_eq!(ranked.len(), 2, "降权不得删除候选");
        assert_eq!(
            ranked.first().map(|entry| entry.candidate().id()),
            Some("stable")
        );
        let localized = ranked
            .iter()
            .find(|entry| entry.candidate().id() == "localized");
        assert!(matches!(localized, Some(entry) if (entry.effective_score() - 0.45).abs() < 1e-9));
    }

    #[test]
    fn test_rank_is_stable_for_equal_scores() {
        let chain = SelectorChain::new(vec![
            candidate("first", 0.5, false),
            candidate("second", 0.5, false),
        ]);
        let ranked = rank_candidates(&chain, 0.0);
        let ids: Vec<&str> = ranked.iter().map(|entry| entry.candidate().id()).collect();
        assert_eq!(ids, vec!["first", "second"], "同分必须保持原顺序");
    }

    #[test]
    fn test_min_score_filter_drops_low_candidates() {
        let chain = SelectorChain::new(vec![
            candidate("keep", 0.8, false),
            candidate("drop", 0.1, false),
        ]);
        let ranked = rank_candidates(&chain, 0.5);
        assert_eq!(ranked.len(), 1);
        assert_eq!(
            ranked.first().map(|entry| entry.candidate().id()),
            Some("keep")
        );
    }

    #[test]
    fn test_decide_no_match_is_not_found() {
        assert_eq!(
            decide_selection(&[], OnAmbiguous::ErrorAndAsk),
            SelectionOutcome::NotFound
        );
    }

    #[test]
    fn test_decide_single_match_is_unique() {
        assert_eq!(
            decide_selection(&[0.4], OnAmbiguous::ErrorAndAsk),
            SelectionOutcome::Unique
        );
    }

    #[test]
    fn test_decide_multiple_matches_error_and_ask_is_ambiguous() {
        assert_eq!(
            decide_selection(&[0.9, 0.8], OnAmbiguous::ErrorAndAsk),
            SelectionOutcome::Ambiguous { matches: 2 }
        );
    }

    #[test]
    fn test_decide_multiple_matches_is_ambiguous_regardless_of_scores() {
        // ADR-0044：平台层没有「取最高分 / 取第一个」这类策略 —— 候选链的分数属于**候选**
        // 而非命中元素，所以「不同分的多个命中」也只能报歧义（负向用例，ADR-0019 N1）。
        assert_eq!(
            decide_selection(&[0.9, 0.8], OnAmbiguous::ErrorAndAsk),
            SelectionOutcome::Ambiguous { matches: 2 }
        );
        assert_eq!(
            decide_selection(&[0.2, 0.2], OnAmbiguous::ErrorAndAsk),
            SelectionOutcome::Ambiguous { matches: 2 }
        );
    }

    #[test]
    fn test_resolution_error_carries_code_and_reasons() {
        let reasons = vec!["aid=TextEditor -> 0 matches".to_string()];
        let not_found = resolution_error(
            "com.microsoft.notepad",
            SelectionOutcome::NotFound,
            &reasons,
        );
        assert_eq!(not_found.code(), ErrorCode::TargetNotFound);
        assert!(not_found.message().contains("TextEditor"));

        let ambiguous = resolution_error(
            "com.microsoft.notepad",
            SelectionOutcome::Ambiguous { matches: 3 },
            &reasons,
        );
        assert_eq!(ambiguous.code(), ErrorCode::TargetAmbiguous);
        assert!(ambiguous.message().contains('3'));
    }

    #[test]
    fn test_resolution_error_explains_empty_reason_list() {
        let error = resolution_error("app", SelectionOutcome::NotFound, &[]);
        assert!(error.message().contains("min_score_to_try"));
    }
}
