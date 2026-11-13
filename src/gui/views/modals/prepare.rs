use crate::config::Config;
use crate::gui::state::AppState;
use crate::gui::theme::{
    COLOR_GRAPHITE_BORDER, COLOR_GRAPHITE_CANVAS, COLOR_GRAPHITE_ELEVATED, COLOR_PINK_PRIMARY,
    COLOR_TEXT_MUTED,
};
use crate::sysmon::optimize::close_processes_fast;
use gpui::InteractiveElement as _;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{IconName, Sizable, Size, h_flex, v_flex};

pub struct PrepareModal;

impl PrepareModal {
    pub fn new(_cfg: &Config) -> Self {
        Self
    }

    pub fn render(
        &mut self,
        state_entity: &Entity<AppState>,
        _window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let state = state_entity.read(cx);
        let candidates = &state.prepare_candidates;

        let total_ram_mib: u64 = candidates
            .iter()
            .filter(|(_, sel)| *sel)
            .map(|(c, _)| c.ram_mib)
            .sum();

        let total_vram_mib: u64 = candidates
            .iter()
            .filter(|(_, sel)| *sel)
            .map(|(c, _)| c.vram_mib)
            .sum();

        let has_candidates = !candidates.is_empty();

        v_flex()
            .w(rems(36.0))
            .p_4()
            .rounded_xl()
            .bg(COLOR_GRAPHITE_CANVAS)
            .border_1()
            .border_color(COLOR_GRAPHITE_BORDER)
            .shadow_lg()
            .gap_3()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_size(rems(1.1))
                            .font_weight(FontWeight::BOLD)
                            .child("Prepare: Освобождение памяти"),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(
                                Button::new("refresh-prepare-btn")
                                    .ghost()
                                    .with_size(Size::Small)
                                    .icon(IconName::RefreshCw)
                                    .tooltip("Обновить список процессов")
                                    .on_click({
                                        let state_entity = state_entity.clone();
                                        move |_, _, cx| {
                                            state_entity.update(cx, |s, cx| {
                                                s.rescan_prepare();
                                                s.refresh_vram();
                                                cx.notify();
                                            });
                                        }
                                    }),
                            )
                            .child(
                                Button::new("close-prepare")
                                    .ghost()
                                    .with_size(Size::Small)
                                    .icon(IconName::Close)
                                    .tooltip("Закрыть")
                                    .on_click({
                                        let state_entity = state_entity.clone();
                                        move |_, _, cx| {
                                            state_entity.update(cx, |s, cx| {
                                                s.active_modal = None;
                                                cx.notify();
                                            });
                                        }
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(rems(0.85))
                    .text_color(COLOR_TEXT_MUTED)
                    .child("Выберите фоновые процессы для закрытия. После очистки список обновится автоматически."),
            )
            .child(
                v_flex()
                    .id("prepare-scroll")
                    .max_h(rems(18.0))
                    .overflow_y_scroll()
                    .gap_1()
                    .children(if !has_candidates {
                        vec![div()
                            .p_4()
                            .rounded_md()
                            .bg(COLOR_GRAPHITE_ELEVATED)
                            .text_size(rems(0.9))
                            .text_color(COLOR_TEXT_MUTED)
                            .child("Все чисто. Фоновые процессы не занимают критический объем RAM или VRAM.")
                            .into_any_element()]
                    } else {
                        candidates
                            .iter()
                            .enumerate()
                            .map(|(idx, (c, selected))| {
                                let name = c.name.clone();
                                let pids_count = c.pids.len();
                                let ram = c.ram_mib;
                                let vram = c.vram_mib;
                                let sel = *selected;
                                let state_entity = state_entity.clone();

                                h_flex()
                                    .id(format!("cand-item-{}", idx))
                                    .justify_between()
                                    .items_center()
                                    .p_2()
                                    .rounded_md()
                                    .bg(COLOR_GRAPHITE_ELEVATED)
                                    .cursor_pointer()
                                    .on_click(move |_, _, cx| {
                                        state_entity.update(cx, |s, cx| {
                                            if let Some(item) = s.prepare_candidates.get_mut(idx) {
                                                item.1 = !item.1;
                                                cx.notify();
                                            }
                                        });
                                    })
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div()
                                                    .px_1p5()
                                                    .py_0p5()
                                                    .rounded_sm()
                                                    .bg(if sel {
                                                        COLOR_PINK_PRIMARY
                                                    } else {
                                                        COLOR_GRAPHITE_BORDER
                                                    })
                                                    .text_color(if sel {
                                                        COLOR_GRAPHITE_CANVAS
                                                    } else {
                                                        COLOR_TEXT_MUTED
                                                    })
                                                    .text_size(rems(0.7))
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(if sel { "✓" } else { " " }),
                                            )
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child(format!("{name} ({pids_count} процессов)")),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(rems(0.8))
                                            .text_color(COLOR_TEXT_MUTED)
                                            .child(format!("{ram} MB RAM | {vram} MB VRAM")),
                                    )
                                    .into_any_element()
                            })
                            .collect()
                    }),
            )
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .pt_2()
                    .border_t_1()
                    .border_color(COLOR_GRAPHITE_BORDER)
                    .child(
                        div()
                            .text_size(rems(0.85))
                            .child(if has_candidates {
                                format!("Освободится: ~{total_ram_mib} MB RAM, ~{total_vram_mib} MB VRAM")
                            } else {
                                "Осталось процессов: 0".to_string()
                            }),
                    )
                    .child(if has_candidates {
                        Button::new("terminate-hogs-btn")
                            .danger()
                            .with_size(Size::Medium)
                            .label("Освободить память")
                            .on_click({
                                let state_entity = state_entity.clone();
                                move |_, _, cx| {
                                    state_entity.update(cx, |s, cx| {
                                        let target_pids: Vec<u32> = s
                                            .prepare_candidates
                                            .iter()
                                            .filter(|(_, sel)| *sel)
                                            .flat_map(|(c, _)| c.pids.clone())
                                            .collect();
                                        close_processes_fast(&target_pids);
                                        s.rescan_prepare();
                                        s.refresh_vram();
                                        cx.notify();
                                    });
                                }
                            })
                    } else {
                        Button::new("done-prepare-btn")
                            .secondary()
                            .with_size(Size::Medium)
                            .label("Готово")
                            .on_click({
                                let state_entity = state_entity.clone();
                                move |_, _, cx| {
                                    state_entity.update(cx, |s, cx| {
                                        s.active_modal = None;
                                        cx.notify();
                                    });
                                }
                            })
                    }),
            )
    }
}
