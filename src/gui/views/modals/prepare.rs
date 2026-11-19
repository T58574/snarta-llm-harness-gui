use crate::config::Config;
use crate::gui::icons::icons;
use crate::gui::state::AppState;
use crate::gui::theme::{
    COLOR_ACCENT_GREEN, COLOR_ACCENT_ORANGE, COLOR_BG_CANVAS, COLOR_BG_ELEVATED,
    COLOR_BG_ELEVATED_HOVER, COLOR_BORDER, COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY,
};
use crate::sysmon::optimize::close_processes_fast;
use gpui::InteractiveElement as _;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::checkbox::Checkbox;
use gpui_component::{Sizable, Size, h_flex, v_flex};

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
            .w(rems(40.0))
            .p_5()
            .rounded_2xl()
            .bg(crate::gui::theme::COLOR_GLASS_MODAL)
            .border_1()
            .border_color(crate::gui::theme::COLOR_BORDER_GLASS_STRONG)
            .shadow_2xl()
            .gap_3p5()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(icons::broom().with_size(Size::Medium).text_color(COLOR_ACCENT_ORANGE))
                            .child(
                                div()
                                    .text_size(rems(1.1))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(COLOR_TEXT_PRIMARY)
                                    .child("Освобождение памяти"),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(
                                Button::new("refresh-prepare-btn")
                                    .ghost()
                                    .with_size(Size::Small)
                                    .icon(icons::refresh())
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
                                    .icon(icons::close())
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
                    .text_size(rems(0.84))
                    .text_color(COLOR_TEXT_MUTED)
                    .child("Отметьте фоновые процессы для закрытия. Список обновится автоматически после очистки."),
            )
            .child(
                v_flex()
                    .id("prepare-scroll")
                    .max_h(rems(19.0))
                    .overflow_y_scroll()
                    .gap_1p5()
                    .children(if !has_candidates {
                        vec![
                            h_flex()
                                .p_4()
                                .gap_2()
                                .items_center()
                                .rounded_lg()
                                .bg(COLOR_BG_ELEVATED)
                                .border_1()
                                .border_color(COLOR_BORDER)
                                .child(icons::check().with_size(Size::Medium).text_color(COLOR_ACCENT_GREEN))
                                .child(
                                    div()
                                        .text_size(rems(0.88))
                                        .text_color(COLOR_TEXT_MUTED)
                                        .child("Фоновые процессы не занимают заметный объём RAM или VRAM."),
                                )
                                .into_any_element(),
                        ]
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
                                    .px_3()
                                    .py_2()
                                    .gap_3()
                                    .rounded_lg()
                                    .bg(COLOR_BG_ELEVATED)
                                    .border_1()
                                    .border_color(if sel {
                                        COLOR_ACCENT_ORANGE.opacity(0.45)
                                    } else {
                                        COLOR_BORDER
                                    })
                                    .hover(|s| s.bg(COLOR_BG_ELEVATED_HOVER))
                                    .cursor_pointer()
                                    .on_click({
                                        let state_entity = state_entity.clone();
                                        move |_, _, cx| {
                                            state_entity.update(cx, |s, cx| {
                                                if let Some(item) =
                                                    s.prepare_candidates.get_mut(idx)
                                                {
                                                    item.1 = !item.1;
                                                    cx.notify();
                                                }
                                            });
                                        }
                                    })
                                    .child(
                                        h_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .gap_2p5()
                                            .items_center()
                                            .child(
                                                Checkbox::new(format!("cand-chk-{}", idx))
                                                    .checked(sel)
                                                    .with_size(Size::Small)
                                                    .on_change({
                                                        let state_entity = state_entity.clone();
                                                        move |next, _, cx| {
                                                            let next_val = *next;
                                                            state_entity.update(cx, |s, cx| {
                                                                if let Some(item) = s.prepare_candidates.get_mut(idx) {
                                                                    item.1 = next_val;
                                                                    cx.notify();
                                                                }
                                                            });
                                                        }
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .truncate()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_size(rems(0.86))
                                                    .text_color(COLOR_TEXT_PRIMARY)
                                                    .child(name),
                                            )
                                            .child(
                                                div()
                                                    .flex_shrink_0()
                                                    .px_1p5()
                                                    .py_0p5()
                                                    .rounded_sm()
                                                    .bg(COLOR_BG_CANVAS)
                                                    .text_size(rems(0.7))
                                                    .text_color(COLOR_TEXT_MUTED)
                                                    .child(format_proc_count(pids_count)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .text_size(rems(0.78))
                                            .text_color(COLOR_TEXT_MUTED)
                                            .child(format!("{ram} MB RAM · {vram} MB VRAM")),
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
                    .pt_3()
                    .border_t_1()
                    .border_color(COLOR_BORDER)
                    .child(
                        div()
                            .text_size(rems(0.84))
                            .text_color(COLOR_TEXT_MUTED)
                            .child(if has_candidates {
                                format!("Освободится ~{total_ram_mib} MB RAM · ~{total_vram_mib} MB VRAM")
                            } else {
                                "Активных целей нет".to_string()
                            }),
                    )
                    .child(if has_candidates {
                        Button::new("terminate-hogs-btn")
                            .danger()
                            .with_size(Size::Medium)
                            .icon(icons::trash())
                            .label("Закрыть выбранные")
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
                            .outline()
                            .with_size(Size::Medium)
                            .icon(icons::check())
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

fn format_proc_count(n: usize) -> String {
    let rem100 = n % 100;
    let rem10 = n % 10;
    let word = if (11..=14).contains(&rem100) {
        "процессов"
    } else if rem10 == 1 {
        "процесс"
    } else if (2..=4).contains(&rem10) {
        "процесса"
    } else {
        "процессов"
    };
    format!("{n} {word}")
}
