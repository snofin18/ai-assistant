//! 窗口候选链的**纯函数**匹配（ADR-0022 D4 / 架构 v2 §6.2）。
//!
//! 职责：判定「这个窗口是不是这个候选要的那个」，并把「本通道不支持该候选种类」与
//! 「支持但不匹配」**分开**表达 —— 前者进错误 message 的 reasons，后者只是"没命中"。
//! 边界：**不碰** Win32 / COM；只吃已经采集好的 `WindowFacts`。
//!
//! ## 为什么不支持的种类要显式返回 `Unsupported`
//! 静默把它们当"不匹配"会让调用方看到 `TargetNotFound`，而真实原因是"这个候选种类本通道
//! 根本没实现" —— 那是两种完全不同的处置（换候选 vs 换通道）。铁律 1 禁止这种混淆。
//!
//! ## 不变量
//! 1. **本地化属性不得作主 selector**（ADR-0022 D4）：`TitleRegex` / `NameRegex` 只按
//!    **低分兜底**参与，且它们匹配的是标题（本地化文本）。
//! 2. 匹配是纯函数：同样的 `(facts, candidate)` 必得同样的结论（可单测、可回放）。
//! 3. 大小写不敏感只用于 **class 名与标题**；`AutomationId` / `RuntimeId` **区分大小写**
//!    （它们是机器标识，不是给人读的文本）。
//!
//! 相关：架构 v2 §6.2 / §6.3 / §6.4、ADR-0022 D4、`docs/spec/naming.md` §7。

use assistant_platform_api::{SelectorCandidate, SelectorKind, SelectorValue};

/// 一个窗口用于匹配的**事实**（不含句柄、不含 COM 对象）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WindowFacts {
    /// Win32 窗口类名（非本地化）。
    pub(crate) class_name: String,
    /// 窗口标题（本地化文本；只作兜底）。
    pub(crate) title: String,
    /// UIA `AutomationId`（非本地化）；空串 = 该窗口没有，或 UIA 不可用。
    pub(crate) automation_id: String,
    /// UIA `RuntimeId` 的规范化形态（十进制、逗号分隔）；空串 = 不可用。
    pub(crate) runtime_id: String,
    /// UIA 元素是否**成功**创建过（false = 提升权限 / 窗口已消失，事实不完整）。
    pub(crate) uia_available: bool,
}

/// 候选与窗口的匹配结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateMatch {
    /// 命中。
    Matched,
    /// 该种类本通道支持，但这个窗口不满足它。
    NotMatched,
    /// 该种类本通道**未实现**（理由进错误 message，绝不静默当作"不匹配"）。
    Unsupported(&'static str),
}

/// 判定一个窗口是否满足一个候选（**纯函数**）。
pub fn match_window(facts: &WindowFacts, candidate: &SelectorCandidate) -> CandidateMatch {
    match candidate.kind() {
        SelectorKind::RuntimeId => text_value(candidate, |expected| {
            // RuntimeId 是机器标识 → 区分大小写、要求非空期望值。
            if !facts.uia_available {
                return CandidateMatch::Unsupported(
                    "RuntimeId 候选需要 UIA 元素，但该窗口的 UIA 元素创建失败（提升权限或已消失）",
                );
            }
            boolean_match(!expected.is_empty() && facts.runtime_id == expected)
        }),
        SelectorKind::AutomationId => text_value(candidate, |expected| {
            if !facts.uia_available {
                return CandidateMatch::Unsupported(
                    "AutomationId 候选需要 UIA 元素，但该窗口的 UIA 元素创建失败（提升权限或已消失）",
                );
            }
            boolean_match(!expected.is_empty() && facts.automation_id == expected)
        }),
        SelectorKind::ClassAndRole => match candidate.value() {
            SelectorValue::ClassAndRole { class, role } => {
                if !role.is_empty() && !role.eq_ignore_ascii_case("window") {
                    // 顶层窗口的 UIA control type 恒为 Window；其它角色属于元素域。
                    return CandidateMatch::NotMatched;
                }
                boolean_match(facts.class_name.eq_ignore_ascii_case(class))
            }
            _ => CandidateMatch::Unsupported(
                "ClassAndRole 候选必须用 SelectorValue::ClassAndRole 表达取值",
            ),
        },
        SelectorKind::TitleRegex | SelectorKind::NameRegex => text_value(candidate, |expected| {
            // DRIFT-017-4：本卡没有 regex 引擎（加依赖 = 漂移触发器 ①），因此这里按
            // **大小写不敏感的字面量子串**处理。绝不产生"假命中"：只可能漏命中，
            // 漏命中会让链继续往下走或最终报 TargetNotFound（明确失败）。
            boolean_match(contains_ignore_ascii_case(&facts.title, expected))
        }),
        SelectorKind::AxIdentifier => CandidateMatch::Unsupported(
            "AxIdentifier 是 macOS 无障碍概念；Windows 通道的等价物是 AutomationId",
        ),
        SelectorKind::RoleAndParent => CandidateMatch::Unsupported(
            "RoleAndParent 用于元素域（父候选引用同一链内的 id）；窗口域没有父候选",
        ),
        SelectorKind::A11yPath => CandidateMatch::Unsupported(
            "A11yPath 需要无障碍树路径遍历；窗口域未实现（元素域见 uia::resolve）",
        ),
        SelectorKind::VisualAnchor => {
            CandidateMatch::Unsupported("VisualAnchor 需要视觉 / OCR 通道（TASK-041 / 042）")
        }
        _ => CandidateMatch::Unsupported("未知的候选种类（本通道尚未支持）"),
    }
}

/// 把候选的取值取成文本后交给 `check`；取值形态不对 → `Unsupported`（不是静默不匹配）。
fn text_value(
    candidate: &SelectorCandidate,
    check: impl FnOnce(&str) -> CandidateMatch,
) -> CandidateMatch {
    match candidate.value() {
        SelectorValue::Text(text) => check(text),
        _ => CandidateMatch::Unsupported("该候选种类要求 SelectorValue::Text 取值"),
    }
}

/// `bool` → 匹配结论。
const fn boolean_match(matched: bool) -> CandidateMatch {
    if matched {
        CandidateMatch::Matched
    } else {
        CandidateMatch::NotMatched
    }
}

/// 大小写不敏感的 ASCII 子串包含（标题里的中文按**字节**比较，因此对非 ASCII 也正确 ——
/// UTF-8 下大小写折叠只影响 ASCII 字节，逐字节扫描不会破坏码点边界）。
pub fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        // 空模式会匹配一切 —— 那是"配置错误"，不是"命中所有窗口"。
        return false;
    }
    let haystack = haystack.as_bytes();
    let needle = needle.as_bytes();
    if needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        kind: SelectorKind,
        value: SelectorValue,
        score: f64,
        locale_dependent: bool,
    ) -> SelectorCandidate {
        match SelectorCandidate::new("c1", kind, value, score, locale_dependent) {
            Ok(candidate) => candidate,
            Err(error) => unreachable!("测试夹具必须合法：{error}"),
        }
    }

    fn notepad_facts() -> WindowFacts {
        WindowFacts {
            class_name: "Notepad".to_string(),
            title: "未命名 - 记事本".to_string(),
            automation_id: String::new(),
            runtime_id: "42,1,0".to_string(),
            uia_available: true,
        }
    }

    #[test]
    fn test_class_and_role_matches_case_insensitively() {
        let facts = notepad_facts();
        let hit = candidate(
            SelectorKind::ClassAndRole,
            SelectorValue::ClassAndRole {
                class: "notepad".to_string(),
                role: "Window".to_string(),
            },
            0.9,
            false,
        );
        assert_eq!(match_window(&facts, &hit), CandidateMatch::Matched);
    }

    #[test]
    fn test_class_and_role_with_element_role_is_not_a_window_match() {
        // 负向用例（ADR-0019 N1）：角色不是 Window 时**不得**当成窗口命中。
        let facts = notepad_facts();
        let other_role = candidate(
            SelectorKind::ClassAndRole,
            SelectorValue::ClassAndRole {
                class: "Notepad".to_string(),
                role: "Document".to_string(),
            },
            0.9,
            false,
        );
        assert_eq!(
            match_window(&facts, &other_role),
            CandidateMatch::NotMatched
        );
    }

    #[test]
    fn test_automation_id_requires_exact_case_sensitive_match() {
        let mut facts = notepad_facts();
        facts.automation_id = "TextEditor".to_string();
        let exact = candidate(
            SelectorKind::AutomationId,
            SelectorValue::Text("TextEditor".to_string()),
            0.9,
            false,
        );
        let wrong_case = candidate(
            SelectorKind::AutomationId,
            SelectorValue::Text("texteditor".to_string()),
            0.9,
            false,
        );
        assert_eq!(match_window(&facts, &exact), CandidateMatch::Matched);
        assert_eq!(
            match_window(&facts, &wrong_case),
            CandidateMatch::NotMatched
        );
    }

    #[test]
    fn test_empty_automation_id_never_matches() {
        // 负向用例：空期望值不得匹配"没有 automation id"的窗口（否则会命中一切）。
        let facts = notepad_facts();
        let empty = candidate(
            SelectorKind::AutomationId,
            SelectorValue::Text(String::new()),
            0.9,
            false,
        );
        assert_eq!(match_window(&facts, &empty), CandidateMatch::NotMatched);
    }

    #[test]
    fn test_title_fallback_is_case_insensitive_substring() {
        let facts = notepad_facts();
        let hit = candidate(
            SelectorKind::TitleRegex,
            SelectorValue::Text("记事本".to_string()),
            0.2,
            true,
        );
        assert_eq!(match_window(&facts, &hit), CandidateMatch::Matched);
    }

    #[test]
    fn test_empty_title_pattern_matches_nothing() {
        // 负向用例：空模式是配置错误，**不得**匹配所有窗口。
        let facts = notepad_facts();
        let empty = candidate(
            SelectorKind::TitleRegex,
            SelectorValue::Text(String::new()),
            0.2,
            true,
        );
        assert_eq!(match_window(&facts, &empty), CandidateMatch::NotMatched);
    }

    #[test]
    fn test_unsupported_kinds_are_reported_not_silently_unmatched() {
        // 负向用例（铁律 1）：未实现的种类必须显式报 Unsupported。
        let facts = notepad_facts();
        for kind in [SelectorKind::AxIdentifier, SelectorKind::VisualAnchor] {
            let entry = candidate(kind, SelectorValue::Text("x".to_string()), 0.5, false);
            assert!(
                matches!(match_window(&facts, &entry), CandidateMatch::Unsupported(_)),
                "{kind:?} 必须报 Unsupported"
            );
        }
    }

    #[test]
    fn test_missing_uia_element_makes_uia_based_candidates_unsupported() {
        // 负向用例：拿不到 UIA 元素时**不得**把 AutomationId 候选当成"不匹配"（那会掩盖权限问题）。
        let facts = WindowFacts {
            uia_available: false,
            ..notepad_facts()
        };
        let entry = candidate(
            SelectorKind::AutomationId,
            SelectorValue::Text("TextEditor".to_string()),
            0.9,
            false,
        );
        assert!(matches!(
            match_window(&facts, &entry),
            CandidateMatch::Unsupported(_)
        ));
    }
}
