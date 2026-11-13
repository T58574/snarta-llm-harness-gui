use crate::gui::anim::status::{apply_vram_pulse, render_server_online_dot};
use crate::gui::state::{ActiveModal, AppState, ServerStatus};
use crate::gui::theme::{
    COLOR_GRAPHITE_BORDER, COLOR_GRAPHITE_CANVAS, COLOR_GRAPHITE_SIDEBAR, TOPBAR_HEIGHT,
    vram_status_color,
};
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{ActiveTheme, IconName, Sizable, Size, h_flex};

pub struct TopBar;

impl TopBar {
    pub fn render(
        state_entity: &Entity<AppState>,
        _window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let state = state_entity.read(cx);
        let model_name = state
            .active_model_meta
            .as_ref()
            .map(|m| m.file_name.trim_end_matches(".gguf").to_string())
            .unwrap_or_else(|| "Модель не выбрана".to_string());
        let persona_name = state.persona.name.clone();

        let vram_used = state.vram.used_mib;
        let vram_total = state.vram.total_mib;
        let vram_spill = state.vram.spill_mib;
        let vram_color = vram_status_color(vram_used, vram_total, vram_spill);

        let sidebar_open = state.sidebar_open;

        h_flex()
            .h(TOPBAR_HEIGHT)
            .w_full()
            .items_center()
            .justify_between()
            .px_3()
            .border_b_1()
            .border_color(COLOR_GRAPHITE_BORDER)
            .bg(COLOR_GRAPHITE_SIDEBAR)
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("toggle-sidebar")
                            .secondary()
                            .with_size(Size::Small)
                            .label(if sidebar_open { "◀" } else { "▶" })
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
                        div()
                            .px_1()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().primary)
                            .child("Snarta"),
                    )
                    .child(
                        Button::new("model-selector")
                            .secondary()
                            .with_size(Size::Small)
                            .icon(IconName::Cpu)
                            .label(model_name)
                            .on_click({
                                let state_entity = state_entity.clone();
                                move |_, _, cx| {
                                    state_entity.update(cx, |s, cx| {
                                        s.open_modal(ActiveModal::Models);
                                        cx.notify();
                                    });
                                }
                            }),
                    )
                    .child(
                        Button::new("persona-selector")
                            .secondary()
                            .with_size(Size::Small)
                            .icon(IconName::User)
                            .label(persona_name)
                            .on_click({
                                let state_entity = state_entity.clone();
                                move |_, _, cx| {
                                    state_entity.update(cx, |s, cx| {
                                        s.open_modal(ActiveModal::Personas);
                                        cx.notify();
                                    });
                                }
                            }),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("prepare-modal-btn")
                            .secondary()
                            .with_size(Size::Small)
                            .label("Очистка памяти")
                            .on_click({
                                let state_entity = state_entity.clone();
                                move |_, _, cx| {
                                    state_entity.update(cx, |s, cx| {
                                        s.open_modal(ActiveModal::Prepare);
                                        cx.notify();
                                    });
                                }
                            }),
                    )
                    .child(match &state.server_status {
                        ServerStatus::Running { pid, port, .. } => Button::new("server-status-btn")
                            .secondary()
                            .with_size(Size::Small)
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_1p5()
                                    .child(render_server_online_dot("server-online-dot"))
                                    .child(format!("Сервер запущен · {port} · PID {pid}")),
                            )
                            .on_click({
                                let state_entity = state_entity.clone();
                                move |_, _, cx| {
                                    state_entity.update(cx, |s, cx| {
                                        s.stop_server();
                                        cx.notify();
                                    });
                                }
                            })
                            .into_any_element(),
                        ServerStatus::Stopped => Button::new("server-status-btn")
                            .secondary()
                            .with_size(Size::Small)
                            .label("Запустить сервер")
                            .on_click({
                                let state_entity = state_entity.clone();
                                move |_, _, cx| {
                                    state_entity.update(cx, |s, cx| {
                                        if let Some(ref m) = s.active_model_meta {
                                            let fname = m.file_name.clone();
                                            let settings = s.settings_for(&fname);
                                            s.start_server_for_model(&fname, settings.ctx);
                                            cx.notify();
                                        } else {
                                            s.open_modal(ActiveModal::Models);
                                            cx.notify();
                                        }
                                    });
                                }
                            })
                            .into_any_element(),
                        ServerStatus::Starting { .. } => Button::new("server-status-btn")
                            .secondary()
                            .with_size(Size::Small)
                            .loading(true)
                            .label("Запуск сервера...")
                            .into_any_element(),
                        ServerStatus::Error(err) => {
                            let err_msg = err.clone();
                            Button::new("server-status-btn")
                                .danger()
                                .with_size(Size::Small)
                                .label("Ошибка сервера")
                                .tooltip(err_msg.clone())
                                .on_click({
                                    let state_entity = state_entity.clone();
                                    move |_, _, cx| {
                                        state_entity.update(cx, |s, cx| {
                                            s.status_notice =
                                                Some(format!("Ошибка сервера: {err_msg}"));
                                            s.server_status = ServerStatus::Stopped;
                                            cx.notify();
                                        });
                                    }
                                })
                                .into_any_element()
                        }
                    })
                    .child({
                        let is_vram_danger = vram_spill > 0
                            || (vram_total > 0 && vram_used * 10 >= vram_total * 9);
                        let vram_badge = div()
                            .px_2p5()
                            .py_1()
                            .rounded_md()
                            .bg(COLOR_GRAPHITE_CANVAS)
                            .border_1()
                            .border_color(vram_color)
                            .text_size(rems(0.75))
                            .child(format_vram_display(vram_used, vram_total, vram_spill));
                        apply_vram_pulse(vram_badge, is_vram_danger, vram_color)
                    }),
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

/// Formats VRAM metrics for topbar display:
/// Total VRAM is rounded to the nearest whole gigabyte (e.g. RX 7800 XT with ~16368 MiB correctly displays as 16 GB).
/// Used VRAM is displayed with 1 decimal place precision (e.g. 0.0/16 GB).
pub fn format_vram_display(used_mib: u64, total_mib: u64, spill_mib: u64) -> String {
    let total_gb = (total_mib + 512) / 1024;
    let used_gb = used_mib as f64 / 1024.0;
    format!(
        "VRAM: {:.1}/{} GB{}",
        used_gb,
        total_gb,
        if spill_mib > 0 {
            format!(" (перелив: {spill_mib} МБ)")
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
        // RX 7800 XT with 16368 MiB should round up to 16 GB, not truncate to 15 GB
        assert_eq!(format_vram_display(0, 16368, 0), "VRAM: 0.0/16 GB");
        assert_eq!(format_vram_display(1536, 16368, 0), "VRAM: 1.5/16 GB");
        assert_eq!(format_vram_display(2048, 16384, 0), "VRAM: 2.0/16 GB");
        assert_eq!(
            format_vram_display(16368, 16368, 512),
            "VRAM: 16.0/16 GB (перелив: 512 МБ)"
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
