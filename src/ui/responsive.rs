//! Pure responsive layout policy for the Rustle UI.
//!
//! This module deliberately deals only in logical viewport geometry and UI
//! values. It does not know about application state, routes, messages, or
//! business data. View code can build a [`ResponsiveContext`] from iced's
//! layout size and pass the resulting tokens to the component that owns the
//! relevant interaction.

use iced::Size;

use crate::ui::theme;

/// Width of the reference design rectangle in logical pixels.
pub const REFERENCE_WIDTH: f32 = 1_920.0;
/// Height of the reference design rectangle in logical pixels.
pub const REFERENCE_HEIGHT: f32 = 1_080.0;

/// Lowest design density. The floor keeps compact layouts readable while
/// profile changes handle the loss of available composition space.
pub const DENSITY_FLOOR: f32 = 0.9;
/// Highest design density used by the reference/2K composition.
pub const DENSITY_CEILING: f32 = 4.0 / 3.0;

/// Compatibility name for the compact density floor used by the design docs.
pub const COMPACT_MIN_SCALE: f32 = DENSITY_FLOOR;
/// Compatibility name for the upper density bound used by the design docs.
pub const MAX_SCALE: f32 = DENSITY_CEILING;

/// The narrow profile begins below this logical width.
pub const NARROW_MAX_WIDTH: f32 = 640.0;
/// The tablet profile begins at this logical width when it is not narrow.
pub const TABLET_MIN_WIDTH: f32 = 640.0;
/// The compact profile begins at this logical width.
pub const COMPACT_MIN_WIDTH: f32 = 840.0;
/// The standard profile begins at this logical width.
pub const STANDARD_MIN_WIDTH: f32 = 1_120.0;
/// The expanded profile requires this logical width.
pub const EXPANDED_MIN_WIDTH: f32 = 1_440.0;
/// The expanded profile also requires enough height for desktop chrome.
pub const EXPANDED_MIN_HEIGHT: f32 = 820.0;
/// A portrait-like aspect ratio promotes a width-based profile to Tablet.
pub const TABLET_MAX_ASPECT_RATIO: f32 = 1.15;

/// Minimum width retained by the existing measured-content compatibility
/// helper.
pub const MIN_USABLE_CONTENT_WIDTH: f32 = 200.0;
/// Minimum hit target for an interactive control.
pub const MIN_INTERACTION_TARGET: f32 = 36.0;

/// A named composition profile selected from logical viewport geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutProfile {
    /// Full desktop lanes and expanded content composition.
    Expanded,
    /// Desktop composition with bounded content and fewer columns.
    Standard,
    /// Reduced chrome and compact desktop composition.
    Compact,
    /// Stacked content with drawer/rail-oriented chrome.
    Tablet,
    /// Single-column emergency composition with explicit overflow.
    Narrow,
}

impl LayoutProfile {
    /// Classify a logical viewport using the shared breakpoint policy.
    #[inline]
    pub fn from_viewport(viewport: Size) -> Self {
        classify_profile(viewport)
    }

    /// Whether this profile is intended for desktop lane composition.
    #[inline]
    pub const fn is_desktop(self) -> bool {
        matches!(self, Self::Expanded | Self::Standard)
    }

    /// Whether this profile needs a stacked or overflow-oriented composition.
    #[inline]
    pub const fn is_compact(self) -> bool {
        matches!(self, Self::Compact | Self::Tablet | Self::Narrow)
    }

    /// Whether this profile is narrow enough to require single-column fallbacks.
    #[inline]
    pub const fn is_narrow(self) -> bool {
        matches!(self, Self::Narrow)
    }

    /// Whether this profile uses a navigation drawer instead of the full tree.
    #[inline]
    pub const fn uses_navigation_drawer(self) -> bool {
        matches!(self, Self::Compact | Self::Tablet | Self::Narrow)
    }

    /// Whether the top bar needs a dedicated search row.
    #[inline]
    pub const fn uses_wrapped_top_bar(self) -> bool {
        matches!(self, Self::Tablet | Self::Narrow)
    }
}

/// Presentation selected for the application navigation surface.
///
/// The presentation is deliberately independent from route state. All three
/// variants consume the same navigation model and callbacks at the component
/// boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarPresentation {
    Full,
    Rail,
    Drawer,
    Hidden,
}

/// Select the navigation presentation for a profile and drawer state.
#[inline]
pub fn sidebar_presentation(profile: LayoutProfile, drawer_open: bool) -> SidebarPresentation {
    match profile {
        LayoutProfile::Expanded | LayoutProfile::Standard => SidebarPresentation::Full,
        LayoutProfile::Compact => {
            if drawer_open {
                SidebarPresentation::Drawer
            } else {
                SidebarPresentation::Rail
            }
        }
        LayoutProfile::Tablet => {
            if drawer_open {
                SidebarPresentation::Drawer
            } else {
                SidebarPresentation::Rail
            }
        }
        LayoutProfile::Narrow => {
            if drawer_open {
                SidebarPresentation::Drawer
            } else {
                SidebarPresentation::Hidden
            }
        }
    }
}

/// Return the finite, positive part of a logical dimension.
#[inline]
fn positive_dimension(value: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

/// Return a finite non-negative value for geometry calculations.
#[inline]
fn non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

/// Compute the aspect ratio of a logical viewport.
///
/// Degenerate or invalid dimensions return `0.0`, allowing the classifier to
/// choose a conservative compact profile instead of propagating `NaN` or an
/// infinite ratio into layout decisions.
#[inline]
pub fn viewport_aspect_ratio(viewport: Size) -> f32 {
    let width = positive_dimension(viewport.width);
    let height = positive_dimension(viewport.height);

    if width > 0.0 && height > 0.0 {
        width / height
    } else {
        0.0
    }
}

/// Compute the unclamped density against the 1920x1080 reference rectangle.
///
/// Invalid or degenerate viewports return `0.0`; [`DensityScale`] applies the
/// compact floor to that value.
#[inline]
pub fn raw_density(viewport: Size) -> f32 {
    let width = positive_dimension(viewport.width);
    let height = positive_dimension(viewport.height);

    if width > 0.0 && height > 0.0 {
        (width / REFERENCE_WIDTH).min(height / REFERENCE_HEIGHT)
    } else {
        0.0
    }
}

/// Select a layout profile from the logical viewport.
///
/// Width breakpoints are centralized here. The aspect-ratio check runs before
/// the width tiers so a portrait-like window cannot accidentally render a
/// desktop row merely because one axis is wide. Expanded is additionally
/// height-gated; narrower tiers intentionally remain usable in short windows.
#[inline]
pub fn classify_profile(viewport: Size) -> LayoutProfile {
    let width = positive_dimension(viewport.width);
    let height = positive_dimension(viewport.height);
    let aspect_ratio = viewport_aspect_ratio(viewport);

    if width < NARROW_MAX_WIDTH {
        LayoutProfile::Narrow
    } else if aspect_ratio < TABLET_MAX_ASPECT_RATIO {
        LayoutProfile::Tablet
    } else if width >= EXPANDED_MIN_WIDTH && height >= EXPANDED_MIN_HEIGHT {
        LayoutProfile::Expanded
    } else if width >= STANDARD_MIN_WIDTH {
        LayoutProfile::Standard
    } else if width >= COMPACT_MIN_WIDTH {
        LayoutProfile::Compact
    } else if width >= TABLET_MIN_WIDTH {
        LayoutProfile::Tablet
    } else {
        LayoutProfile::Narrow
    }
}

/// Bounded design density derived from a logical viewport.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct DensityScale(f32);

impl DensityScale {
    /// Clamp an arbitrary scale to the supported design-density range.
    ///
    /// Non-finite values are treated as invalid and resolve to the compact
    /// floor rather than poisoning every derived token with `NaN`.
    #[inline]
    pub fn new(scale: f32) -> Self {
        let scale = if scale.is_finite() {
            scale
        } else {
            DENSITY_FLOOR
        };

        Self(scale.clamp(DENSITY_FLOOR, DENSITY_CEILING))
    }

    /// Derive a scale from the logical viewport and the reference rectangle.
    #[inline]
    pub fn from_viewport(viewport: Size) -> Self {
        Self::new(raw_density(viewport))
    }

    /// Return the scale as a primitive value.
    #[inline]
    pub fn value(self) -> f32 {
        self.0
    }

    /// Alias useful at numeric API boundaries.
    #[inline]
    pub fn as_f32(self) -> f32 {
        self.0
    }

    /// Scale a finite value by this density.
    #[inline]
    pub fn scale(self, value: f32) -> f32 {
        if value.is_finite() {
            value * self.0
        } else {
            0.0
        }
    }
}

impl Default for DensityScale {
    #[inline]
    fn default() -> Self {
        Self::new(1.0)
    }
}

impl From<DensityScale> for f32 {
    #[inline]
    fn from(scale: DensityScale) -> Self {
        scale.value()
    }
}

/// Immutable responsive inputs and semantic UI tokens for one layout pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResponsiveContext {
    /// Logical size supplied by iced's layout system.
    pub viewport: Size,
    /// Width divided by height, or `0.0` for invalid/degenerate geometry.
    pub aspect_ratio: f32,
    /// Named composition profile for this viewport.
    pub profile: LayoutProfile,
    /// Bounded design density for Rustle-owned UI dimensions.
    pub density: DensityScale,
    /// Scaled semantic UI values.
    pub tokens: UiTokens,
}

impl ResponsiveContext {
    /// Build a context from iced's logical layout size.
    #[inline]
    pub fn new(viewport: Size) -> Self {
        let density = DensityScale::from_viewport(viewport);

        Self {
            viewport,
            aspect_ratio: viewport_aspect_ratio(viewport),
            profile: classify_profile(viewport),
            density,
            tokens: UiTokens::new(density),
        }
    }

    /// Alias that makes the logical-size boundary explicit at call sites.
    #[inline]
    pub fn from_viewport(viewport: Size) -> Self {
        Self::new(viewport)
    }

    /// Return a sanitized logical width for pure geometry policies.
    #[inline]
    pub fn width(&self) -> f32 {
        positive_dimension(self.viewport.width)
    }

    /// Return a sanitized logical height for pure geometry policies.
    #[inline]
    pub fn height(&self) -> f32 {
        positive_dimension(self.viewport.height)
    }

    /// Derive usable content width through the shared measured-content policy.
    #[inline]
    pub fn usable_content_width(&self, horizontal_padding: f32) -> f32 {
        usable_content_width(self.viewport, horizontal_padding)
    }

    /// Calculate token-aware grid columns for a measured content width.
    #[inline]
    pub fn grid_columns(
        &self,
        content_width: f32,
        base_card_width: f32,
        base_spacing: f32,
        max_columns: usize,
    ) -> usize {
        calculate_grid_columns_clamped(
            content_width,
            self.tokens.size(base_card_width),
            self.tokens.space(base_spacing),
            max_columns,
        )
    }
}

impl From<Size> for ResponsiveContext {
    #[inline]
    fn from(viewport: Size) -> Self {
        Self::new(viewport)
    }
}

/// Semantic spacing roles used by [`UiTokens`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpacingRole {
    ExtraSmall,
    Small,
    Medium,
    Large,
    ExtraLarge,
    Section,
    Page,
}

/// Short alias for [`SpacingRole`] at call sites that use `space` terminology.
pub type SpaceRole = SpacingRole;

impl SpacingRole {
    #[inline]
    fn reference(self) -> f32 {
        match self {
            Self::ExtraSmall => 4.0,
            Self::Small => 8.0,
            Self::Medium => 12.0,
            Self::Large => 16.0,
            Self::ExtraLarge => 24.0,
            Self::Section => 32.0,
            Self::Page => 40.0,
        }
    }
}

/// Semantic typography roles used by [`UiTokens`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRole {
    Micro,
    Caption,
    Label,
    Body,
    BodyLarge,
    Subtitle,
    Title,
    TitleLarge,
    Hero,
    Display,
    DisplayLarge,
}

impl TextRole {
    #[inline]
    fn reference(self) -> (f32, f32) {
        match self {
            Self::Micro => (theme::TEXT_SIZE_MICRO, 10.0),
            Self::Caption => (theme::TEXT_SIZE_CAPTION, 11.0),
            Self::Label => (theme::TEXT_SIZE_LABEL, 12.0),
            Self::Body => (theme::TEXT_SIZE_BODY, 13.0),
            Self::BodyLarge => (theme::TEXT_SIZE_BODY_LARGE, 14.0),
            Self::Subtitle => (theme::TEXT_SIZE_SUBTITLE, 16.0),
            Self::Title => (theme::TEXT_SIZE_TITLE, 20.0),
            Self::TitleLarge => (theme::TEXT_SIZE_TITLE_LARGE, 24.0),
            Self::Hero => (theme::TEXT_SIZE_HERO, 24.0),
            Self::Display => (theme::TEXT_SIZE_DISPLAY, 32.0),
            Self::DisplayLarge => (theme::TEXT_SIZE_DISPLAY_LARGE, 48.0),
        }
    }
}

/// Semantic icon-size roles used by [`UiTokens`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconRole {
    Small,
    Medium,
    Large,
    WindowControl,
    Hero,
}

impl IconRole {
    #[inline]
    fn reference(self) -> (f32, f32) {
        match self {
            Self::Small => (16.0, 14.0),
            Self::Medium => (20.0, 18.0),
            Self::Large => (24.0, 20.0),
            Self::WindowControl => (15.0, 14.0),
            Self::Hero => (32.0, 24.0),
        }
    }
}

/// Semantic interaction-target roles used by [`UiTokens`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetRole {
    Icon,
    Control,
    Row,
    WindowControl,
}

impl TargetRole {
    #[inline]
    fn reference(self) -> (f32, f32) {
        match self {
            Self::Icon => (36.0, MIN_INTERACTION_TARGET),
            Self::Control => (40.0, MIN_INTERACTION_TARGET),
            Self::Row => (44.0, MIN_INTERACTION_TARGET),
            Self::WindowControl => (36.0, MIN_INTERACTION_TARGET),
        }
    }
}

/// Semantic surface-radius roles used by [`UiTokens`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadiusRole {
    Small,
    Medium,
    Large,
    Pill,
}

impl RadiusRole {
    #[inline]
    fn reference(self) -> f32 {
        match self {
            Self::Small => 4.0,
            Self::Medium => 8.0,
            Self::Large => 16.0,
            Self::Pill => 24.0,
        }
    }
}

/// Shared card dimension roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardRole {
    Playlist,
    Detail,
    Feature,
}

/// Scaled geometry for a card family.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CardMetrics {
    pub width: f32,
    pub height: f32,
    pub gap: f32,
    pub radius: f32,
}

impl CardRole {
    #[inline]
    fn reference(self) -> CardMetrics {
        match self {
            Self::Playlist => CardMetrics {
                width: 161.0,
                height: 217.0,
                gap: 24.0,
                radius: 10.0,
            },
            Self::Detail => CardMetrics {
                width: 200.0,
                height: 200.0,
                gap: 20.0,
                radius: 16.0,
            },
            Self::Feature => CardMetrics {
                width: 320.0,
                height: 180.0,
                gap: 24.0,
                radius: 14.0,
            },
        }
    }
}

/// Shared application-chrome dimension roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeRole {
    TopBar,
    PlayerBar,
    Sidebar,
    SidebarRail,
    ResizeHandle,
}

impl ChromeRole {
    #[inline]
    fn reference(self) -> f32 {
        match self {
            Self::TopBar => theme::TOP_BAR_HEIGHT,
            Self::PlayerBar => 80.0,
            Self::Sidebar => 280.0,
            Self::SidebarRail => 68.0,
            Self::ResizeHandle => 6.0,
        }
    }
}

/// Return the top-bar height for the current composition profile.
#[inline]
pub fn top_bar_height(context: &ResponsiveContext) -> f32 {
    let base = if context.profile.uses_wrapped_top_bar() {
        104.0
    } else {
        ChromeRole::TopBar.reference()
    };
    context.tokens.size(base)
}

/// Immutable semantic UI dimensions derived from a [`DensityScale`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiTokens {
    density: DensityScale,
}

impl UiTokens {
    /// Create tokens for a bounded density.
    #[inline]
    pub fn new(density: DensityScale) -> Self {
        Self { density }
    }

    /// Return the density that produced these tokens.
    #[inline]
    pub fn density(&self) -> DensityScale {
        self.density
    }

    /// Scale an arbitrary spacing value, treating invalid values as zero.
    #[inline]
    pub fn space(&self, base: f32) -> f32 {
        non_negative(self.density.scale(base))
    }

    /// Alias for callers that use generic size terminology.
    #[inline]
    pub fn size(&self, base: f32) -> f32 {
        self.space(base)
    }

    /// Resolve a named spacing role.
    #[inline]
    pub fn spacing(&self, role: SpacingRole) -> f32 {
        self.space(role.reference())
    }

    /// Resolve a named typography role with its readability floor.
    #[inline]
    pub fn text(&self, role: TextRole) -> f32 {
        let (base, floor) = role.reference();
        self.density.scale(base).max(floor)
    }

    /// Resolve a named icon role with its minimum raster size.
    #[inline]
    pub fn icon(&self, role: IconRole) -> f32 {
        let (base, floor) = role.reference();
        self.density.scale(base).max(floor)
    }

    /// Resolve a named interaction target while preserving the global minimum.
    #[inline]
    pub fn target(&self, role: TargetRole) -> f32 {
        let (base, floor) = role.reference();
        self.density.scale(base).max(floor)
    }

    /// Resolve a square interaction-target size.
    #[inline]
    pub fn target_size(&self, role: TargetRole) -> Size {
        let target = self.target(role);
        Size::new(target, target)
    }

    /// Resolve a named surface radius.
    #[inline]
    pub fn radius(&self, role: RadiusRole) -> f32 {
        self.space(role.reference())
    }

    /// Resolve shared card geometry.
    #[inline]
    pub fn card(&self, role: CardRole) -> CardMetrics {
        let reference = role.reference();
        CardMetrics {
            width: self.size(reference.width),
            height: self.size(reference.height),
            gap: self.space(reference.gap),
            radius: self.size(reference.radius),
        }
    }

    /// Resolve a shared chrome dimension.
    #[inline]
    pub fn chrome(&self, role: ChromeRole) -> f32 {
        self.size(role.reference())
    }
}

impl Default for UiTokens {
    #[inline]
    fn default() -> Self {
        Self::new(DensityScale::default())
    }
}

/// Clamp a preferred width to a viewport while reserving one gutter on both
/// sides. Invalid inputs resolve to zero.
#[inline]
pub fn bounded_width(preferred: f32, viewport_width: f32, gutter: f32) -> f32 {
    let preferred = non_negative(preferred);
    let available = (positive_dimension(viewport_width) - 2.0 * non_negative(gutter)).max(0.0);
    preferred.min(available)
}

/// Clamp a preferred height to a viewport while reserving one gutter on both
/// sides. Invalid inputs resolve to zero.
#[inline]
pub fn bounded_height(preferred: f32, viewport_height: f32, gutter: f32) -> f32 {
    let preferred = non_negative(preferred);
    let available = (positive_dimension(viewport_height) - 2.0 * non_negative(gutter)).max(0.0);
    preferred.min(available)
}

/// Clamp a panel to a viewport, reserving chrome from the available height.
#[inline]
pub fn bounded_panel_size(
    preferred: Size,
    viewport: Size,
    horizontal_gutter: f32,
    vertical_gutter: f32,
    reserved_height: f32,
) -> Size {
    let available_height =
        (positive_dimension(viewport.height) - non_negative(reserved_height)).max(0.0);

    Size::new(
        bounded_width(preferred.width, viewport.width, horizontal_gutter),
        bounded_height(preferred.height, available_height, vertical_gutter),
    )
}

/// Derive usable content width while retaining the existing 200px floor.
#[inline]
pub fn usable_content_width(size: Size, horizontal_padding: f32) -> f32 {
    (positive_dimension(size.width) - non_negative(horizontal_padding))
        .max(MIN_USABLE_CONTENT_WIDTH)
}

/// Calculate how many complete cards fit into a content width.
#[inline]
pub fn calculate_grid_columns(content_width: f32, card_width: f32, spacing: f32) -> usize {
    let content_width = positive_dimension(content_width);
    let card_width = positive_dimension(card_width);
    let spacing = non_negative(spacing);

    if content_width <= 0.0 || card_width <= 0.0 {
        return 1;
    }

    let denominator = card_width + spacing;
    let numerator = content_width + spacing;
    if !denominator.is_finite() || !numerator.is_finite() || denominator <= 0.0 {
        return 1;
    }

    (numerator / denominator).floor().max(1.0) as usize
}

/// Calculate grid columns and clamp the result to a positive maximum.
#[inline]
pub fn calculate_grid_columns_clamped(
    content_width: f32,
    card_width: f32,
    spacing: f32,
    max_columns: usize,
) -> usize {
    calculate_grid_columns(content_width, card_width, spacing).clamp(1, max_columns.max(1))
}

/// Adaptive arrangement selected when a row's minimum width cannot fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowVariant {
    /// All children can remain on one line.
    Inline,
    /// Children can wrap while retaining their row-level semantics.
    Wrap,
    /// Children should be arranged vertically.
    Stack,
    /// Children remain in a horizontally scrollable surface.
    Scroll,
}

/// Select a generic overflow arrangement from profile and geometry.
///
/// The helper only chooses composition. The caller remains responsible for
/// building the corresponding iced row, column, or scrollable and for keeping
/// the same actions and stable widget IDs in every variant.
#[inline]
pub fn select_overflow_variant(
    profile: LayoutProfile,
    available_width: f32,
    required_width: f32,
) -> OverflowVariant {
    if non_negative(required_width) <= positive_dimension(available_width) {
        OverflowVariant::Inline
    } else {
        match profile {
            LayoutProfile::Expanded | LayoutProfile::Standard | LayoutProfile::Compact => {
                OverflowVariant::Wrap
            }
            LayoutProfile::Tablet => OverflowVariant::Stack,
            LayoutProfile::Narrow => OverflowVariant::Scroll,
        }
    }
}

/// Enforce the minimum size of an interactive hit target.
#[inline]
pub fn minimum_hit_target(candidate: f32) -> f32 {
    if candidate.is_finite() {
        candidate.max(MIN_INTERACTION_TARGET)
    } else {
        MIN_INTERACTION_TARGET
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_approx(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
    }

    #[test]
    fn density_matches_reference_and_2k_viewports() {
        assert_approx(
            DensityScale::from_viewport(Size::new(1_920.0, 1_080.0)).value(),
            1.0,
        );
        assert_approx(
            DensityScale::from_viewport(Size::new(2_560.0, 1_440.0)).value(),
            4.0 / 3.0,
        );
    }

    #[test]
    fn density_is_clamped_and_invalid_values_are_safe() {
        assert_eq!(DensityScale::new(0.1).value(), DENSITY_FLOOR);
        assert_eq!(DensityScale::new(10.0).value(), DENSITY_CEILING);
        assert_eq!(DensityScale::new(f32::NAN).value(), DENSITY_FLOOR);
        assert_eq!(
            DensityScale::from_viewport(Size::new(0.0, 0.0)).value(),
            DENSITY_FLOOR
        );
        assert_eq!(raw_density(Size::new(f32::INFINITY, 1_080.0)), 0.0);
    }

    #[test]
    fn profile_boundaries_are_centralized_and_deterministic() {
        assert_eq!(
            classify_profile(Size::new(1_440.0, 820.0)),
            LayoutProfile::Expanded
        );
        assert_eq!(
            classify_profile(Size::new(1_439.0, 820.0)),
            LayoutProfile::Standard
        );
        assert_eq!(
            classify_profile(Size::new(1_120.0, 700.0)),
            LayoutProfile::Standard
        );
        assert_eq!(
            classify_profile(Size::new(1_119.0, 700.0)),
            LayoutProfile::Compact
        );
        assert_eq!(
            classify_profile(Size::new(840.0, 720.0)),
            LayoutProfile::Compact
        );
        assert_eq!(
            classify_profile(Size::new(839.0, 720.0)),
            LayoutProfile::Tablet
        );
        assert_eq!(
            classify_profile(Size::new(640.0, 400.0)),
            LayoutProfile::Tablet
        );
        assert_eq!(
            classify_profile(Size::new(639.0, 1_024.0)),
            LayoutProfile::Narrow
        );
    }

    #[test]
    fn portrait_aspect_promotes_wide_viewports_to_tablet() {
        assert_eq!(
            classify_profile(Size::new(1_200.0, 1_400.0)),
            LayoutProfile::Tablet
        );
        assert_eq!(
            classify_profile(Size::new(1_200.0, 1_043.0)),
            LayoutProfile::Standard
        );
    }

    #[test]
    fn context_contains_derived_geometry_and_2k_tokens() {
        let context = ResponsiveContext::new(Size::new(2_560.0, 1_440.0));

        assert_eq!(context.profile, LayoutProfile::Expanded);
        assert_approx(context.aspect_ratio, 16.0 / 9.0);
        assert_approx(context.tokens.space(16.0), 64.0 / 3.0);
        assert_approx(context.tokens.text(TextRole::Body), 56.0 / 3.0);
    }

    #[test]
    fn token_floors_keep_compact_controls_and_text_readable() {
        let tokens = UiTokens::new(DensityScale::new(0.1));

        assert!(tokens.target(TargetRole::Icon) >= MIN_INTERACTION_TARGET);
        assert!(tokens.target(TargetRole::Row) >= MIN_INTERACTION_TARGET);
        assert!(tokens.text(TextRole::Caption) >= 11.0);
        assert!(tokens.icon(IconRole::Small) >= 14.0);
        assert_eq!(minimum_hit_target(f32::NAN), MIN_INTERACTION_TARGET);
        assert_eq!(minimum_hit_target(12.0), MIN_INTERACTION_TARGET);
    }

    #[test]
    fn bounded_geometry_never_exceeds_the_available_viewport() {
        assert_eq!(bounded_width(360.0, 800.0, 16.0), 360.0);
        assert_eq!(bounded_width(600.0, 400.0, 16.0), 368.0);
        assert_eq!(bounded_height(400.0, 300.0, 12.0), 276.0);

        assert_eq!(
            bounded_panel_size(
                Size::new(600.0, 400.0),
                Size::new(400.0, 300.0),
                16.0,
                12.0,
                40.0,
            ),
            Size::new(368.0, 236.0)
        );
        assert_eq!(bounded_width(f32::NAN, 400.0, 16.0), 0.0);
        assert_eq!(bounded_height(400.0, -1.0, 12.0), 0.0);
    }

    #[test]
    fn grid_math_handles_complete_columns_and_invalid_values() {
        assert_eq!(calculate_grid_columns(160.0, 160.0, 24.0), 1);
        assert_eq!(calculate_grid_columns(528.0, 160.0, 24.0), 3);
        assert_eq!(calculate_grid_columns(-1.0, 160.0, 24.0), 1);
        assert_eq!(calculate_grid_columns(500.0, 0.0, 24.0), 1);
        assert_eq!(calculate_grid_columns(f32::NAN, 160.0, 24.0), 1);
        assert_eq!(calculate_grid_columns_clamped(4_000.0, 160.0, 24.0, 5), 5);
    }

    #[test]
    fn overflow_policy_changes_composition_before_clipping() {
        assert_eq!(
            select_overflow_variant(LayoutProfile::Standard, 400.0, 360.0),
            OverflowVariant::Inline
        );
        assert_eq!(
            select_overflow_variant(LayoutProfile::Compact, 300.0, 360.0),
            OverflowVariant::Wrap
        );
        assert_eq!(
            select_overflow_variant(LayoutProfile::Tablet, 300.0, 360.0),
            OverflowVariant::Stack
        );
        assert_eq!(
            select_overflow_variant(LayoutProfile::Narrow, 300.0, 360.0),
            OverflowVariant::Scroll
        );
    }

    #[test]
    fn validation_fixtures_cover_resize_profile_transitions() {
        let fixtures = [
            (Size::new(1_920.0, 1_080.0), LayoutProfile::Expanded),
            (Size::new(2_560.0, 1_440.0), LayoutProfile::Expanded),
            (Size::new(960.0, 1_080.0), LayoutProfile::Tablet),
            (Size::new(768.0, 1_024.0), LayoutProfile::Tablet),
            (Size::new(720.0, 800.0), LayoutProfile::Tablet),
            (Size::new(960.0, 540.0), LayoutProfile::Compact),
            (Size::new(560.0, 800.0), LayoutProfile::Narrow),
        ];

        for (viewport, expected_profile) in fixtures {
            let context = ResponsiveContext::from_viewport(viewport);
            assert_eq!(context.profile, expected_profile);
            assert!(context.tokens.target(TargetRole::Control) >= MIN_INTERACTION_TARGET);
        }

        let transition = [
            Size::new(1_920.0, 1_080.0),
            Size::new(1_119.0, 700.0),
            Size::new(960.0, 540.0),
            Size::new(768.0, 1_024.0),
            Size::new(560.0, 800.0),
            Size::new(768.0, 1_024.0),
            Size::new(1_920.0, 1_080.0),
        ];
        let profiles: Vec<_> = transition
            .into_iter()
            .map(ResponsiveContext::from_viewport)
            .map(|context| context.profile)
            .collect();

        assert_eq!(
            profiles,
            vec![
                LayoutProfile::Expanded,
                LayoutProfile::Compact,
                LayoutProfile::Compact,
                LayoutProfile::Tablet,
                LayoutProfile::Narrow,
                LayoutProfile::Tablet,
                LayoutProfile::Expanded,
            ]
        );
    }

    #[test]
    fn drawer_presentation_keeps_navigation_available_in_compact_profiles() {
        assert_eq!(
            sidebar_presentation(LayoutProfile::Expanded, false),
            SidebarPresentation::Full
        );
        assert_eq!(
            sidebar_presentation(LayoutProfile::Compact, false),
            SidebarPresentation::Rail
        );
        assert_eq!(
            sidebar_presentation(LayoutProfile::Compact, true),
            SidebarPresentation::Drawer
        );
        assert_eq!(
            sidebar_presentation(LayoutProfile::Tablet, true),
            SidebarPresentation::Drawer
        );
        assert_eq!(
            sidebar_presentation(LayoutProfile::Narrow, false),
            SidebarPresentation::Hidden
        );
        assert_eq!(
            sidebar_presentation(LayoutProfile::Narrow, true),
            SidebarPresentation::Drawer
        );
    }
}
