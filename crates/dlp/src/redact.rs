//! 截图脱敏的纯规则模型与遮挡矩形决策（ADR-0073）。
//!
//! 职责：把调用方**已判定**的敏感区域（密码框、文本扫描命中区间）解析为可直接消费的
//! 遮挡矩形，并在越界、负坐标、空规则等情况下 fail-closed。
//!
//! 边界：本模块不读取像素、不调用平台 API、不实现正则或文本扫描；文本命中必须由调用方
//! 传入区间或显式词表预判。像素遮挡由平台层执行，本模块只产出决策。
//!
//! ## 不变量
//! 1. 空规则、负坐标、零宽高、越界、溢出、超上限都必须显式失败。
//! 2. 规则数量有硬上限，输出矩形数量不超过输入规则数量。
//! 3. 所有输出矩形都已归一化为非负 `u32` 坐标，并完整落在截图边界内。

/// 单次截图允许的最大脱敏规则数量（ADR-0063 / ADR-0073）。
pub const MAX_REDACTION_RULES: usize = 256;

/// 一次截图的像素边界（只描述尺寸，不持有像素）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageDimensions {
    width: u32,
    height: u32,
}

impl ImageDimensions {
    /// 构造截图边界；宽或高为零时显式失败。
    ///
    /// # Errors
    /// `width == 0` 或 `height == 0` 时返回 [`RedactError::ZeroDimension`]。
    pub const fn new(width: u32, height: u32) -> Result<Self, RedactError> {
        if width == 0 || height == 0 {
            return Err(RedactError::ZeroDimension);
        }
        Ok(Self { width, height })
    }

    /// 像素宽度。
    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    /// 像素高度。
    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }
}

/// 调用方已判定的敏感区域（矩形坐标）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RedactionRegion {
    x: i64,
    y: i64,
    width: u32,
    height: u32,
    right: i64,
    bottom: i64,
}

impl RedactionRegion {
    /// 构造敏感区域。
    ///
    /// # Errors
    /// 负坐标、零宽高或坐标加法溢出时分别返回
    /// [`RedactError::NegativeCoordinate`]、[`RedactError::ZeroDimension`]
    /// 或 [`RedactError::CoordinateOverflow`]。
    pub fn new(x: i64, y: i64, width: u32, height: u32) -> Result<Self, RedactError> {
        if x < 0 || y < 0 {
            return Err(RedactError::NegativeCoordinate);
        }
        if width == 0 || height == 0 {
            return Err(RedactError::ZeroDimension);
        }
        let Some(right) = x.checked_add(i64::from(width)) else {
            return Err(RedactError::CoordinateOverflow);
        };
        let Some(bottom) = y.checked_add(i64::from(height)) else {
            return Err(RedactError::CoordinateOverflow);
        };
        Ok(Self {
            x,
            y,
            width,
            height,
            right,
            bottom,
        })
    }

    /// 左边界。
    #[must_use]
    pub const fn x(self) -> i64 {
        self.x
    }

    /// 上边界。
    #[must_use]
    pub const fn y(self) -> i64 {
        self.y
    }

    /// 宽度。
    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    /// 高度。
    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }

    /// 右边界（不含）。
    #[must_use]
    pub const fn right(self) -> i64 {
        self.right
    }

    /// 下边界（不含）。
    #[must_use]
    pub const fn bottom(self) -> i64 {
        self.bottom
    }
}

/// 脱敏规则。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RedactionRule {
    /// 调用方已识别的密码框或同等敏感控件区域。
    PasswordField {
        /// 已判定区域。
        region: RedactionRegion,
    },
    /// 文本扫描器已判定的命中区域；本模块不重新匹配文本。
    MatchedRegion {
        /// 已判定区域。
        region: RedactionRegion,
    },
}

impl RedactionRule {
    /// 构造密码框规则。
    #[must_use]
    pub const fn password_field(region: RedactionRegion) -> Self {
        Self::PasswordField { region }
    }

    /// 构造已判定文本命中规则。
    #[must_use]
    pub const fn matched_region(region: RedactionRegion) -> Self {
        Self::MatchedRegion { region }
    }

    /// 规则对应的敏感区域。
    #[must_use]
    pub const fn region(self) -> RedactionRegion {
        match self {
            Self::PasswordField { region } | Self::MatchedRegion { region } => region,
        }
    }
}

/// 已归一化、已验证落在截图边界内的遮挡矩形。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OcclusionRectangle {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl OcclusionRectangle {
    const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// 左边界。
    #[must_use]
    pub const fn x(self) -> u32 {
        self.x
    }

    /// 上边界。
    #[must_use]
    pub const fn y(self) -> u32 {
        self.y
    }

    /// 宽度。
    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    /// 高度。
    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }
}

/// 脱敏规则解析错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RedactError {
    /// 调用方要求脱敏但没有提供任何规则。
    NoRules,
    /// 区域坐标为负。
    NegativeCoordinate,
    /// 区域宽或高为零，或截图宽高为零。
    ZeroDimension,
    /// 坐标加宽高发生溢出。
    CoordinateOverflow,
    /// 区域没有完整落在截图边界内。
    RegionOutOfBounds,
    /// 规则数量超过 [`MAX_REDACTION_RULES`]。
    TooManyRules,
}

impl std::fmt::Display for RedactError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::NoRules => "redaction was requested without any rules",
            Self::NegativeCoordinate => "redaction region coordinates must be non-negative",
            Self::ZeroDimension => "redaction dimensions must be non-zero",
            Self::CoordinateOverflow => "redaction region coordinates overflowed",
            Self::RegionOutOfBounds => "redaction region is outside the screenshot bounds",
            Self::TooManyRules => "redaction rule count exceeds the hard limit",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for RedactError {}

/// 把已判定规则解析成遮挡矩形。
///
/// # Errors
/// 规则为空、超过上限、包含负坐标 / 零宽高 / 溢出，或任一区域越出 `bounds` 时返回
/// 对应的 [`RedactError`]。任何失败都不会产生部分结果。
#[must_use = "redaction failures must be handled explicitly"]
pub fn resolve_occlusions(
    bounds: ImageDimensions,
    rules: &[RedactionRule],
) -> Result<Vec<OcclusionRectangle>, RedactError> {
    if rules.is_empty() {
        return Err(RedactError::NoRules);
    }
    if rules.len() > MAX_REDACTION_RULES {
        return Err(RedactError::TooManyRules);
    }

    let mut occlusions = Vec::with_capacity(rules.len());
    for rule in rules {
        let region = rule.region();
        if region.right() > i64::from(bounds.width())
            || region.bottom() > i64::from(bounds.height())
        {
            return Err(RedactError::RegionOutOfBounds);
        }
        let x = u32::try_from(region.x()).map_err(|_| RedactError::CoordinateOverflow)?;
        let y = u32::try_from(region.y()).map_err(|_| RedactError::CoordinateOverflow)?;
        occlusions.push(OcclusionRectangle::new(
            x,
            y,
            region.width(),
            region.height(),
        ));
    }
    Ok(occlusions)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(x: i64, y: i64, width: u32, height: u32) -> Result<RedactionRegion, RedactError> {
        RedactionRegion::new(x, y, width, height)
    }

    #[test]
    fn test_password_field_region_maps_to_occlusion() -> Result<(), Box<dyn std::error::Error>> {
        let bounds = ImageDimensions::new(100, 80)?;
        let rules = [RedactionRule::password_field(region(4, 5, 20, 10)?)];
        let occlusions = resolve_occlusions(bounds, &rules)?;
        assert_eq!(occlusions.len(), 1);
        assert_eq!(
            occlusions.first(),
            Some(&OcclusionRectangle::new(4, 5, 20, 10))
        );
        Ok(())
    }

    #[test]
    fn test_matched_region_maps_to_occlusion() -> Result<(), Box<dyn std::error::Error>> {
        let bounds = ImageDimensions::new(100, 80)?;
        let rules = [RedactionRule::matched_region(region(1, 2, 3, 4)?)];
        let occlusions = resolve_occlusions(bounds, &rules)?;
        assert_eq!(
            occlusions.first(),
            Some(&OcclusionRectangle::new(1, 2, 3, 4))
        );
        Ok(())
    }

    #[test]
    fn test_empty_rules_fail_explicitly() -> Result<(), Box<dyn std::error::Error>> {
        let bounds = ImageDimensions::new(100, 80)?;
        assert_eq!(resolve_occlusions(bounds, &[]), Err(RedactError::NoRules));
        Ok(())
    }

    #[test]
    fn test_negative_coordinate_fails_explicitly() {
        assert_eq!(region(-1, 0, 1, 1), Err(RedactError::NegativeCoordinate));
        assert_eq!(region(0, -1, 1, 1), Err(RedactError::NegativeCoordinate));
    }

    #[test]
    fn test_zero_dimension_fails_explicitly() {
        assert_eq!(ImageDimensions::new(0, 1), Err(RedactError::ZeroDimension));
        assert_eq!(region(0, 0, 0, 1), Err(RedactError::ZeroDimension));
        assert_eq!(region(0, 0, 1, 0), Err(RedactError::ZeroDimension));
    }

    #[test]
    fn test_out_of_bounds_region_fails_without_partial_result()
    -> Result<(), Box<dyn std::error::Error>> {
        let bounds = ImageDimensions::new(10, 10)?;
        let rules = [RedactionRule::matched_region(region(9, 0, 2, 1)?)];
        assert_eq!(
            resolve_occlusions(bounds, &rules),
            Err(RedactError::RegionOutOfBounds)
        );
        Ok(())
    }

    #[test]
    fn test_coordinate_overflow_fails_explicitly() {
        assert_eq!(
            region(i64::MAX, 0, 1, 1),
            Err(RedactError::CoordinateOverflow)
        );
    }

    #[test]
    fn test_rule_limit_is_enforced() -> Result<(), Box<dyn std::error::Error>> {
        let bounds = ImageDimensions::new(10, 10)?;
        let rule = RedactionRule::matched_region(region(0, 0, 1, 1)?);
        let rules = vec![rule; MAX_REDACTION_RULES + 1];
        assert_eq!(
            resolve_occlusions(bounds, &rules),
            Err(RedactError::TooManyRules)
        );
        Ok(())
    }
}
