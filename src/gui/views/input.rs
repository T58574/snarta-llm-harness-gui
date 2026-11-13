use crate::gui::commands::COMMANDS;
use crate::gui::state::AppState;
use crate::gui::theme::{
    COLOR_GRAPHITE_BORDER, COLOR_GRAPHITE_CANVAS, COLOR_GRAPHITE_ELEVATED, COLOR_PINK_PRIMARY,
    COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY,
};
use crate::vision;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Enter, Paste, Textarea, TextareaState};
use gpui_component::{Icon, Sizable, Size, h_flex, v_flex};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

const SVG_PAPERCLIP: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m21.44 11.05-9.19 9.19a6 6 0 0 1-8.49-8.49l8.57-8.57A4 4 0 1 1 18 8.84l-8.59 8.57a2 2 0 0 1-2.83-2.83l8.49-8.48"/></svg>"#;

pub struct InputDock {
    pub textarea: Entity<TextareaState>,
    pub file_pick_tx: Sender<PathBuf>,
    pub file_pick_rx: Receiver<PathBuf>,
}

impl InputDock {
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        let textarea = cx.new(|cx| {
            TextareaState::new(window, cx)
                .submit_on_enter(true)
                .auto_grow(1, 8)
                .placeholder("Напишите сообщение...")
        });
        let (file_pick_tx, file_pick_rx) = mpsc::channel();
        Self {
            textarea,
            file_pick_tx,
            file_pick_rx,
        }
    }

    pub fn poll_file_picker(&mut self, state: &mut AppState) -> bool {
        let mut changed = false;
        while let Ok(path) = self.file_pick_rx.try_recv() {
            match vision::load_image_file(&path) {
                Ok(_) => {
                    state.attached_images.push(path);
                    changed = true;
                }
                Err(e) => {
                    state.status_notice = Some(format!("Ошибка загрузки изображения: {e}"));
                    changed = true;
                }
            }
        }
        changed
    }

    pub fn render(
        &mut self,
        state_entity: &Entity<AppState>,
        _window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let state = state_entity.read(cx);
        let is_generating = state.is_generating;
        let attached = state.attached_images.clone();
        let status_notice = state.status_notice.clone();

        let current_text = self.textarea.read(cx).value().to_string();
        let is_slash_cmd = current_text.starts_with('/');
        let matching_commands = if is_slash_cmd {
            let q = current_text.trim();
            COMMANDS
                .iter()
                .filter(|c| {
                    c.name.starts_with(q)
                        || q == "/"
                        || c.aliases.iter().any(|a| a.starts_with(q))
                })
                .take(6)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        v_flex()
            .w_full()
            .border_t_1()
            .border_color(COLOR_GRAPHITE_BORDER)
            .bg(COLOR_GRAPHITE_CANVAS)
            .p_3()
            .gap_2()
            .children(status_notice.map(|notice| {
                h_flex()
                    .w_full()
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .bg(COLOR_GRAPHITE_ELEVATED)
                    .border_1()
                    .border_color(COLOR_PINK_PRIMARY)
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_size(rems(0.75))
                            .text_color(COLOR_TEXT_PRIMARY)
                            .child(notice),
                    )
                    .child(
                        Button::new("dismiss-notice-btn")
                            .ghost()
                            .with_size(Size::XSmall)
                            .label("✕")
                            .on_click({
                                let state_entity = state_entity.clone();
                                move |_, _, cx| {
                                    state_entity.update(cx, |s, cx| {
                                        s.status_notice = None;
                                        cx.notify();
                                    });
                                }
                            }),
                    )
            }))
            .children((!matching_commands.is_empty()).then(|| {
                h_flex()
                    .w_full()
                    .gap_1p5()
                    .items_center()
                    .flex_wrap()
                    .pb_1()
                    .children(matching_commands.into_iter().map(|cmd| {
                        Button::new(format!("slash-sug-{}", cmd.name))
                            .ghost()
                            .with_size(Size::XSmall)
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(COLOR_PINK_PRIMARY)
                                            .child(cmd.name),
                                    )
                                    .child(
                                        div()
                                            .text_size(rems(0.72))
                                            .text_color(COLOR_TEXT_MUTED)
                                            .child(cmd.description),
                                    ),
                            )
                            .on_click({
                                let textarea = self.textarea.clone();
                                let name = cmd.name;
                                move |_, window, cx| {
                                    textarea.update(cx, |t, cx| {
                                        t.set_value(format!("{name} "), window, cx);
                                    });
                                }
                            })
                    }))
            }))
            .children((!attached.is_empty()).then(|| {
                h_flex()
                    .gap_2p5()
                    .items_center()
                    .flex_wrap()
                    .children(attached.into_iter().enumerate().map(|(idx, path)| {
                        let fname = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        h_flex()
                            .items_center()
                            .gap_2()
                            .p_1p5()
                            .rounded_md()
                            .bg(COLOR_GRAPHITE_ELEVATED)
                            .border_1()
                            .border_color(COLOR_GRAPHITE_BORDER)
                            .text_size(rems(0.75))
                            .child(
                                img(path.clone())
                                    .w(px(48.0))
                                    .h(px(48.0))
                                    .rounded_sm()
                                    .object_fit(ObjectFit::Cover),
                            )
                            .child(
                                div()
                                    .max_w(px(180.0))
                                    .truncate()
                                    .text_color(COLOR_TEXT_PRIMARY)
                                    .child(fname),
                            )
                            .child(
                                Button::new(format!("remove-img-{}", idx))
                                    .ghost()
                                    .with_size(Size::XSmall)
                                    .label("✕")
                                    .on_click({
                                        let state_entity = state_entity.clone();
                                        move |_, _, cx| {
                                            state_entity.update(cx, |s, cx| {
                                                if idx < s.attached_images.len() {
                                                    s.attached_images.remove(idx);
                                                    cx.notify();
                                                }
                                            });
                                        }
                                    }),
                            )
                    }))
            }))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("attach-image-btn")
                            .secondary()
                            .with_size(Size::Medium)
                            .icon(Icon::default().data(SVG_PAPERCLIP))
                            .tooltip("Прикрепить изображение")
                            .on_click({
                                let tx = self.file_pick_tx.clone();
                                move |_, _, _cx| {
                                    let tx = tx.clone();
                                    std::thread::spawn(move || {
                                        if let Some(p) = vision::pick_image_file_dialog() {
                                            let _ = tx.send(p);
                                        }
                                    });
                                }
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .capture_action({
                                let state_entity = state_entity.clone();
                                move |_: &Paste, _window, cx| {
                                    let dir = state_entity.read(cx).paths.attachments_dir();
                                    if let Ok(Some(saved)) = vision::read_clipboard_image(&dir) {
                                        state_entity.update(cx, |s, cx| {
                                            s.attached_images.push(saved.path);
                                            cx.notify();
                                        });
                                        return;
                                    }
                                    cx.propagate();
                                }
                            })
                            .on_action({
                                let state_entity = state_entity.clone();
                                let textarea = self.textarea.clone();
                                move |action: &Enter, window, cx| {
                                    if !action.shift {
                                        send_input_message(&textarea, &state_entity, window, cx);
                                    }
                                }
                            })
                            .capture_key_down({
                                let state_entity = state_entity.clone();
                                let textarea = self.textarea.clone();
                                move |event, window, cx| {
                                    let key = event.keystroke.key.as_str();
                                    let is_ctrl = event.keystroke.modifiers.control
                                        || event.keystroke.modifiers.platform;
                                    let is_v = key.eq_ignore_ascii_case("v")
                                        || key == "м"
                                        || key == "М"
                                        || event.keystroke.key_char.as_deref() == Some("м")
                                        || event.keystroke.key_char.as_deref() == Some("v");

                                    if is_ctrl && is_v {
                                        let dir = state_entity.read(cx).paths.attachments_dir();
                                        if let Ok(Some(saved)) = vision::read_clipboard_image(&dir)
                                        {
                                            cx.stop_propagation();
                                            state_entity.update(cx, |s, cx| {
                                                s.attached_images.push(saved.path);
                                                cx.notify();
                                            });
                                            return;
                                        }
                                    }

                                    if (key.eq_ignore_ascii_case("enter") || key == "\r")
                                        && !event.keystroke.modifiers.shift
                                    {
                                        cx.stop_propagation();
                                        send_input_message(&textarea, &state_entity, window, cx);
                                    }
                                }
                            })
                            .child(Textarea::new(&self.textarea).bordered(true)),
                    )
                    .child(if is_generating {
                        Button::new("stop-btn")
                            .danger()
                            .with_size(Size::Medium)
                            .label("Остановить")
                            .on_click({
                                let state_entity = state_entity.clone();
                                move |_, _, cx| {
                                    state_entity.update(cx, |s, cx| {
                                        s.cancel_generation();
                                        cx.notify();
                                    });
                                }
                            })
                    } else {
                        Button::new("send-btn")
                            .primary()
                            .with_size(Size::Medium)
                            .label("Отправить")
                            .on_click({
                                let state_entity = state_entity.clone();
                                let textarea = self.textarea.clone();
                                move |_, window, cx| {
                                    send_input_message(&textarea, &state_entity, window, cx);
                                }
                            })
                    }),
            )
    }
}

fn send_input_message(
    textarea: &Entity<TextareaState>,
    state_entity: &Entity<AppState>,
    window: &mut Window,
    cx: &mut App,
) {
    let text = textarea.read(cx).value().to_string();
    if !text.trim().is_empty() || !state_entity.read(cx).attached_images.is_empty() {
        textarea.update(cx, |t, cx| {
            t.clean(window, cx);
        });
        state_entity.update(cx, |s, cx| {
            s.send_message(text);
            cx.notify();
        });
    }
}
