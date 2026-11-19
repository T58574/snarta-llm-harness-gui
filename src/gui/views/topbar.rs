use crate::gui::anim::avatar::render_avatar_circle;
use crate::gui::anim::status::render_server_online_dot;
use crate::gui::icons::icons;
use crate::gui::state::{ActiveModal, AppState, ServerStatus};
use crate::gui::theme::{
    COLOR_ACCENT_BLUE, COLOR_ACCENT_GREEN, COLOR_ACCENT_ORANGE, COLOR_BG_ELEVATED,
    COLOR_BORDER_GLASS, COLOR_BORDER_GLASS_STRONG, COLOR_DANGER, COLOR_GLASS_MODAL,
    COLOR_GLASS_SURFACE, COLOR_GLASS_TOPBAR, COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY, TOPBAR_HEIGHT,
    vram_status_color,
};
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{
    Icon, IconName, InteractiveElementExt as _, Sizable, Size, h_flex, v_flex,
};

pub struct TopBar;

impl TopBar {
    pub fn render(
        state_entity: &Entity<AppState>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let state = state_entity.read(cx);
        let model_name = state
            .active_model_meta
            .as_ref()
            .map(|m| m.file_name.trim_end_matches(".gguf").to_string())
            .unwrap_or_else(|| "Модель не выбрана".to_string());
        let persona_display = state.persona.display_name.clone();
        let persona_avatar_img = state.persona.avatar_image.clone();

        let vram_used = state.vram.used_mib;
        let vram_total = state.vram.total_mib;
        let vram_spill = state.vram.spill_mib;
        let vram_color = vram_status_color(vram_used, vram_total, vram_spill);

        let sidebar_open = state.sidebar_open;
        let system_menu_open = state.system_menu_open;
        let server_status = state.server_status.clone();

        let status_dot = match &server_status {
            ServerStatus::Running { .. } => {
                render_server_online_dot("topbar-srv-dot").into_any_element()
            }
            ServerStatus::Starting { .. } => div()
                .w(px(7.0))
                .h(px(7.0))
                .rounded_full()
                .bg(COLOR_ACCENT_ORANGE)
                .into_any_element(),
            ServerStatus::Error(_) => div()
                .w(px(7.0))
                .h(px(7.0))
                .rounded_full()
                .bg(COLOR_DANGER)
                .into_any_element(),
            ServerStatus::Stopped => div()
                .w(px(7.0))
                .h(px(7.0))
                .rounded_full()
                .bg(COLOR_TEXT_MUTED.opacity(0.6))
                .into_any_element(),
        };

        let vram_total_gb = (vram_total + 512) / 1024;
        let vram_used_gb = vram_used as f64 / 1024.0;
        let is_maximized = window.is_maximized();
        let is_windows = cfg!(target_os = "windows");
        let is_linux = cfg!(target_os = "linux");

        h_flex()
            .id("custom-topbar")
            .h(TOPBAR_HEIGHT)
            .w_full()
            .items_center()
            .bg(COLOR_GLASS_TOPBAR)
            .border_b_1()
            .border_color(COLOR_BORDER_GLASS)
            // 1. Left interactive controls (NO WindowControlArea::Drag on or above these buttons)
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .pl_3()
                    .flex_shrink_0()
                    .child(
                        Button::new("toggle-sidebar")
                            .ghost()
                            .with_size(Size::Small)
                            .icon(if sidebar_open {
                                icons::sidebar_close()
                            } else {
                                icons::sidebar_open()
                            })
                            .tooltip(if sidebar_open {
                                "Скрыть боковую панель"
                            } else {
                                "Показать боковую панель"
                            })
                            .on_click({
                                let state_entity = state_entity.clone();
                                move |_, _, cx| {
                                    state_entity.update(cx, |s, cx| {
                                        s.sidebar_open = !s.sidebar_open;
                                        cx.notify();
                                    });
                                }
                            }),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1p5()
                            .px_1()
                            .child(
                                div()
                                    .w(px(8.0))
                                    .h(px(8.0))
                                    .rounded_full()
                                    .bg(COLOR_ACCENT_ORANGE),
                            )
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(rems(0.92))
                                    .text_color(COLOR_TEXT_PRIMARY)
                                    .child("Snarta"),
                            ),
                    )
                    .child(
                        Button::new("model-selector")
                            .secondary()
                            .outline()
                            .with_size(Size::Small)
                            .icon(icons::cpu())
                            .label(model_name)
                            .tooltip("Выбор и статус локальной модели GGUF")
                            .on_click({
                                let state_entity = state_entity.clone();
                                move |_, _, cx| {
                                    state_entity.update(cx, |s, cx| {
                                        s.system_menu_open = false;
                                        s.open_modal(ActiveModal::Models);
                                        cx.notify();
                                    });
                                }
                            }),
                    )
                    .child(
                        Button::new("persona-selector")
                            .secondary()
                            .outline()
                            .with_size(Size::Small)
                            .tooltip("Персоны и аватар")
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_1p5()
                                    .child(render_avatar_circle(
                                        &persona_display,
                                        persona_avatar_img.as_ref(),
                                        18.0,
                                    ))
                                    .child(persona_display),
                            )
                            .on_click({
                                let state_entity = state_entity.clone();
                                move |_, _, cx| {
                                    state_entity.update(cx, |s, cx| {
                                        s.system_menu_open = false;
                                        s.open_modal(ActiveModal::Personas);
                                        cx.notify();
                                    });
                                }
                            }),
                    ),
            )
            // 2. Dedicated middle drag spacer (ONLY this empty region has WindowControlArea::Drag)
            .child(
                div()
                    .id("topbar-drag-spacer")
                    .flex_1()
                    .h_full()
                    .min_w(px(24.0))
                    .window_control_area(WindowControlArea::Drag)
                    .when(is_linux, |this| {
                        this.on_double_click(|_, window, _| window.zoom_window())
                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                window.start_window_move();
                            })
                    }),
            )
            // 3. Right cluster: System dropdown button + native-style Window Controls (— □ ×)
            .child(
                h_flex()
                    .items_center()
                    .h_full()
                    .gap_2()
                    .flex_shrink_0()
                    .child(
                        Button::new("system-menu-toggle")
                            .secondary()
                            .outline()
                            .with_size(Size::Small)
                            .tooltip("Сервер, очистка памяти, VRAM и ОЗУ")
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_1p5()
                                    .child(status_dot)
                                    .child(
                                        div()
                                            .text_size(rems(0.76))
                                            .text_color(COLOR_TEXT_PRIMARY)
                                            .child(format!(
                                                "Система · {vram_used_gb:.1}/{vram_total_gb}G"
                                            )),
                                    )
                                    .child(
                                        div()
                                            .w(px(6.0))
                                            .h(px(6.0))
                                            .rounded_full()
                                            .bg(vram_color),
                                    )
                                    .child(if system_menu_open {
                                        icons::chevron_down().with_size(Size::XSmall)
                                    } else {
                                        icons::chevron_right().with_size(Size::XSmall)
                                    }),
                            )
                            .on_click({
                                let state_entity = state_entity.clone();
                                move |_, _, cx| {
                                    state_entity.update(cx, |s, cx| {
                                        s.system_menu_open = !s.system_menu_open;
                                        cx.notify();
                                    });
                                }
                            }),
                    )
                    .child(
                        div()
                            .w(px(1.0))
                            .h(px(18.0))
                            .bg(COLOR_BORDER_GLASS_STRONG),
                    )
                    .child(
                        h_flex()
                            .id("window-controls-bar")
                            .items_center()
                            .h_full()
                            .child(
                                div()
                                    .id("win-btn-minimize")
                                    .flex()
                                    .w(px(38.0))
                                    .h_full()
                                    .items_center()
                                    .justify_center()
                                    .text_color(COLOR_TEXT_PRIMARY)
                                    .hover(|s| s.bg(COLOR_BG_ELEVATED))
                                    .when(is_windows, |this| {
                                        this.window_control_area(WindowControlArea::Min)
                                    })
                                    .on_click(|_, window, _| {
                                        window.minimize_window();
                                    })
                                    .child(Icon::new(IconName::WindowMinimize).small()),
                            )
                            .child(
                                div()
                                    .id("win-btn-maximize")
                                    .flex()
                                    .w(px(38.0))
                                    .h_full()
                                    .items_center()
                                    .justify_center()
                                    .text_color(COLOR_TEXT_PRIMARY)
                                    .hover(|s| s.bg(COLOR_BG_ELEVATED))
                                    .when(is_windows, |this| {
                                        this.window_control_area(WindowControlArea::Max)
                                    })
                                    .on_click(|_, window, _| {
                                        window.zoom_window();
                                    })
                                    .child(
                                        Icon::new(if is_maximized {
                                            IconName::WindowRestore
                                        } else {
                                            IconName::WindowMaximize
                                        })
                                        .small(),
                                    ),
                            )
                            .child(
                                div()
                                    .id("win-btn-close")
                                    .flex()
                                    .w(px(40.0))
                                    .h_full()
                                    .items_center()
                                    .justify_center()
                                    .text_color(COLOR_TEXT_PRIMARY)
                                    .hover(|s| s.bg(COLOR_DANGER).text_color(gpui::white()))
                                    .when(is_windows, |this| {
                                        this.window_control_area(WindowControlArea::Close)
                                    })
                                    .on_click(|_, window, _| {
                                        window.remove_window();
                                    })
                                    .child(Icon::new(IconName::WindowClose).small()),
                            ),
                    ),
            )
    }

    /// Floating glassmorphic dropdown menu for Server control, Memory cleanup, VRAM & RAM.
    pub fn render_system_menu_popover(
        state_entity: &Entity<AppState>,
        cx_state: &AppState,
    ) -> impl IntoElement {
        let vram_used = cx_state.vram.used_mib;
        let vram_total = cx_state.vram.total_mib;
        let vram_spill = cx_state.vram.spill_mib;
        let vram_color = vram_status_color(vram_used, vram_total, vram_spill);
        let vram_pct = if vram_total > 0 {
            (vram_used as f32 / vram_total as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let ram_used = cx_state.vram.ram_used_mib;
        let ram_total = cx_state.vram.ram_total_mib;
        let ram_used_gb = ram_used as f64 / 1024.0;
        let ram_total_gb = ram_total as f64 / 1024.0;
        let ram_pct = if ram_total > 0 {
            (ram_used as f32 / ram_total as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let server_status = cx_state.server_status.clone();

        v_flex()
            .id("system-menu-popover")
            .w(px(300.0))
            .p_3p5()
            .rounded_2xl()
            .bg(COLOR_GLASS_MODAL)
            .border_1()
            .border_color(COLOR_BORDER_GLASS_STRONG)
            .shadow_xl()
            .gap_3()
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(rems(0.74))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(COLOR_TEXT_MUTED)
                            .child("Управление сервером"),
                    )
                    .child(match &server_status {
                        ServerStatus::Running { pid, port, .. } => Button::new("menu-srv-stop")
                            .secondary()
                            .outline()
                            .w_full()
                            .with_size(Size::Small)
                            .tooltip(format!("PID {pid}"))
                            .child(
                                h_flex()
                                    .items_center()
                                    .justify_center()
                                    .gap_2()
                                    .child(render_server_online_dot("menu-online-dot"))
                                    .child(format!("Остановить сервер (:{port})")),
                            )
                            .on_click({
                                let state_entity = state_entity.clone();
                                move |_, _, cx| {
                                    state_entity.update(cx, |s, cx| {
                                        s.stop_server();
                                        s.system_menu_open = false;
                                        cx.notify();
                                    });
                                }
                            })
                            .into_any_element(),
                        ServerStatus::Stopped => Button::new("menu-srv-start")
                            .primary()
                            .w_full()
                            .with_size(Size::Small)
                            .icon(icons::play())
                            .label("Запустить сервер")
                            .on_click({
                                let state_entity = state_entity.clone();
                                move |_, _, cx| {
                                    state_entity.update(cx, |s, cx| {
                                        s.system_menu_open = false;
                                        if let Some(ref m) = s.active_model_meta {
                                            let fname = m.file_name.clone();
                                            let settings = s.settings_for(&fname);
                                            s.start_server_for_model(&fname, settings.ctx);
                                        } else {
                                            s.open_modal(ActiveModal::Models);
                                        }
                                        cx.notify();
                                    });
                                }
                            })
                            .into_any_element(),
                        ServerStatus::Starting { .. } => Button::new("menu-srv-starting")
                            .secondary()
                            .outline()
                            .w_full()
                            .with_size(Size::Small)
                            .loading(true)
                            .label("Запуск сервера...")
                            .into_any_element(),
                        ServerStatus::Error(err) => {
                            let err_msg = err.clone();
                            Button::new("menu-srv-err")
                                .danger()
                                .w_full()
                                .with_size(Size::Small)
                                .icon(icons::alert())
                                .label("Сбросить ошибку сервера")
                                .tooltip(err_msg.clone())
                                .on_click({
                                    let state_entity = state_entity.clone();
                                    move |_, _, cx| {
                                        state_entity.update(cx, |s, cx| {
                                            s.status_notice =
                                                Some(format!("Ошибка сервера: {err_msg}"));
                                            s.server_status = ServerStatus::Stopped;
                                            s.system_menu_open = false;
                                            cx.notify();
                                        });
                                    }
                                })
                                .into_any_element()
                        }
                    })
                    .child(
                        Button::new("menu-prepare-btn")
                            .secondary()
                            .outline()
                            .w_full()
                            .with_size(Size::Small)
                            .icon(icons::broom())
                            .label("Очистить память")
                            .on_click({
                                let state_entity = state_entity.clone();
                                move |_, _, cx| {
                                    state_entity.update(cx, |s, cx| {
                                        s.system_menu_open = false;
                                        s.open_modal(ActiveModal::Prepare);
                                        cx.notify();
                                    });
                                }
                            }),
                    ),
            )
            .child(div().h(px(1.0)).w_full().bg(COLOR_BORDER_GLASS))
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .text_size(rems(0.76))
                                    .child(
                                        div()
                                            .text_color(COLOR_TEXT_MUTED)
                                            .child("Видеопамять (VRAM)"),
                                    )
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(vram_color)
                                            .child(
                                                format_vram_display(
                                                    vram_used,
                                                    vram_total,
                                                    vram_spill,
                                                )
                                                .replace("VRAM ", ""),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .h(px(5.0))
                                    .rounded_full()
                                    .bg(COLOR_GLASS_SURFACE)
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .h_full()
                                            .w(relative(vram_pct))
                                            .rounded_full()
                                            .bg(vram_color),
                                    ),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .text_size(rems(0.76))
                                    .child(div().text_color(COLOR_TEXT_MUTED).child("ОЗУ (RAM)"))
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(COLOR_ACCENT_BLUE)
                                            .child(if ram_total > 0 {
                                                format!("{ram_used_gb:.1} / {ram_total_gb:.0} GB")
                                            } else {
                                                "—".to_string()
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .h(px(5.0))
                                    .rounded_full()
                                    .bg(COLOR_GLASS_SURFACE)
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .h_full()
                                            .w(relative(ram_pct))
                                            .rounded_full()
                                            .bg(if ram_pct > 0.88 {
                                                COLOR_ACCENT_ORANGE
                                            } else {
                                                COLOR_ACCENT_GREEN
                                            }),
                                    ),
                            ),
                    ),
            )
    }
}

/// Formats context token count into a compact human-friendly string (e.g. 32768 -> "32k", 4120 -> "4.0k", 500 -> "500").
pub fn format_compact_ctx(val: u32) -> String {
    if val >= 1024 {
        if val % 1024 == 0 {
            format!("{}k", val / 1024)
        } else {
            let k = val as f64 / 1024.0;
            format!("{:.1}k", k)
        }
    } else {
        format!("{val}")
    }
}

/// Formats VRAM metrics for display.
pub fn format_vram_display(used_mib: u64, total_mib: u64, spill_mib: u64) -> String {
    let total_gb = (total_mib + 512) / 1024;
    let used_gb = used_mib as f64 / 1024.0;
    format!(
        "VRAM {used_gb:.1}/{total_gb} GB{}",
        if spill_mib > 0 {
            format!(" (+{spill_mib} МБ RAM)")
        } else {
            String::new()
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn test_format_vram_display_rounding() {
        assert_eq!(format_vram_display(0, 16368, 0), "VRAM 0.0/16 GB");
        assert_eq!(format_vram_display(1536, 16368, 0), "VRAM 1.5/16 GB");
        assert_eq!(format_vram_display(2048, 16384, 0), "VRAM 2.0/16 GB");
        assert_eq!(
            format_vram_display(16368, 16368, 512),
            "VRAM 16.0/16 GB (+512 МБ RAM)"
        );
    }

    #[core::prelude::v1::test]
    fn test_format_compact_ctx() {
        assert_eq!(format_compact_ctx(0), "0");
        assert_eq!(format_compact_ctx(512), "512");
        assert_eq!(format_compact_ctx(1024), "1k");
        assert_eq!(format_compact_ctx(1536), "1.5k");
        assert_eq!(format_compact_ctx(4096), "4k");
        assert_eq!(format_compact_ctx(32768), "32k");
        assert_eq!(format_compact_ctx(65536), "64k");
        assert_eq!(format_compact_ctx(131072), "128k");
    }
}
