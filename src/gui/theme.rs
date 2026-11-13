use gpui::{App, Hsla, Pixels, hsla, px};
use gpui_component::theme::{Theme, ThemeMode};

/// Custom colors and layout metrics for Snarta GUI.
pub const SIDEBAR_WIDTH: Pixels = px(260.0);
pub const TOPBAR_HEIGHT: Pixels = px(44.0);
#[allow(dead_code)]
pub const INPUT_MIN_HEIGHT: Pixels = px(44.0);
#[allow(dead_code)]
pub const INPUT_MAX_HEIGHT: Pixels = px(200.0);

/// Graphite base palette constants (neutral warm charcoal).
pub const COLOR_GRAPHITE_SIDEBAR: Hsla = hsla(30.0 / 360.0, 0.04, 0.065, 1.0);
pub const COLOR_GRAPHITE_CANVAS: Hsla = hsla(30.0 / 360.0, 0.03, 0.085, 1.0);
pub const COLOR_GRAPHITE_ELEVATED: Hsla = hsla(30.0 / 360.0, 0.04, 0.125, 1.0);
pub const COLOR_GRAPHITE_INPUT: Hsla = hsla(30.0 / 360.0, 0.03, 0.10, 1.0);
pub const COLOR_GRAPHITE_BORDER: Hsla = hsla(30.0 / 360.0, 0.04, 0.18, 1.0);
pub const COLOR_GRAPHITE_BORDER_SUBTLE: Hsla = hsla(30.0 / 360.0, 0.03, 0.14, 1.0);

/// Neutral Warm Stone / Sand accent constants (replaces pink, non-blue).
pub const COLOR_PINK_PRIMARY: Hsla = hsla(34.0 / 360.0, 0.28, 0.68, 1.0);
pub const COLOR_PINK_HOVER: Hsla = hsla(34.0 / 360.0, 0.34, 0.75, 1.0);
pub const COLOR_PINK_ACTIVE: Hsla = hsla(34.0 / 360.0, 0.25, 0.58, 1.0);
pub const COLOR_PINK_MUTED_BG: Hsla = hsla(34.0 / 360.0, 0.16, 0.15, 1.0);
pub const COLOR_PINK_FOREGROUND_ACCENT: Hsla = hsla(34.0 / 360.0, 0.38, 0.84, 1.0);
pub const COLOR_PINK_SELECTION: Hsla = hsla(34.0 / 360.0, 0.28, 0.65, 0.28);

/// Neutral typography constants.
pub const COLOR_TEXT_PRIMARY: Hsla = hsla(34.0 / 360.0, 0.06, 0.94, 1.0);
pub const COLOR_TEXT_MUTED: Hsla = hsla(34.0 / 360.0, 0.04, 0.62, 1.0);
pub const COLOR_TEXT_ON_PINK: Hsla = hsla(30.0 / 360.0, 0.08, 0.08, 1.0);

/// Apply the pure dark graphite & neon pink accent theme to the application.
/// White theme is explicitly excluded (both light and dark configs are locked to graphite & pink).
pub fn apply_graphite_pink_theme(cx: &mut App) {
    // 1. Switch mode to Dark first so default dark config is initialized.
    Theme::change(ThemeMode::Dark, None, cx);

    // 2. Secondary graphite button states
    let sec_btn_bg = hsla(240.0 / 360.0, 0.05, 0.14, 1.0);
    let sec_btn_hover = hsla(240.0 / 360.0, 0.05, 0.19, 1.0);
    let sec_btn_active = hsla(240.0 / 360.0, 0.05, 0.11, 1.0);

    // 3. Update theme tokens and lock out light theme
    Theme::update(cx, |theme| {
        theme.mode = ThemeMode::Dark;

        let c = &mut theme.colors;

        // Graphite surfaces
        c.background = COLOR_GRAPHITE_CANVAS;
        c.sidebar = COLOR_GRAPHITE_SIDEBAR;
        c.sidebar_border = COLOR_GRAPHITE_BORDER;
        c.sidebar_foreground = COLOR_TEXT_PRIMARY;
        c.sidebar_accent = COLOR_PINK_MUTED_BG;
        c.sidebar_accent_foreground = COLOR_PINK_FOREGROUND_ACCENT;
        c.sidebar_primary = COLOR_PINK_PRIMARY;
        c.sidebar_primary_foreground = COLOR_TEXT_ON_PINK;

        c.muted = COLOR_GRAPHITE_ELEVATED;
        c.muted_foreground = COLOR_TEXT_MUTED;
        c.border = COLOR_GRAPHITE_BORDER;
        c.input = COLOR_GRAPHITE_INPUT;
        c.popover = COLOR_GRAPHITE_ELEVATED;
        c.popover_foreground = COLOR_TEXT_PRIMARY;
        c.accordion = COLOR_GRAPHITE_ELEVATED;
        c.group_box = COLOR_GRAPHITE_ELEVATED;
        c.group_box_foreground = COLOR_TEXT_PRIMARY;
        c.table = COLOR_GRAPHITE_ELEVATED;
        c.table_active = COLOR_PINK_MUTED_BG;
        c.table_active_border = COLOR_PINK_PRIMARY;
        c.table_even = COLOR_GRAPHITE_CANVAS;
        c.table_head = COLOR_GRAPHITE_SIDEBAR;
        c.table_head_foreground = COLOR_TEXT_MUTED;
        c.table_foot = COLOR_GRAPHITE_SIDEBAR;
        c.table_foot_foreground = COLOR_TEXT_MUTED;
        c.table_hover = COLOR_PINK_MUTED_BG;
        c.table_row_border = COLOR_GRAPHITE_BORDER_SUBTLE;
        c.list = COLOR_GRAPHITE_ELEVATED;
        c.list_active = COLOR_PINK_MUTED_BG;
        c.list_active_border = COLOR_PINK_PRIMARY;
        c.list_even = COLOR_GRAPHITE_CANVAS;
        c.list_head = COLOR_GRAPHITE_SIDEBAR;
        c.list_hover = COLOR_PINK_MUTED_BG;
        c.title_bar = COLOR_GRAPHITE_SIDEBAR;
        c.title_bar_border = COLOR_GRAPHITE_BORDER;
        c.status_bar = COLOR_GRAPHITE_SIDEBAR;
        c.status_bar_border = COLOR_GRAPHITE_BORDER;
        c.window_border = COLOR_GRAPHITE_BORDER;
        c.overlay = hsla(240.0 / 360.0, 0.10, 0.04, 0.75);

        // Typography & General
        c.foreground = COLOR_TEXT_PRIMARY;

        // Neon pink primary accents
        c.primary = COLOR_PINK_PRIMARY;
        c.primary_hover = COLOR_PINK_HOVER;
        c.primary_active = COLOR_PINK_ACTIVE;
        c.primary_foreground = COLOR_TEXT_ON_PINK;
        c.accent = COLOR_PINK_MUTED_BG;
        c.accent_foreground = COLOR_PINK_FOREGROUND_ACCENT;

        // Primary buttons
        c.button_primary = COLOR_PINK_PRIMARY;
        c.button_primary_hover = COLOR_PINK_HOVER;
        c.button_primary_active = COLOR_PINK_ACTIVE;
        c.button_primary_foreground = COLOR_TEXT_ON_PINK;

        // Secondary buttons (graphite)
        c.button_secondary = sec_btn_bg;
        c.button_secondary_hover = sec_btn_hover;
        c.button_secondary_active = sec_btn_active;
        c.button_secondary_foreground = COLOR_TEXT_PRIMARY;
        c.secondary = sec_btn_bg;
        c.secondary_hover = sec_btn_hover;
        c.secondary_active = sec_btn_active;
        c.secondary_foreground = COLOR_TEXT_PRIMARY;

        // Ghost / default buttons
        c.button = hsla(0.0, 0.0, 0.0, 0.0);
        c.button_hover = COLOR_GRAPHITE_ELEVATED;
        c.button_active = COLOR_GRAPHITE_INPUT;
        c.button_foreground = COLOR_TEXT_PRIMARY;

        // Interactive controls
        c.caret = COLOR_PINK_PRIMARY;
        c.ring = COLOR_PINK_PRIMARY;
        c.selection = COLOR_PINK_SELECTION;
        c.progress_bar = COLOR_PINK_PRIMARY;
        c.slider_bar = COLOR_PINK_PRIMARY;
        c.slider_thumb = COLOR_PINK_HOVER;
        c.switch = COLOR_PINK_PRIMARY;
        c.switch_thumb = COLOR_TEXT_ON_PINK;
        c.link = COLOR_PINK_HOVER;
        c.link_hover = COLOR_PINK_FOREGROUND_ACCENT;
        c.link_active = COLOR_PINK_ACTIVE;

        // Tabs
        c.tab = COLOR_GRAPHITE_ELEVATED;
        c.tab_active = COLOR_GRAPHITE_CANVAS;
        c.tab_active_foreground = COLOR_PINK_PRIMARY;
        c.tab_bar = COLOR_GRAPHITE_SIDEBAR;
        c.tab_bar_segmented = COLOR_GRAPHITE_INPUT;
        c.tab_foreground = COLOR_TEXT_MUTED;

        // Scrollbar
        c.scrollbar = hsla(0.0, 0.0, 0.0, 0.0);
        c.scrollbar_thumb = hsla(240.0 / 360.0, 0.05, 0.25, 0.8);
        c.scrollbar_thumb_hover = COLOR_PINK_HOVER;

        // Danger (crimson red)
        c.danger = hsla(0.0, 0.84, 0.60, 1.0);
        c.danger_hover = hsla(0.0, 0.90, 0.68, 1.0);
        c.danger_active = hsla(0.0, 0.85, 0.50, 1.0);
        c.danger_foreground = COLOR_TEXT_ON_PINK;
        c.button_danger = hsla(0.0, 0.84, 0.60, 1.0);
        c.button_danger_hover = hsla(0.0, 0.90, 0.68, 1.0);
        c.button_danger_active = hsla(0.0, 0.85, 0.50, 1.0);
        c.button_danger_foreground = COLOR_TEXT_ON_PINK;

        // Info (cool cyan)
        c.info = hsla(190.0 / 360.0, 0.90, 0.55, 1.0);
        c.info_hover = hsla(190.0 / 360.0, 0.95, 0.65, 1.0);
        c.info_active = hsla(190.0 / 360.0, 0.90, 0.45, 1.0);
        c.info_foreground = COLOR_TEXT_ON_PINK;
        c.button_info = hsla(190.0 / 360.0, 0.90, 0.55, 1.0);
        c.button_info_hover = hsla(190.0 / 360.0, 0.95, 0.65, 1.0);
        c.button_info_active = hsla(190.0 / 360.0, 0.90, 0.45, 1.0);
        c.button_info_foreground = COLOR_TEXT_ON_PINK;

        // Success (emerald)
        c.success = hsla(150.0 / 360.0, 0.70, 0.50, 1.0);
        c.success_hover = hsla(150.0 / 360.0, 0.75, 0.60, 1.0);
        c.success_active = hsla(150.0 / 360.0, 0.70, 0.40, 1.0);
        c.success_foreground = COLOR_TEXT_ON_PINK;
        c.button_success = hsla(150.0 / 360.0, 0.70, 0.50, 1.0);
        c.button_success_hover = hsla(150.0 / 360.0, 0.75, 0.60, 1.0);
        c.button_success_active = hsla(150.0 / 360.0, 0.70, 0.40, 1.0);
        c.button_success_foreground = COLOR_TEXT_ON_PINK;

        // Warning (amber)
        c.warning = hsla(38.0 / 360.0, 0.92, 0.50, 1.0);
        c.warning_hover = hsla(38.0 / 360.0, 0.95, 0.60, 1.0);
        c.warning_active = hsla(38.0 / 360.0, 0.92, 0.40, 1.0);
        c.warning_foreground = COLOR_TEXT_ON_PINK;
        c.button_warning = hsla(38.0 / 360.0, 0.92, 0.50, 1.0);
        c.button_warning_hover = hsla(38.0 / 360.0, 0.95, 0.60, 1.0);
        c.button_warning_active = hsla(38.0 / 360.0, 0.92, 0.40, 1.0);
        c.button_warning_foreground = COLOR_TEXT_ON_PINK;

        // Other tokens
        c.description_list_label = COLOR_GRAPHITE_SIDEBAR;
        c.description_list_label_foreground = COLOR_TEXT_MUTED;
        c.drag_border = COLOR_PINK_PRIMARY;
        c.drop_target = COLOR_PINK_MUTED_BG;
        c.skeleton = hsla(240.0 / 360.0, 0.05, 0.16, 1.0);
        c.chart_1 = COLOR_PINK_PRIMARY;
        c.chart_2 = COLOR_PINK_HOVER;
        c.chart_3 = hsla(280.0 / 360.0, 0.75, 0.65, 1.0);
        c.chart_4 = hsla(190.0 / 360.0, 0.85, 0.55, 1.0);
        c.chart_5 = hsla(150.0 / 360.0, 0.70, 0.50, 1.0);
        c.chart_bullish = hsla(150.0 / 360.0, 0.70, 0.50, 1.0);
        c.chart_bearish = hsla(0.0, 0.84, 0.60, 1.0);
        c.chart_grid = COLOR_GRAPHITE_BORDER_SUBTLE;

        // Lock out light theme: even if system appearance triggers light mode,
        // it falls back to this identical dark graphite + pink configuration.
        theme.light_theme = theme.dark_theme.clone();
    });
}

/// VRAM utilization color helper.
#[inline]
pub fn vram_status_color(used_mib: u64, total_mib: u64, spill_mib: u64) -> Hsla {
    if spill_mib > 0 {
        // Red: spilling into shared system RAM
        hsla(0.0, 0.84, 0.60, 1.0)
    } else if total_mib == 0 {
        hsla(0.0, 0.0, 0.60, 1.0)
    } else {
        let pct = (used_mib as f32 / total_mib as f32) * 100.0;
        if pct > 90.0 {
            // Red: almost full
            hsla(0.0, 0.84, 0.60, 1.0)
        } else if pct > 75.0 {
            // Yellow: high load
            hsla(0.12, 0.85, 0.55, 1.0)
        } else {
            // Green: comfortable headroom
            hsla(0.38, 0.65, 0.50, 1.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn vram_status_color_thresholds() {
        let green = vram_status_color(8000, 16000, 0); // 50%
        assert_eq!(green, hsla(0.38, 0.65, 0.50, 1.0));

        let yellow = vram_status_color(13000, 16000, 0); // 81.25%
        assert_eq!(yellow, hsla(0.12, 0.85, 0.55, 1.0));

        let red_high = vram_status_color(15000, 16000, 0); // 93.75%
        assert_eq!(red_high, hsla(0.0, 0.84, 0.60, 1.0));

        let red_spill = vram_status_color(10000, 16000, 500); // has spill
        assert_eq!(red_spill, hsla(0.0, 0.84, 0.60, 1.0));
    }

    #[core::prelude::v1::test]
    #[allow(clippy::assertions_on_constants)]
    fn graphite_pink_palette_constants() {
        // Assert neutral warm stone accent is in warm sand hue range (~34 deg)
        assert!((COLOR_PINK_PRIMARY.h - 34.0 / 360.0).abs() < 0.02);
        // Assert graphite background is dark and desaturated
        assert!(COLOR_GRAPHITE_CANVAS.l < 0.12);
        assert!(COLOR_GRAPHITE_CANVAS.s < 0.10);
        // Assert sidebar is darker than canvas
        assert!(COLOR_GRAPHITE_SIDEBAR.l < COLOR_GRAPHITE_CANVAS.l);
    }
}
