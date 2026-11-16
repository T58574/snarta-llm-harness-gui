use gpui::{App, Hsla, Pixels, hsla, px};
use gpui_component::theme::{Theme, ThemeMode};

/// Layout metrics for Snarta GUI.
pub const SIDEBAR_WIDTH: Pixels = px(268.0);
pub const TOPBAR_HEIGHT: Pixels = px(48.0);

/// Core brand palette (#141413, #faf9f5, #b0aea5, #e8e6dc, #d97757, #6799E2, #87A26A).
pub const COLOR_BRAND_DARK: Hsla = hsla(60.0 / 360.0, 0.026, 0.078, 1.0); // #141413
pub const COLOR_BRAND_LIGHT: Hsla = hsla(48.0 / 360.0, 0.33, 0.97, 1.0); // #faf9f5
pub const COLOR_BRAND_MID_GRAY: Hsla = hsla(48.0 / 360.0, 0.08, 0.67, 1.0); // #b0aea5
pub const COLOR_BRAND_LIGHT_GRAY: Hsla = hsla(48.0 / 360.0, 0.21, 0.89, 1.0); // #e8e6dc

/// Surface tokens derived from #141413.
pub const COLOR_BG_SIDEBAR: Hsla = hsla(60.0 / 360.0, 0.03, 0.062, 1.0);
pub const COLOR_BG_CANVAS: Hsla = COLOR_BRAND_DARK; // #141413
pub const COLOR_BG_ELEVATED: Hsla = hsla(50.0 / 360.0, 0.035, 0.115, 1.0);
pub const COLOR_BG_ELEVATED_HOVER: Hsla = hsla(50.0 / 360.0, 0.04, 0.15, 1.0);
pub const COLOR_BG_INPUT: Hsla = hsla(50.0 / 360.0, 0.03, 0.095, 1.0);
pub const COLOR_BORDER: Hsla = hsla(48.0 / 360.0, 0.05, 0.20, 1.0);
pub const COLOR_BORDER_SUBTLE: Hsla = hsla(48.0 / 360.0, 0.04, 0.15, 1.0);

/// Primary accent: Brand Orange (#d97757).
pub const COLOR_ACCENT_ORANGE: Hsla = hsla(15.0 / 360.0, 0.63, 0.60, 1.0); // #d97757
pub const COLOR_ACCENT_ORANGE_HOVER: Hsla = hsla(15.0 / 360.0, 0.70, 0.66, 1.0);
pub const COLOR_ACCENT_ORANGE_ACTIVE: Hsla = hsla(15.0 / 360.0, 0.58, 0.52, 1.0);
pub const COLOR_ACCENT_ORANGE_MUTED: Hsla = hsla(15.0 / 360.0, 0.30, 0.14, 1.0);
pub const COLOR_ACCENT_ORANGE_SELECTION: Hsla = hsla(15.0 / 360.0, 0.63, 0.60, 0.26);

/// Secondary accent: Brand Blue (#6799E2).
pub const COLOR_ACCENT_BLUE: Hsla = hsla(216.0 / 360.0, 0.68, 0.65, 1.0); // #6799E2
pub const COLOR_ACCENT_BLUE_MUTED: Hsla = hsla(216.0 / 360.0, 0.35, 0.15, 1.0);

/// Tertiary accent: Brand Green (#87A26A).
pub const COLOR_ACCENT_GREEN: Hsla = hsla(89.0 / 360.0, 0.24, 0.53, 1.0); // #87A26A
pub const COLOR_ACCENT_GREEN_MUTED: Hsla = hsla(89.0 / 360.0, 0.20, 0.14, 1.0);

/// Semantic status colors.
pub const COLOR_DANGER: Hsla = hsla(2.0 / 360.0, 0.72, 0.58, 1.0);
pub const COLOR_DANGER_HOVER: Hsla = hsla(2.0 / 360.0, 0.78, 0.64, 1.0);
pub const COLOR_DANGER_ACTIVE: Hsla = hsla(2.0 / 360.0, 0.72, 0.48, 1.0);
pub const COLOR_DANGER_MUTED: Hsla = hsla(2.0 / 360.0, 0.38, 0.15, 1.0);
pub const COLOR_WARNING: Hsla = hsla(36.0 / 360.0, 0.82, 0.55, 1.0);

/// Typography tokens.
pub const COLOR_TEXT_PRIMARY: Hsla = COLOR_BRAND_LIGHT; // #faf9f5
pub const COLOR_TEXT_MUTED: Hsla = COLOR_BRAND_MID_GRAY; // #b0aea5
pub const COLOR_TEXT_SUBTLE: Hsla = COLOR_BRAND_LIGHT_GRAY; // #e8e6dc
pub const COLOR_TEXT_ON_ACCENT: Hsla = COLOR_BRAND_DARK; // #141413

/// Apply the Snarta brand dark theme (#141413 canvas, #faf9f5 text, #d97757 / #6799E2 / #87A26A accents).
pub fn apply_brand_theme(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);

    let sec_btn_bg = COLOR_BG_ELEVATED;
    let sec_btn_hover = COLOR_BG_ELEVATED_HOVER;
    let sec_btn_active = COLOR_BG_INPUT;

    Theme::update(cx, |theme| {
        theme.mode = ThemeMode::Dark;
        theme.radius = px(8.0);
        theme.radius_lg = px(12.0);
        theme.font_size = px(14.0);
        theme.shadow = true;

        let c = &mut theme.colors;

        // Surfaces
        c.background = COLOR_BG_CANVAS;
        c.sidebar = COLOR_BG_SIDEBAR;
        c.sidebar_border = COLOR_BORDER;
        c.sidebar_foreground = COLOR_TEXT_PRIMARY;
        c.sidebar_accent = COLOR_ACCENT_ORANGE_MUTED;
        c.sidebar_accent_foreground = COLOR_TEXT_SUBTLE;
        c.sidebar_primary = COLOR_ACCENT_ORANGE;
        c.sidebar_primary_foreground = COLOR_TEXT_ON_ACCENT;

        c.muted = COLOR_BG_ELEVATED;
        c.muted_foreground = COLOR_TEXT_MUTED;
        c.border = COLOR_BORDER;
        c.input = COLOR_BG_INPUT;
        c.popover = COLOR_BG_ELEVATED;
        c.popover_foreground = COLOR_TEXT_PRIMARY;
        c.accordion = COLOR_BG_ELEVATED;
        c.group_box = COLOR_BG_ELEVATED;
        c.group_box_foreground = COLOR_TEXT_PRIMARY;
        c.table = COLOR_BG_ELEVATED;
        c.table_active = COLOR_ACCENT_ORANGE_MUTED;
        c.table_active_border = COLOR_ACCENT_ORANGE;
        c.table_even = COLOR_BG_CANVAS;
        c.table_head = COLOR_BG_SIDEBAR;
        c.table_head_foreground = COLOR_TEXT_MUTED;
        c.table_foot = COLOR_BG_SIDEBAR;
        c.table_foot_foreground = COLOR_TEXT_MUTED;
        c.table_hover = COLOR_ACCENT_ORANGE_MUTED;
        c.table_row_border = COLOR_BORDER_SUBTLE;
        c.list = COLOR_BG_ELEVATED;
        c.list_active = COLOR_ACCENT_ORANGE_MUTED;
        c.list_active_border = COLOR_ACCENT_ORANGE;
        c.list_even = COLOR_BG_CANVAS;
        c.list_head = COLOR_BG_SIDEBAR;
        c.list_hover = COLOR_ACCENT_ORANGE_MUTED;
        c.title_bar = COLOR_BG_SIDEBAR;
        c.title_bar_border = COLOR_BORDER;
        c.status_bar = COLOR_BG_SIDEBAR;
        c.status_bar_border = COLOR_BORDER;
        c.window_border = COLOR_BORDER;
        c.overlay = COLOR_BRAND_DARK.opacity(0.78);

        // Typography
        c.foreground = COLOR_TEXT_PRIMARY;

        // Primary accents (#d97757)
        c.primary = COLOR_ACCENT_ORANGE;
        c.primary_hover = COLOR_ACCENT_ORANGE_HOVER;
        c.primary_active = COLOR_ACCENT_ORANGE_ACTIVE;
        c.primary_foreground = COLOR_TEXT_ON_ACCENT;
        c.accent = COLOR_ACCENT_ORANGE_MUTED;
        c.accent_foreground = COLOR_TEXT_SUBTLE;

        // Primary buttons
        c.button_primary = COLOR_ACCENT_ORANGE;
        c.button_primary_hover = COLOR_ACCENT_ORANGE_HOVER;
        c.button_primary_active = COLOR_ACCENT_ORANGE_ACTIVE;
        c.button_primary_foreground = COLOR_TEXT_ON_ACCENT;

        // Secondary buttons
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
        c.button_hover = COLOR_BG_ELEVATED_HOVER;
        c.button_active = COLOR_BG_INPUT;
        c.button_foreground = COLOR_TEXT_PRIMARY;

        // Interactive controls
        c.caret = COLOR_ACCENT_ORANGE;
        c.ring = COLOR_ACCENT_ORANGE;
        c.selection = COLOR_ACCENT_ORANGE_SELECTION;
        c.progress_bar = COLOR_ACCENT_ORANGE;
        c.slider_bar = COLOR_ACCENT_ORANGE;
        c.slider_thumb = COLOR_ACCENT_ORANGE_HOVER;
        c.switch = COLOR_BORDER;
        c.switch_thumb = COLOR_TEXT_PRIMARY;
        c.link = COLOR_ACCENT_BLUE;
        c.link_hover = COLOR_TEXT_SUBTLE;
        c.link_active = COLOR_ACCENT_ORANGE;

        // Tabs
        c.tab = COLOR_BG_ELEVATED;
        c.tab_active = COLOR_BG_CANVAS;
        c.tab_active_foreground = COLOR_TEXT_PRIMARY;
        c.tab_bar = COLOR_BG_SIDEBAR;
        c.tab_bar_segmented = COLOR_BG_SIDEBAR;
        c.tab_foreground = COLOR_TEXT_MUTED;

        // Scrollbar
        c.scrollbar = hsla(0.0, 0.0, 0.0, 0.0);
        c.scrollbar_thumb = COLOR_BRAND_MID_GRAY.opacity(0.32);
        c.scrollbar_thumb_hover = COLOR_ACCENT_ORANGE;

        // Semantic statuses
        c.danger = COLOR_DANGER;
        c.danger_hover = COLOR_DANGER_HOVER;
        c.danger_active = COLOR_DANGER_ACTIVE;
        c.danger_foreground = COLOR_TEXT_ON_ACCENT;
        c.button_danger = COLOR_DANGER;
        c.button_danger_hover = COLOR_DANGER_HOVER;
        c.button_danger_active = COLOR_DANGER_ACTIVE;
        c.button_danger_foreground = COLOR_TEXT_ON_ACCENT;

        c.info = COLOR_ACCENT_BLUE;
        c.info_hover = COLOR_ACCENT_BLUE;
        c.info_active = COLOR_ACCENT_BLUE;
        c.info_foreground = COLOR_TEXT_ON_ACCENT;
        c.button_info = COLOR_ACCENT_BLUE;
        c.button_info_hover = COLOR_ACCENT_BLUE;
        c.button_info_active = COLOR_ACCENT_BLUE;
        c.button_info_foreground = COLOR_TEXT_ON_ACCENT;

        c.success = COLOR_ACCENT_GREEN;
        c.success_hover = COLOR_ACCENT_GREEN;
        c.success_active = COLOR_ACCENT_GREEN;
        c.success_foreground = COLOR_TEXT_ON_ACCENT;
        c.button_success = COLOR_ACCENT_GREEN;
        c.button_success_hover = COLOR_ACCENT_GREEN;
        c.button_success_active = COLOR_ACCENT_GREEN;
        c.button_success_foreground = COLOR_TEXT_ON_ACCENT;

        c.warning = COLOR_WARNING;
        c.warning_hover = COLOR_WARNING;
        c.warning_active = COLOR_WARNING;
        c.warning_foreground = COLOR_TEXT_ON_ACCENT;
        c.button_warning = COLOR_WARNING;
        c.button_warning_hover = COLOR_WARNING;
        c.button_warning_active = COLOR_WARNING;
        c.button_warning_foreground = COLOR_TEXT_ON_ACCENT;

        // Other tokens & accent cycle (orange -> blue -> green)
        c.description_list_label = COLOR_BG_SIDEBAR;
        c.description_list_label_foreground = COLOR_TEXT_MUTED;
        c.drag_border = COLOR_ACCENT_ORANGE;
        c.drop_target = COLOR_ACCENT_ORANGE_MUTED;
        c.skeleton = COLOR_BG_ELEVATED;
        c.chart_1 = COLOR_ACCENT_ORANGE;
        c.chart_2 = COLOR_ACCENT_BLUE;
        c.chart_3 = COLOR_ACCENT_GREEN;
        c.chart_4 = COLOR_BRAND_LIGHT_GRAY;
        c.chart_5 = COLOR_BRAND_MID_GRAY;
        c.chart_bullish = COLOR_ACCENT_GREEN;
        c.chart_bearish = COLOR_DANGER;
        c.chart_grid = COLOR_BORDER_SUBTLE;

        theme.light_theme = theme.dark_theme.clone();
    });
}

/// VRAM utilization color helper using brand semantic accents.
#[inline]
pub fn vram_status_color(used_mib: u64, total_mib: u64, spill_mib: u64) -> Hsla {
    if spill_mib > 0 {
        COLOR_DANGER
    } else if total_mib == 0 {
        COLOR_TEXT_MUTED
    } else {
        let pct = (used_mib as f32 / total_mib as f32) * 100.0;
        if pct > 90.0 {
            COLOR_DANGER
        } else if pct > 75.0 {
            COLOR_WARNING
        } else {
            COLOR_ACCENT_GREEN
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn vram_status_color_thresholds() {
        let green = vram_status_color(8000, 16000, 0); // 50%
        assert_eq!(green, COLOR_ACCENT_GREEN);

        let yellow = vram_status_color(13000, 16000, 0); // 81.25%
        assert_eq!(yellow, COLOR_WARNING);

        let red_high = vram_status_color(15000, 16000, 0); // 93.75%
        assert_eq!(red_high, COLOR_DANGER);

        let red_spill = vram_status_color(10000, 16000, 500); // has spill
        assert_eq!(red_spill, COLOR_DANGER);
    }

    #[core::prelude::v1::test]
    #[allow(clippy::assertions_on_constants)]
    fn brand_palette_invariants() {
        assert!(COLOR_BG_CANVAS.l < 0.10);
        assert!(COLOR_BG_SIDEBAR.l < COLOR_BG_CANVAS.l);
        assert!(COLOR_TEXT_PRIMARY.l > 0.90);
        assert!(COLOR_ACCENT_ORANGE.h < 0.08);
    }
}
