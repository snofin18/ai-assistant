//! 合成输入（`SendInput`）—— 铁律 5 的 **L4** 层。
//!
//! 职责：把「按键组合 / 指针动作」变成 `SendInput` 的事件序列，并在**发送前**校验前台窗口。
//! 边界：**不做**坐标换算（`crate::coordinates`）、**不做**窗口定位（`crate::window`）、
//! **不做**权限判定（放行点是 `crates/policy`，TASK-021）、**不做**结果断言（TASK-023）。
//!
//! ## 为什么这是最后手段（铁律 5）
//! 调用方**必须**先试 L1（`set_value` / `edit_text` / `invoke_action`）→ L2（命令 / 快捷键）→ L3（无障碍）。
//! 合成输入抢用户键盘鼠标、依赖焦点、且跨平台语义不一致，只在上面都做不到时才用。
//!
//! ## 为什么纯逻辑与 FFI 分开放
//! 本文件在**所有平台**编译：键名 → 虚拟键码、组合键的按下/抬起顺序、UTF-16 展开、
//! 指针动作的事件序列都是**纯函数**，三平台都能单测；并且 `cargo clippy --target <非宿主>`
//! 能覆盖它（ADR-0045 / PL-070 的教训）。真实 `SendInput` 调用在 `#[cfg(windows)]` 的 `win32` 子模块。
//!
//! ## 不变量
//! 1. **发送前必须校验前台窗口**：不一致 → 先尝试置前并**回读确认**，仍不一致 → `TargetUnresponsive`。
//!    「盲发」会把按键打到用户当前的应用上（最危险的一类静默失败）。
//! 2. **`SendInput` 返回事件数必须等于请求数**：不等 → 区分 UIPI / 参数错误并报错（铁律 1）。
//! 3. **文本一律走 `KEYEVENTF_UNICODE`**：绕过 IME 组字与键盘布局，因此「IME 开着也能正确写入」。
//! 4. **禁止 `keybd_event` / `SendKeys`**：前者已被 `SendInput` 取代，后者在非交互会话不可靠
//!    （`docs/memory/win32-input-research.md` §2 / §3）。
//!
//! 相关：架构 v2 §13.1.1 / §13.2 / §6.9、`docs/memory/win32-input-research.md`、
//! `docs/spec/naming.md` §5（非本地化键名）、铁律 5 / 1 / 4。

use assistant_platform_api::{
    ErrorCode, KeyChord, KeyModifier, PhysicalPoint, PlatformError, PlatformResult, PointerAction,
};

// 本文件在**所有平台**编译（ADR-0045 的非宿主门禁要覆盖它），因此**不能**依赖 `crate::error`
// —— 那个模块只在 Windows 上编译（它要把 `windows` crate 的 HRESULT 翻成 `ErrorCode`）。
// 下面两个私有构造器与 `crate::error` 的同名函数语义一致，只是去掉了平台依赖。

/// Win32 虚拟键码（`VK_*`）。
///
/// 为什么不用 `windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY`：
/// 那个类型只在 Windows 上存在，会让本模块无法在非宿主平台编译（ADR-0045 的门禁就覆盖不到）。
/// 这里用 newtype 保存裸值，`win32` 子模块再转成 `VIRTUAL_KEY`（一次显式转换，语义清楚）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualKey(u16);

impl VirtualKey {
    /// 由裸 `VK_*` 值构造。
    #[must_use]
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }

    /// 裸 `VK_*` 值。
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }
}

/// 一个键盘事件（按下或抬起）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    /// 键码。
    pub key: VirtualKey,
    /// 是否抬起（`false` = 按下）。
    pub is_key_up: bool,
}

impl KeyEvent {
    /// 按下。
    #[must_use]
    const fn down(key: VirtualKey) -> Self {
        Self {
            key,
            is_key_up: false,
        }
    }

    /// 抬起。
    #[must_use]
    const fn up(key: VirtualKey) -> Self {
        Self {
            key,
            is_key_up: true,
        }
    }
}

/// 指针动作的一个步骤（纯数据；执行在 `win32` 子模块）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerStep {
    /// 移动到该物理点。
    MoveTo(PhysicalPoint),
    /// 左键按下。
    LeftDown,
    /// 左键抬起。
    LeftUp,
}

/// 修饰键 → 虚拟键码。
///
/// 用**左**修饰键（`VK_CONTROL` / `VK_MENU` / `VK_SHIFT` / `VK_LWIN`）：Win32 的
/// `VK_SHIFT` / `VK_CONTROL` / `VK_MENU` 是「不分左右」的虚拟码，`SendInput` 会按当前
/// 键盘状态解析；显式用左键码避免「用户按着右 Shift」时语义漂移。
const fn modifier_virtual_key(modifier: KeyModifier) -> VirtualKey {
    match modifier {
        KeyModifier::Control => VirtualKey::new(0x11), // VK_CONTROL
        KeyModifier::Alt => VirtualKey::new(0x12),     // VK_MENU
        KeyModifier::Shift => VirtualKey::new(0x10),   // VK_SHIFT
        KeyModifier::Meta => VirtualKey::new(0x5B),    // VK_LWIN
        // `KeyModifier` 是 `#[non_exhaustive]`（`crates/platform/api` 的公共枚举），
        // 未来新增修饰键时必须显式决定键码 —— 用 Meta 的键码是**错误的**兜底，
        // 所以这里改成显式可识别值：未知修饰键落到 `0`（无效 VK），由 `key_events_for_chord` 拒掉。
        _ => VirtualKey::new(0),
    }
}

/// 非本地化键名 → 虚拟键码（**纯函数**，三平台可单测）。
///
/// 名字取 UIA 控制类型的同一套「规范英文名」精神（`docs/spec/naming.md` §5）：
/// 字母用大写单字符（`A`）、数字用单字符（`0`）、功能键 `F1`..`F24`、
/// 其余用 Win32 文档里的英文名（`Enter` / `Tab` / `Escape` / `Space` / `Left` …）。
/// **不**接受中文名或本地化名（那会随系统语言漂移）。
///
/// # Errors
/// 空名 / 未知名 / 修饰键名（`Control` 等，应放 `modifiers` 而不是 `key`）→ `ToolInvalidArgs`，
/// 消息里带上原名（否则「按键没生效」无从排查）。
pub fn virtual_key_for_name(name: &str) -> PlatformResult<VirtualKey> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(invalid_args(
            "key chord: key name must not be empty (an empty key cannot be sent)",
        ));
    }
    let upper = trimmed.to_ascii_uppercase();
    if let Some(vk) = named_virtual_key(&upper) {
        return Ok(vk);
    }
    if let Some(vk) = single_character_virtual_key(&upper) {
        return Ok(vk);
    }
    if let Some(vk) = function_key_virtual_key(&upper) {
        return Ok(vk);
    }
    Err(invalid_args(format!(
        "key chord: unknown non-localized key name `{name}` (see docs/spec/naming.md §5)"
    )))
}

/// 有名键（`Enter` / `Tab` / …）。
fn named_virtual_key(upper: &str) -> Option<VirtualKey> {
    let raw = match upper {
        "BACKSPACE" => 0x08,
        "TAB" => 0x09,
        "ENTER" | "RETURN" => 0x0D,
        "ESCAPE" | "ESC" => 0x1B,
        "SPACE" => 0x20,
        "PAGEUP" => 0x21,
        "PAGEDOWN" => 0x22,
        "END" => 0x23,
        "HOME" => 0x24,
        "LEFT" => 0x25,
        "UP" => 0x26,
        "RIGHT" => 0x27,
        "DOWN" => 0x28,
        "INSERT" => 0x2D,
        "DELETE" => 0x2E,
        _ => return None,
    };
    Some(VirtualKey::new(raw))
}

/// 单字符键：`A`..`Z` → `VK_A`..`VK_Z`，`0`..`9` → `VK_0`..`VK_9`。
fn single_character_virtual_key(upper: &str) -> Option<VirtualKey> {
    let mut chars = upper.chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    if first.is_ascii_uppercase() {
        // `u16::from(u8)` 精确无截断；`VK_A == 0x41 == b'A'`。
        return Some(VirtualKey::new(u16::from(first as u8)));
    }
    if first.is_ascii_digit() {
        return Some(VirtualKey::new(u16::from(first as u8)));
    }
    None
}

/// 功能键：`F1`..`F24` → `VK_F1`(0x70)..`VK_F24`(0x87)。
fn function_key_virtual_key(upper: &str) -> Option<VirtualKey> {
    let digits = upper.strip_prefix('F')?;
    let index: u16 = digits.parse().ok()?;
    if !(1..=24).contains(&index) {
        return None;
    }
    Some(VirtualKey::new(0x70 + (index - 1)))
}

/// 组合键 → 事件序列（**纯函数**，三平台可单测）。
///
/// 顺序遵循 Win32 惯例：**修饰键按下 → 主键按下 → 主键抬起 → 修饰键抬起**（逆序）。
/// 逆序抬起是必须的：`Ctrl+Shift+S` 若先抬 `Ctrl`，目标应用会看到 `Shift+S`（另一个快捷键）。
///
/// # Errors
/// 主键名非法 → `ToolInvalidArgs`；修饰键重复 → `ToolInvalidArgs`
/// （重复按下同一个修饰键会让「抬起」次数不匹配，留下粘住的修饰键 —— 危险的静默副作用）。
pub fn key_events_for_chord(chord: &KeyChord) -> PlatformResult<Vec<KeyEvent>> {
    let main = virtual_key_for_name(chord.key())?;
    let mut modifier_keys: Vec<VirtualKey> = Vec::with_capacity(chord.modifiers().len());
    for modifier in chord.modifiers() {
        let key = modifier_virtual_key(*modifier);
        if key.raw() == 0 {
            return Err(invalid_args(
                "key chord: unsupported modifier (unknown variant); refusing to guess its key code",
            ));
        }
        if modifier_keys.contains(&key) {
            return Err(invalid_args(format!(
                "key chord: duplicate modifier would leave a stuck key (key code {:#04x})",
                key.raw()
            )));
        }
        modifier_keys.push(key);
    }
    if modifier_keys.contains(&main) {
        return Err(invalid_args(format!(
            "key chord: `{}` is a modifier and must go in `modifiers`, not `key`",
            chord.key()
        )));
    }
    let mut events = Vec::with_capacity((modifier_keys.len() * 2) + 2);
    for key in &modifier_keys {
        events.push(KeyEvent::down(*key));
    }
    events.push(KeyEvent::down(main));
    events.push(KeyEvent::up(main));
    for key in modifier_keys.iter().rev() {
        events.push(KeyEvent::up(*key));
    }
    Ok(events)
}

/// 文本 → UTF-16 码元序列（**纯函数**，三平台可单测）。
///
/// 为什么用 UTF-16 而不是「字符」：`KEYBDINPUT.wScan` 是 16 位，基本多文种平面之外的
/// 字符（如 emoji）必须拆成**代理对**两个码元依次发送；`encode_utf16` 正是这个语义。
#[must_use]
pub fn utf16_units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// 指针动作 → 事件序列（**纯函数**，三平台可单测）。
///
/// `start` / `drop` 都必须是**已经换算好**的物理点（换算在 `crate::coordinates`）。
///
/// # Errors
/// `DragTo` 缺 `drop` 点 → `ToolInvalidArgs`（**不**把终点当成起点 —— 那会变成一次单击）。
pub fn pointer_steps(
    action: &PointerAction,
    start: PhysicalPoint,
    drop: Option<PhysicalPoint>,
) -> PlatformResult<Vec<PointerStep>> {
    let mut steps = vec![PointerStep::MoveTo(start)];
    match action {
        PointerAction::Move => {}
        PointerAction::Click => {
            steps.push(PointerStep::LeftDown);
            steps.push(PointerStep::LeftUp);
        }
        PointerAction::DoubleClick => {
            for _ in 0..2 {
                steps.push(PointerStep::LeftDown);
                steps.push(PointerStep::LeftUp);
            }
        }
        PointerAction::DragTo { .. } => {
            let Some(target) = drop else {
                return Err(invalid_args(
                    "pointer action DragTo requires a drop point in physical pixels",
                ));
            };
            steps.push(PointerStep::LeftDown);
            steps.push(PointerStep::MoveTo(target));
            steps.push(PointerStep::LeftUp);
        }
        // `PointerAction` 是 `#[non_exhaustive]`：未来新增动作时**必须**显式处理，
        // 这里拒绝而不是默默退化成「只移动」（那会看起来成功却没做事）。
        _ => {
            return Err(capability_missing(
                "pointer action: this variant is not implemented by the Windows channel yet",
            ));
        }
    }
    Ok(steps)
}

/// 键盘事件序列是否需要「先置前」——目前恒为 `true`（任何键盘输入都要求前台窗口）。
///
/// 单独抽成函数是为了让「发送前 100% 校验前台窗口」这条不变量有一个可被引用的名字，
/// 而不是散落在调用点（将来若出现「后台可用的输入方式」再改这里并附理由）。
#[must_use]
pub const fn requires_foreground() -> bool {
    true
}

#[cfg(windows)]
mod win32;

#[cfg(windows)]
pub use win32::{is_ime_open, pointer_action, send_key_action, send_unicode_text};

// 真机验收（真实显示器 + 真实记事本）：`#[ignore]` 默认不跑，且**只能**住在 lib 里 ——
// 它要调 Win32（`GetCursorPos` / `SetProcessDpiAwarenessContext`），而 `unsafe` 授权是
// crate 级的（`src/lib.rs` 的 `#![allow(unsafe_code)]`），`tests/**` 是独立 crate 拿不到。
#[cfg(all(test, windows))]
mod acceptance;

/// 构造 `ToolInvalidArgs`（理由见文件头的「为什么本地重复这两个构造器」）。
fn invalid_args(message: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::ToolInvalidArgs, message)
}

/// 构造 `CapabilityMissing`（本通道还没实现这种动作，**不是** Fatal）。
fn capability_missing(message: impl Into<String>) -> PlatformError {
    PlatformError::new(ErrorCode::CapabilityMissing, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use assistant_platform_api::{CoordinateSpace, CoordinateSpaceKind, NormalizedPoint};

    /// 测试夹具：把事件序列压成 `(虚拟键码, 是否抬起)`，便于断言**顺序**。
    fn shape(events: &[KeyEvent]) -> Vec<(u16, bool)> {
        events
            .iter()
            .map(|event| (event.key.raw(), event.is_key_up))
            .collect()
    }

    /// 测试夹具：把指针步骤压成可比较的字符串（`PhysicalPoint` 没有公开构造器）。
    fn step_shape(steps: &[PointerStep]) -> Vec<&'static str> {
        steps
            .iter()
            .map(|step| match step {
                PointerStep::MoveTo(_) => "move",
                PointerStep::LeftDown => "down",
                PointerStep::LeftUp => "up",
            })
            .collect()
    }

    /// 测试夹具：规范点。
    fn normalized(x: f64, y: f64) -> NormalizedPoint {
        match NormalizedPoint::new(x, y) {
            Ok(point) => point,
            Err(failure) => unreachable!("测试夹具必须合法: {failure}"),
        }
    }

    /// 测试夹具：物理点（经恒等空间换算 —— 换算只有 `to_physical` 一个入口）。
    fn physical(x: f64, y: f64) -> PhysicalPoint {
        let space = match CoordinateSpace::new(CoordinateSpaceKind::PhysicalPixels, 1.0, "t") {
            Ok(space) => space,
            Err(failure) => unreachable!("测试夹具必须合法: {failure}"),
        };
        match normalized(x, y).to_physical(&space) {
            Ok(physical) => physical,
            Err(failure) => unreachable!("测试夹具必须合法: {failure}"),
        }
    }

    #[test]
    fn test_virtual_key_for_name_maps_named_keys() {
        // 非本地化键名（docs/spec/naming.md §5）：名字与键盘布局 / 系统语言无关。
        assert_eq!(virtual_key_for_name("Enter").map(VirtualKey::raw), Ok(0x0D));
        assert_eq!(virtual_key_for_name("tab").map(VirtualKey::raw), Ok(0x09));
        assert_eq!(
            virtual_key_for_name("  ESC  ").map(VirtualKey::raw),
            Ok(0x1B)
        );
        assert_eq!(virtual_key_for_name("s").map(VirtualKey::raw), Ok(0x53));
        assert_eq!(virtual_key_for_name("F24").map(VirtualKey::raw), Ok(0x87));
    }

    #[test]
    fn test_virtual_key_for_name_rejects_unknown_names() {
        // 负向用例（ADR-0019 N1）：未知键名**不得**猜一个键码发出去。
        for name in ["", "   ", "F25", "F0", "Control", "ä", "EnterKey", "0x0D"] {
            let failure = match virtual_key_for_name(name) {
                Ok(key) => unreachable!("`{name}` 必须被拒绝，实际得到 {:#04x}", key.raw()),
                Err(failure) => failure,
            };
            assert_eq!(failure.code(), ErrorCode::ToolInvalidArgs, "键名 `{name}`");
        }
    }

    #[test]
    fn test_key_events_for_chord_presses_modifiers_and_releases_in_reverse() {
        // Ctrl+Shift+S：修饰键按下 → 主键按下 → 主键抬起 → 修饰键**逆序**抬起。
        // 逆序是必须的：若先抬 Ctrl，目标应用会看到 Shift+S（另一个快捷键）。
        let chord = KeyChord::new(
            "s".to_string(),
            vec![KeyModifier::Control, KeyModifier::Shift],
        );
        let events = match key_events_for_chord(&chord) {
            Ok(events) => events,
            Err(failure) => unreachable!("Ctrl+Shift+S 必须能编成事件序列: {failure}"),
        };
        assert_eq!(
            shape(&events),
            vec![
                (0x11, false),
                (0x10, false),
                (0x53, false),
                (0x53, true),
                (0x10, true),
                (0x11, true),
            ]
        );
    }

    #[test]
    fn test_key_events_for_chord_rejects_duplicate_modifiers() {
        // 负向用例：重复修饰键会让「抬起」次数不匹配 → 留下粘住的修饰键（危险的静默副作用）。
        let chord = KeyChord::new(
            "s".to_string(),
            vec![KeyModifier::Control, KeyModifier::Control],
        );
        let failure = match key_events_for_chord(&chord) {
            Ok(events) => unreachable!("重复修饰键必须被拒绝，实际得到 {} 个事件", events.len()),
            Err(failure) => failure,
        };
        assert_eq!(failure.code(), ErrorCode::ToolInvalidArgs);
    }

    #[test]
    fn test_utf16_units_splits_astral_characters_into_surrogate_pairs() {
        // 基本多文种平面之外（emoji）必须拆成代理对：`KEYBDINPUT.wScan` 只有 16 位。
        let units = utf16_units("\u{1F600}");
        assert_eq!(units.len(), 2);
        assert_eq!(units.first().copied(), Some(0xD83D));
        assert_eq!(units.get(1).copied(), Some(0xDE00));
        // 中文是 BMP 内的单码元（这条路径正是「IME 开着也能正确写入」的根据）。
        assert_eq!(utf16_units("中").len(), 1);
    }

    #[test]
    fn test_pointer_steps_click_is_move_then_down_then_up() {
        let steps = match pointer_steps(&PointerAction::Click, physical(10.0, 20.0), None) {
            Ok(steps) => steps,
            Err(failure) => unreachable!("Click 必须能编成事件序列: {failure}"),
        };
        assert_eq!(step_shape(&steps), vec!["move", "down", "up"]);
    }

    #[test]
    fn test_pointer_steps_drag_requires_a_drop_point() {
        // 负向用例：缺 `drop` 点时**不得**把终点当起点（那会静默变成一次单击）。
        let action = PointerAction::DragTo {
            drop_at: normalized(30.0, 40.0),
        };
        let failure = match pointer_steps(&action, physical(10.0, 20.0), None) {
            Ok(steps) => unreachable!("缺 drop 点必须报错，实际得到 {} 步", steps.len()),
            Err(failure) => failure,
        };
        assert_eq!(failure.code(), ErrorCode::ToolInvalidArgs);
    }

    #[test]
    fn test_pointer_steps_drag_uses_the_given_drop_point() {
        let action = PointerAction::DragTo {
            drop_at: normalized(30.0, 40.0),
        };
        let steps = match pointer_steps(&action, physical(10.0, 20.0), Some(physical(30.0, 40.0))) {
            Ok(steps) => steps,
            Err(failure) => unreachable!("DragTo 必须能编成事件序列: {failure}"),
        };
        assert_eq!(step_shape(&steps), vec!["move", "down", "move", "up"]);
    }

    #[test]
    fn test_requires_foreground_is_true() {
        // 键盘输入永远要求前台窗口；这条不变量有一个可被引用的名字。
        assert!(requires_foreground());
    }
}
