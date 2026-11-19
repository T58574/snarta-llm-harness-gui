use crate::gui::anim::avatar::render_avatar_circle;
use crate::gui::icons::icons;
use crate::gui::state::AppState;
use crate::gui::views::MainWindow;
use crate::gui::theme::{
    COLOR_ACCENT_ORANGE, COLOR_ACCENT_ORANGE_MUTED, COLOR_BG_CANVAS, COLOR_BORDER_GLASS,
    COLOR_BORDER_GLASS_STRONG, COLOR_GLASS_MODAL, COLOR_GLASS_SURFACE, COLOR_TEXT_MUTED,
    COLOR_TEXT_PRIMARY,
};
use crate::persona::{Persona, PersonaSaveInput, crop_and_save_avatar, sanitize_persona_slug};
use crate::vision;
use gpui::InteractiveElement as _;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputState, Textarea, TextareaState};
use gpui_component::{Sizable, Size, h_flex, v_flex};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PersonasViewMode {
    List,
    Editor,
    Cropper,
}

pub struct PersonasModal {
    mode: PersonasViewMode,
    editing_stem: Option<String>,
    name_input: Entity<InputState>,
    age_input: Entity<InputState>,
    character_input: Entity<InputState>,
    modifiers_input: Entity<TextareaState>,
    avatar_rel: Option<String>,
    avatar_bg_rel: Option<String>,
    avatar_preview_path: Option<PathBuf>,
    crop_src_path: Option<PathBuf>,
    crop_zoom: f32,
    crop_pan_x: f32,
    crop_pan_y: f32,
    crop_dragging: bool,
    crop_last_pos: Option<Point<Pixels>>,
    error_msg: Option<String>,
    pick_tx: Sender<PathBuf>,
    pick_rx: Receiver<PathBuf>,
}

impl PersonasModal {
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("Например: Akane"));
        let age_input = cx.new(|cx| InputState::new(window, cx).placeholder("Например: 21"));
        let character_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Спокойная, внимательная, ироничная")
        });
        let modifiers_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 7)
                .placeholder("Стиль речи, правила поведения, ограничения...")
        });
        let (pick_tx, pick_rx) = mpsc::channel();

        Self {
            mode: PersonasViewMode::List,
            editing_stem: None,
            name_input,
            age_input,
            character_input,
            modifiers_input,
            avatar_rel: None,
            avatar_bg_rel: None,
            avatar_preview_path: None,
            crop_src_path: None,
            crop_zoom: 1.0,
            crop_pan_x: 0.0,
            crop_pan_y: 0.0,
            crop_dragging: false,
            crop_last_pos: None,
            error_msg: None,
            pick_tx,
            pick_rx,
        }
    }

    pub fn poll_file_picker(&mut self) -> bool {
        let mut updated = false;
        while let Ok(path) = self.pick_rx.try_recv() {
            self.crop_src_path = Some(path);
            self.crop_zoom = 1.2;
            self.crop_pan_x = 0.0;
            self.crop_pan_y = 0.0;
            self.error_msg = None;
            self.mode = PersonasViewMode::Cropper;
            updated = true;
        }
        updated
    }

    fn start_create(&mut self, window: &mut Window, cx: &mut App) {
        self.editing_stem = None;
        self.name_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.age_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.character_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.modifiers_input
            .update(cx, |s, cx| s.set_value("", window, cx));
        self.avatar_rel = None;
        self.avatar_bg_rel = None;
        self.avatar_preview_path = None;
        self.crop_src_path = None;
        self.error_msg = None;
        self.mode = PersonasViewMode::Editor;
    }

    fn start_edit(&mut self, p: &Persona, personas_dir: &std::path::Path, window: &mut Window, cx: &mut App) {
        self.editing_stem = Some(p.name.clone());
        self.name_input
            .update(cx, |s, cx| s.set_value(p.display_name.clone(), window, cx));
        self.age_input.update(cx, |s, cx| {
            s.set_value(p.age.clone().unwrap_or_default(), window, cx)
        });
        self.character_input.update(cx, |s, cx| {
            s.set_value(
                p.character
                    .clone()
                    .or_else(|| p.description.clone())
                    .unwrap_or_default(),
                window,
                cx,
            )
        });
        self.modifiers_input.update(cx, |s, cx| {
            s.set_value(
                p.modifiers
                    .clone()
                    .unwrap_or_else(|| p.system_prompt.clone()),
                window,
                cx,
            )
        });
        self.avatar_preview_path = p.avatar_path.clone();
        self.avatar_rel = p.avatar_path.as_ref().and_then(|ap| {
            ap.strip_prefix(personas_dir)
                .ok()
                .map(|r| r.to_string_lossy().replace('\\', "/"))
        });
        self.avatar_bg_rel = p.avatar_bg_path.as_ref().and_then(|bp| {
            bp.strip_prefix(personas_dir)
                .ok()
                .map(|r| r.to_string_lossy().replace('\\', "/"))
        });
        self.crop_src_path = None;
        self.error_msg = None;
        self.mode = PersonasViewMode::Editor;
    }

    pub fn render(
        &mut self,
        state_entity: &Entity<AppState>,
        window: &mut Window,
        cx: &mut Context<MainWindow>,
    ) -> impl IntoElement {
        let state = state_entity.read(cx);
        let active_persona = state.persona.name.clone();
        let personas = state.available_personas.clone();
        let personas_dir = state.paths.personas_dir();

        v_flex()
            .w(rems(46.0))
            .p_5()
            .rounded_2xl()
            .bg(COLOR_GLASS_MODAL)
            .border_1()
            .border_color(COLOR_BORDER_GLASS_STRONG)
            .shadow_2xl()
            .gap_4()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2p5()
                            .child(
                                icons::user()
                                    .with_size(Size::Medium)
                                    .text_color(COLOR_ACCENT_ORANGE),
                            )
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_size(rems(1.08))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(COLOR_TEXT_PRIMARY)
                                            .child(match self.mode {
                                                PersonasViewMode::List => "Персоны и аватары",
                                                PersonasViewMode::Editor => "Редактор персоны",
                                                PersonasViewMode::Cropper => "Кадрирование аватара",
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(rems(0.74))
                                            .text_color(COLOR_TEXT_MUTED)
                                            .child(match self.mode {
                                                PersonasViewMode::List => {
                                                    "Выберите персону или создайте новую в формате persona_Name.md"
                                                }
                                                PersonasViewMode::Editor => {
                                                    "Имя, возраст, характер и модификаторы сохраняются в .md"
                                                }
                                                PersonasViewMode::Cropper => {
                                                    "Перетащите фото и настройте масштаб для круглой аватарки"
                                                }
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1p5()
                            .children((self.mode == PersonasViewMode::List).then(|| {
                                Button::new("new-persona-btn")
                                    .primary()
                                    .with_size(Size::Small)
                                    .icon(icons::plus())
                                    .label("Создать")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.personas_modal.start_create(window, cx);
                                        cx.notify();
                                    }))
                            }))
                            .child(
                                Button::new("open-personas-folder")
                                    .ghost()
                                    .with_size(Size::Small)
                                    .icon(icons::folder())
                                    .tooltip("Открыть папку персон")
                                    .on_click({
                                        let dir = personas_dir.clone();
                                        move |_, _, _cx| {
                                            crate::vision::open_in_explorer(&dir);
                                        }
                                    }),
                            )
                            .child(
                                Button::new("close-personas")
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
            .children(self.error_msg.as_ref().map(|err| {
                div()
                    .px_3()
                    .py_2()
                    .rounded_lg()
                    .bg(crate::gui::theme::COLOR_DANGER_MUTED)
                    .border_1()
                    .border_color(crate::gui::theme::COLOR_DANGER)
                    .text_size(rems(0.78))
                    .text_color(COLOR_TEXT_PRIMARY)
                    .child(err.clone())
            }))
            .child(match self.mode {
                PersonasViewMode::List => self
                    .render_list(personas, &active_persona, &personas_dir, state_entity, cx)
                    .into_any_element(),
                PersonasViewMode::Editor => self
                    .render_editor(state_entity, &personas_dir, window, cx)
                    .into_any_element(),
                PersonasViewMode::Cropper => self
                    .render_cropper(&personas_dir, cx)
                    .into_any_element(),
            })
    }

    fn render_list(
        &self,
        personas: Vec<Persona>,
        active_persona: &str,
        personas_dir: &std::path::Path,
        state_entity: &Entity<AppState>,
        cx: &mut Context<MainWindow>,
    ) -> impl IntoElement {
        let dir_owned = personas_dir.to_path_buf();
        v_flex()
            .id("personas-scroll")
            .max_h(rems(28.0))
            .overflow_y_scroll()
            .gap_2p5()
            .children(personas.into_iter().map(|p| {
                let is_active = p.name == active_persona;
                let pname = p.name.clone();
                let display = p.display_name.clone();
                let meta_line = match (&p.age, &p.character, &p.description) {
                    (Some(a), Some(c), _) => format!("{a} · {c}"),
                    (Some(a), None, _) => format!("Возраст: {a}"),
                    (None, Some(c), _) => c.clone(),
                    (None, None, Some(d)) => d.clone(),
                    _ => String::new(),
                };
                let prompt_preview = {
                    let clean = p.system_prompt.trim();
                    let chars: Vec<char> = clean.chars().collect();
                    if chars.len() > 180 {
                        format!("{}...", chars[..180].iter().collect::<String>())
                    } else {
                        clean.to_string()
                    }
                };
                let p_path = p.path.clone();
                let p_clone = p.clone();
                let dir_for_edit = dir_owned.clone();

                v_flex()
                    .p_3()
                    .rounded_xl()
                    .bg(if is_active {
                        COLOR_ACCENT_ORANGE_MUTED
                    } else {
                        COLOR_GLASS_SURFACE
                    })
                    .border_1()
                    .border_color(if is_active {
                        COLOR_ACCENT_ORANGE.opacity(0.65)
                    } else {
                        COLOR_BORDER_GLASS
                    })
                    .gap_2()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .gap_3()
                            .child(
                                h_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .items_center()
                                    .gap_2p5()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, {
                                        let state_entity = state_entity.clone();
                                        let pname = pname.clone();
                                        move |_, _, cx| {
                                            state_entity.update(cx, |s, cx| {
                                                s.switch_persona(&pname);
                                                s.active_modal = None;
                                                cx.notify();
                                            });
                                        }
                                    })
                                    .child(render_avatar_circle(
                                        &display,
                                        p.avatar_path.as_deref(),
                                        40.0,
                                    ))
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .truncate()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(if is_active {
                                                        COLOR_ACCENT_ORANGE
                                                    } else {
                                                        COLOR_TEXT_PRIMARY
                                                    })
                                                    .child(display),
                                            )
                                            .children((!meta_line.is_empty()).then(|| {
                                                div()
                                                    .truncate()
                                                    .text_size(rems(0.75))
                                                    .text_color(COLOR_TEXT_MUTED)
                                                    .child(meta_line)
                                            })),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        Button::new(format!("edit-persona-{}", pname))
                                            .ghost()
                                            .with_size(Size::XSmall)
                                            .icon(icons::settings())
                                            .label("Изменить")
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.personas_modal.start_edit(
                                                    &p_clone,
                                                    &dir_for_edit,
                                                    window,
                                                    cx,
                                                );
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new(format!("open-persona-file-{}", pname))
                                            .ghost()
                                            .with_size(Size::XSmall)
                                            .icon(icons::folder_open())
                                            .tooltip("Открыть .md файл")
                                            .on_click(move |_, _, _cx| {
                                                crate::vision::open_in_explorer(&p_path);
                                            }),
                                    )
                                    .child(if is_active {
                                        Button::new(format!("active-persona-{}", pname))
                                            .primary()
                                            .with_size(Size::XSmall)
                                            .icon(icons::check())
                                            .label("Активна")
                                    } else {
                                        Button::new(format!("select-persona-{}", pname))
                                            .secondary()
                                            .outline()
                                            .with_size(Size::XSmall)
                                            .label("Выбрать")
                                            .on_click({
                                                let state_entity = state_entity.clone();
                                                let pname = pname.clone();
                                                move |_, _, cx| {
                                                    state_entity.update(cx, |s, cx| {
                                                        s.switch_persona(&pname);
                                                        s.active_modal = None;
                                                        cx.notify();
                                                    });
                                                }
                                            })
                                    }),
                            ),
                    )
                    .children((!prompt_preview.is_empty()).then(|| {
                        div()
                            .text_size(rems(0.76))
                            .text_color(COLOR_TEXT_MUTED)
                            .child(prompt_preview)
                    }))
            }))
    }

    fn render_editor(
        &self,
        state_entity: &Entity<AppState>,
        _personas_dir: &std::path::Path,
        _window: &mut Window,
        cx: &mut Context<MainWindow>,
    ) -> impl IntoElement {
        let preview_name = self.name_input.read(cx).value().to_string();
        let display_for_badge = if preview_name.trim().is_empty() {
            "A"
        } else {
            preview_name.trim()
        };

        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .gap_4()
                    .p_3()
                    .rounded_xl()
                    .bg(COLOR_GLASS_SURFACE)
                    .border_1()
                    .border_color(COLOR_BORDER_GLASS)
                    .child(render_avatar_circle(
                        display_for_badge,
                        self.avatar_preview_path.as_deref(),
                        68.0,
                    ))
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_size(rems(0.84))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(COLOR_TEXT_PRIMARY)
                                    .child("Аватарка персоны"),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("pick-avatar-photo-btn")
                                            .secondary()
                                            .outline()
                                            .with_size(Size::Small)
                                            .icon(icons::image())
                                            .label("Выбрать фото...")
                                            .on_click({
                                                let tx = self.pick_tx.clone();
                                                move |_, _, _cx| {
                                                    let tx = tx.clone();
                                                    std::thread::spawn(move || {
                                                        if let Some(p) =
                                                            vision::pick_image_file_dialog()
                                                        {
                                                            let _ = tx.send(p);
                                                        }
                                                    });
                                                }
                                            }),
                                    )
                                    .children(self.avatar_preview_path.is_some().then(|| {
                                        Button::new("clear-avatar-photo-btn")
                                            .ghost()
                                            .with_size(Size::Small)
                                            .icon(icons::trash())
                                            .label("Убрать")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.personas_modal.avatar_rel = None;
                                                this.personas_modal.avatar_bg_rel = None;
                                                this.personas_modal.avatar_preview_path = None;
                                                cx.notify();
                                            }))
                                    })),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(rems(0.76))
                                    .text_color(COLOR_TEXT_MUTED)
                                    .child("Имя"),
                            )
                            .child(Input::new(&self.name_input)),
                    )
                    .child(
                        v_flex()
                            .w(px(140.0))
                            .gap_1()
                            .child(
                                div()
                                    .text_size(rems(0.76))
                                    .text_color(COLOR_TEXT_MUTED)
                                    .child("Возраст"),
                            )
                            .child(Input::new(&self.age_input)),
                    ),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_size(rems(0.76))
                            .text_color(COLOR_TEXT_MUTED)
                            .child("Характер"),
                    )
                    .child(Input::new(&self.character_input)),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_size(rems(0.76))
                            .text_color(COLOR_TEXT_MUTED)
                            .child("Кастом модификаторы и инструкции"),
                    )
                    .child(Textarea::new(&self.modifiers_input).bordered(true)),
            )
            .child(
                h_flex()
                    .justify_between()
                    .pt_2()
                    .child(
                        Button::new("persona-editor-back")
                            .ghost()
                            .with_size(Size::Small)
                            .label("← К списку")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.personas_modal.mode = PersonasViewMode::List;
                                this.personas_modal.error_msg = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("persona-editor-save")
                            .primary()
                            .with_size(Size::Small)
                            .icon(icons::check())
                            .label("Сохранить в persona_Name.md")
                            .on_click({
                                let state_entity = state_entity.clone();
                                cx.listener(move |this, _, _, cx| {
                                    let pm = &mut this.personas_modal;
                                    let name = pm.name_input.read(cx).value().to_string();
                                    let age = pm.age_input.read(cx).value().to_string();
                                    let character =
                                        pm.character_input.read(cx).value().to_string();
                                    let modifiers =
                                        pm.modifiers_input.read(cx).value().to_string();
                                    let input = PersonaSaveInput {
                                        existing_file_stem: pm.editing_stem.clone(),
                                        display_name: name,
                                        age,
                                        character,
                                        modifiers,
                                        avatar_rel: pm.avatar_rel.clone(),
                                        avatar_bg_rel: pm.avatar_bg_rel.clone(),
                                    };
                                    let res = state_entity
                                        .update(cx, |s, _| s.save_persona(&input, true));
                                    match res {
                                        Ok(_) => {
                                            pm.error_msg = None;
                                            pm.mode = PersonasViewMode::List;
                                        }
                                        Err(err) => {
                                            pm.error_msg = Some(err);
                                        }
                                    }
                                    cx.notify();
                                })
                            }),
                    ),
            )
    }

    fn render_cropper(
        &self,
        personas_dir: &std::path::Path,
        cx: &mut Context<MainWindow>,
    ) -> impl IntoElement {
        let src_path = self.crop_src_path.clone();
        let zoom = self.crop_zoom;
        let pan_x = self.crop_pan_x;
        let pan_y = self.crop_pan_y;
        let viewport_px = 240.0_f32;
        let scaled_px = viewport_px * zoom;
        let max_shift = (scaled_px - viewport_px) / 2.0;
        let offset_x = -(scaled_px - viewport_px) / 2.0 - pan_x * max_shift;
        let offset_y = -(scaled_px - viewport_px) / 2.0 - pan_y * max_shift;

        let name_val = self.name_input.read(cx).value().to_string();
        let slug = if name_val.trim().is_empty() {
            format!(
                "custom_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            )
        } else {
            sanitize_persona_slug(&name_val)
        };
        let personas_dir_owned = personas_dir.to_path_buf();

        v_flex()
            .items_center()
            .gap_4()
            .child(
                div()
                    .id("avatar-crop-viewport")
                    .relative()
                    .w(px(viewport_px))
                    .h(px(viewport_px))
                    .rounded_xl()
                    .overflow_hidden()
                    .bg(COLOR_BG_CANVAS)
                    .border_1()
                    .border_color(COLOR_BORDER_GLASS_STRONG)
                    .cursor_grab()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, ev: &MouseDownEvent, _, _| {
                            this.personas_modal.crop_dragging = true;
                            this.personas_modal.crop_last_pos = Some(ev.position);
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, _| {
                            this.personas_modal.crop_dragging = false;
                            this.personas_modal.crop_last_pos = None;
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                        let pm = &mut this.personas_modal;
                        if pm.crop_dragging {
                            if let Some(prev) = pm.crop_last_pos {
                                let dx = (ev.position.x - prev.x) / px(1.0);
                                let dy = (ev.position.y - prev.y) / px(1.0);
                                let sens = 1.0 / (110.0 * pm.crop_zoom.max(1.0));
                                pm.crop_pan_x = (pm.crop_pan_x - dx * sens).clamp(-1.0, 1.0);
                                pm.crop_pan_y = (pm.crop_pan_y - dy * sens).clamp(-1.0, 1.0);
                                cx.notify();
                            }
                            pm.crop_last_pos = Some(ev.position);
                        }
                    }))
                    .children(src_path.clone().map(|p| {
                        img(p)
                            .absolute()
                            .left(px(offset_x))
                            .top(px(offset_y))
                            .w(px(scaled_px))
                            .h(px(scaled_px))
                            .object_fit(ObjectFit::Cover)
                    }))
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .rounded_full()
                            .border_2()
                            .border_color(COLOR_ACCENT_ORANGE.opacity(0.85)),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("crop-zoom-out")
                            .secondary()
                            .outline()
                            .with_size(Size::Small)
                            .label("−")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.personas_modal.crop_zoom =
                                    (this.personas_modal.crop_zoom - 0.2).clamp(1.0, 3.5);
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .px_2()
                            .text_size(rems(0.8))
                            .text_color(COLOR_TEXT_MUTED)
                            .child(format!("Масштаб: {:.1}x", zoom)),
                    )
                    .child(
                        Button::new("crop-zoom-in")
                            .secondary()
                            .outline()
                            .with_size(Size::Small)
                            .label("+")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.personas_modal.crop_zoom =
                                    (this.personas_modal.crop_zoom + 0.2).clamp(1.0, 3.5);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("crop-reset")
                            .ghost()
                            .with_size(Size::Small)
                            .label("Сброс")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.personas_modal.crop_zoom = 1.2;
                                this.personas_modal.crop_pan_x = 0.0;
                                this.personas_modal.crop_pan_y = 0.0;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .child(
                        Button::new("crop-cancel")
                            .ghost()
                            .with_size(Size::Small)
                            .label("Отмена")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.personas_modal.mode = PersonasViewMode::Editor;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("crop-apply")
                            .primary()
                            .with_size(Size::Small)
                            .icon(icons::check())
                            .label("Применить кадрирование")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let pm = &mut this.personas_modal;
                                if let Some(ref src) = pm.crop_src_path {
                                    match crop_and_save_avatar(
                                        src,
                                        &personas_dir_owned,
                                        &slug,
                                        pm.crop_zoom,
                                        pm.crop_pan_x,
                                        pm.crop_pan_y,
                                    ) {
                                        Ok((av_rel, bg_rel)) => {
                                            pm.avatar_preview_path =
                                                Some(personas_dir_owned.join(&av_rel));
                                            pm.avatar_rel = Some(av_rel);
                                            pm.avatar_bg_rel = Some(bg_rel);
                                            pm.error_msg = None;
                                            pm.mode = PersonasViewMode::Editor;
                                        }
                                        Err(e) => {
                                            pm.error_msg = Some(format!("Ошибка кадрирования: {e}"));
                                        }
                                    }
                                }
                                cx.notify();
                            })),
                    ),
            )
    }
}
