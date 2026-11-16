use super::DUR_AMBIENT;
use crate::gui::state::AppState;
use crate::gui::theme::{
    COLOR_ACCENT_BLUE, COLOR_ACCENT_BLUE_MUTED, COLOR_ACCENT_GREEN, COLOR_ACCENT_GREEN_MUTED,
    COLOR_ACCENT_ORANGE, COLOR_ACCENT_ORANGE_MUTED, COLOR_BG_CANVAS, COLOR_BG_ELEVATED,
    COLOR_BORDER, COLOR_DANGER, COLOR_DANGER_MUTED, COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY,
    COLOR_WARNING,
};
use crate::persona::PersonaGender;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{ActiveTheme, Sizable, Size, h_flex, v_flex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AvatarEmotion {
    #[default]
    Neutral,
    Happy,
    Thinking,
    Focused,
    Curious,
    Surprised,
    Skeptical,
    Playful,
    Proud,
    Warm,
    Alert,
}

impl AvatarEmotion {
    pub fn from_str(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "happy" | "joy" | "smile" => Self::Happy,
            "thinking" | "thought" | "ponder" => Self::Thinking,
            "focused" | "work" | "coding" | "tool" => Self::Focused,
            "curious" | "wonder" => Self::Curious,
            "surprised" | "wow" | "shocked" => Self::Surprised,
            "skeptical" | "doubt" => Self::Skeptical,
            "playful" | "wink" | "cheeky" => Self::Playful,
            "proud" | "confident" => Self::Proud,
            "warm" | "kind" | "friendly" => Self::Warm,
            "alert" | "warn" | "warning" | "error" => Self::Alert,
            _ => Self::Neutral,
        }
    }

    pub fn label_ru(self) -> &'static str {
        match self {
            Self::Neutral => "Спокойный",
            Self::Happy => "Радость",
            Self::Thinking => "Размышление",
            Self::Focused => "Фокус",
            Self::Curious => "Интерес",
            Self::Surprised => "Удивление",
            Self::Skeptical => "Сомнение",
            Self::Playful => "Игривый",
            Self::Proud => "Уверенность",
            Self::Warm => "Тепло",
            Self::Alert => "Внимание",
        }
    }

    pub fn colors(self) -> (Hsla, Hsla) {
        match self {
            Self::Happy | Self::Warm | Self::Playful | Self::Proud => {
                (COLOR_ACCENT_ORANGE, COLOR_ACCENT_ORANGE_MUTED)
            }
            Self::Thinking | Self::Curious | Self::Focused => {
                (COLOR_ACCENT_BLUE, COLOR_ACCENT_BLUE_MUTED)
            }
            Self::Neutral => (COLOR_ACCENT_GREEN, COLOR_ACCENT_GREEN_MUTED),
            Self::Surprised | Self::Skeptical => (COLOR_WARNING, COLOR_ACCENT_ORANGE_MUTED),
            Self::Alert => (COLOR_DANGER, COLOR_DANGER_MUTED),
        }
    }
}

#[cfg(test)]
pub const ASCII_FACE_WIDTH: usize = 15;

pub(crate) struct AsciiFace {
    pub top: &'static str,
    pub hair: &'static str,
    pub brows: &'static str,
    pub eyes: &'static str,
    pub mouth: &'static str,
    pub chin: &'static str,
}

impl AsciiFace {
    pub fn lines(&self) -> [&'static str; 6] {
        [self.top, self.hair, self.brows, self.eyes, self.mouth, self.chin]
    }
}

pub(crate) fn face_art(gender: PersonaGender, emotion: AvatarEmotion) -> AsciiFace {
    const F_TOP: &str = "   .~~~~~~~.   ";
    const F_HAIR: &str = "  / ~~~~~~~ \\  ";
    const F_CHIN: &str = "  '-._____.-'  ";

    const M_TOP: &str = "   .-------.   ";
    const M_HAIR: &str = "  /|||||||||\\  ";
    const M_CHIN: &str = "   \\_______/   ";

    match (gender, emotion) {
        // Female persona faces (soft long hair frame, 15-col fixed-width pure ASCII)
        (PersonaGender::Female, AvatarEmotion::Neutral) => AsciiFace {
            top: F_TOP,
            hair: F_HAIR,
            brows: " /|  -   -  |\\ ",
            eyes: " || (o) (o) || ",
            mouth: " \\|   '-'   |/ ",
            chin: F_CHIN,
        },
        (PersonaGender::Female, AvatarEmotion::Happy) => AsciiFace {
            top: F_TOP,
            hair: F_HAIR,
            brows: " /|  ^   ^  |\\ ",
            eyes: " || (^) (^) || ",
            mouth: " \\|  \\___/  |/ ",
            chin: F_CHIN,
        },
        (PersonaGender::Female, AvatarEmotion::Thinking) => AsciiFace {
            top: F_TOP,
            hair: F_HAIR,
            brows: " /|  ~   -  |\\ ",
            eyes: " || (o) (.) || ",
            mouth: " \\|   .o.   |/ ",
            chin: F_CHIN,
        },
        (PersonaGender::Female, AvatarEmotion::Focused) => AsciiFace {
            top: F_TOP,
            hair: F_HAIR,
            brows: " /|  \\   /  |\\ ",
            eyes: " || (*) (*) || ",
            mouth: " \\|   ---   |/ ",
            chin: F_CHIN,
        },
        (PersonaGender::Female, AvatarEmotion::Curious) => AsciiFace {
            top: F_TOP,
            hair: F_HAIR,
            brows: " /|  ^   -  |\\ ",
            eyes: " || (O) (o) || ",
            mouth: " \\|   .?.   |/ ",
            chin: F_CHIN,
        },
        (PersonaGender::Female, AvatarEmotion::Surprised) => AsciiFace {
            top: F_TOP,
            hair: F_HAIR,
            brows: " /|  ^   ^  |\\ ",
            eyes: " || (O) (O) || ",
            mouth: " \\|   (o)   |/ ",
            chin: F_CHIN,
        },
        (PersonaGender::Female, AvatarEmotion::Skeptical) => AsciiFace {
            top: F_TOP,
            hair: F_HAIR,
            brows: " /|  ~   \\  |\\ ",
            eyes: " || (o) (-) || ",
            mouth: " \\|   ._/   |/ ",
            chin: F_CHIN,
        },
        (PersonaGender::Female, AvatarEmotion::Playful) => AsciiFace {
            top: F_TOP,
            hair: F_HAIR,
            brows: " /|  ^   ~  |\\ ",
            eyes: " || (o) (>) || ",
            mouth: " \\|   \\_/   |/ ",
            chin: F_CHIN,
        },
        (PersonaGender::Female, AvatarEmotion::Proud) => AsciiFace {
            top: F_TOP,
            hair: F_HAIR,
            brows: " /|  ^   ^  |\\ ",
            eyes: " || (*) (*) || ",
            mouth: " \\|  \\===/  |/ ",
            chin: F_CHIN,
        },
        (PersonaGender::Female, AvatarEmotion::Warm) => AsciiFace {
            top: F_TOP,
            hair: F_HAIR,
            brows: " /|  ~   ~  |\\ ",
            eyes: " || (o) (o) || ",
            mouth: " \\|  `---'  |/ ",
            chin: F_CHIN,
        },
        (PersonaGender::Female, AvatarEmotion::Alert) => AsciiFace {
            top: F_TOP,
            hair: F_HAIR,
            brows: " /|  \\   /  |\\ ",
            eyes: " || (!) (!) || ",
            mouth: " \\|   ___   |/ ",
            chin: F_CHIN,
        },

        // Male persona faces (short cropped hair + structured jaw, 15-col fixed-width pure ASCII)
        (PersonaGender::Male, AvatarEmotion::Neutral) => AsciiFace {
            top: M_TOP,
            hair: M_HAIR,
            brows: "  |  -   -  |  ",
            eyes: "  | [o] [o] |  ",
            mouth: "  |   ---   |  ",
            chin: M_CHIN,
        },
        (PersonaGender::Male, AvatarEmotion::Happy) => AsciiFace {
            top: M_TOP,
            hair: M_HAIR,
            brows: "  |  ^   ^  |  ",
            eyes: "  | [^] [^] |  ",
            mouth: "  |  \\___/  |  ",
            chin: M_CHIN,
        },
        (PersonaGender::Male, AvatarEmotion::Thinking) => AsciiFace {
            top: M_TOP,
            hair: M_HAIR,
            brows: "  |  ~   -  |  ",
            eyes: "  | [o] [.] |  ",
            mouth: "  |   .~.   |  ",
            chin: M_CHIN,
        },
        (PersonaGender::Male, AvatarEmotion::Focused) => AsciiFace {
            top: M_TOP,
            hair: M_HAIR,
            brows: "  |  \\   /  |  ",
            eyes: "  | [=] [=] |  ",
            mouth: "  |   ---   |  ",
            chin: M_CHIN,
        },
        (PersonaGender::Male, AvatarEmotion::Curious) => AsciiFace {
            top: M_TOP,
            hair: M_HAIR,
            brows: "  |  ^   -  |  ",
            eyes: "  | [O] [o] |  ",
            mouth: "  |   .o.   |  ",
            chin: M_CHIN,
        },
        (PersonaGender::Male, AvatarEmotion::Surprised) => AsciiFace {
            top: M_TOP,
            hair: M_HAIR,
            brows: "  |  ^   ^  |  ",
            eyes: "  | [O] [O] |  ",
            mouth: "  |   (O)   |  ",
            chin: M_CHIN,
        },
        (PersonaGender::Male, AvatarEmotion::Skeptical) => AsciiFace {
            top: M_TOP,
            hair: M_HAIR,
            brows: "  |  ~   \\  |  ",
            eyes: "  | [o] [-] |  ",
            mouth: "  |   ._/   |  ",
            chin: M_CHIN,
        },
        (PersonaGender::Male, AvatarEmotion::Playful) => AsciiFace {
            top: M_TOP,
            hair: M_HAIR,
            brows: "  |  ^   ~  |  ",
            eyes: "  | [o] [>] |  ",
            mouth: "  |   \\_/   |  ",
            chin: M_CHIN,
        },
        (PersonaGender::Male, AvatarEmotion::Proud) => AsciiFace {
            top: M_TOP,
            hair: M_HAIR,
            brows: "  |  ^   ^  |  ",
            eyes: "  | [*] [*] |  ",
            mouth: "  |  \\===/  |  ",
            chin: M_CHIN,
        },
        (PersonaGender::Male, AvatarEmotion::Warm) => AsciiFace {
            top: M_TOP,
            hair: M_HAIR,
            brows: "  |  ~   ~  |  ",
            eyes: "  | [o] [o] |  ",
            mouth: "  |  `---'  |  ",
            chin: M_CHIN,
        },
        (PersonaGender::Male, AvatarEmotion::Alert) => AsciiFace {
            top: M_TOP,
            hair: M_HAIR,
            brows: "  |  \\   /  |  ",
            eyes: "  | [!] [!] |  ",
            mouth: "  |   ___   |  ",
            chin: M_CHIN,
        },
    }
}

fn render_face_grid(
    art: &AsciiFace,
    font_size: Pixels,
    line_height: Pixels,
    color: Hsla,
    mono: SharedString,
) -> impl IntoElement {
    v_flex()
        .items_start()
        .font_family(mono)
        .text_size(font_size)
        .line_height(line_height)
        .text_color(color)
        .children(
            art.lines()
                .into_iter()
                .map(|line| div().whitespace_nowrap().child(line)),
        )
}

/// Renders a miniature static ASCII portrait for persona picker cards.
pub fn render_mini_avatar(
    gender: PersonaGender,
    emotion: AvatarEmotion,
    cx: &App,
) -> impl IntoElement {
    let art = face_art(gender, emotion);
    let (accent, accent_bg) = match gender {
        PersonaGender::Female => (COLOR_ACCENT_ORANGE, COLOR_ACCENT_ORANGE_MUTED),
        PersonaGender::Male => (COLOR_ACCENT_BLUE, COLOR_ACCENT_BLUE_MUTED),
    };
    let mono = cx.theme().mono_font_family.clone();

    v_flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .px_1p5()
        .py_1()
        .rounded_md()
        .bg(accent_bg)
        .border_1()
        .border_color(accent.opacity(0.4))
        .child(render_face_grid(&art, px(8.5), px(9.5), accent, mono))
}

/// Renders the compact live ASCII persona card for the sidebar HUD.
pub fn render_sidebar_avatar(
    persona_name: &str,
    gender: PersonaGender,
    emotion: AvatarEmotion,
    thought: Option<&str>,
    is_generating: bool,
    state_entity: &Entity<AppState>,
    cx: &App,
) -> impl IntoElement {
    let art = face_art(gender, emotion);
    let (accent, accent_bg) = emotion.colors();
    let mono = cx.theme().mono_font_family.clone();
    let status_text = thought
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| emotion.label_ru().to_string());

    let card = v_flex()
        .w_full()
        .p_2()
        .rounded_lg()
        .bg(COLOR_BG_CANVAS)
        .border_1()
        .border_color(if is_generating { accent } else { COLOR_BORDER })
        .gap_2()
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .child(
                    v_flex()
                        .items_center()
                        .justify_center()
                        .flex_shrink_0()
                        .px_1p5()
                        .py_1()
                        .rounded_md()
                        .bg(accent_bg)
                        .border_1()
                        .border_color(accent.opacity(0.45))
                        .child(render_face_grid(&art, px(9.0), px(10.5), accent, mono)),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1p5()
                        .child(
                            div()
                                .w_full()
                                .truncate()
                                .text_size(rems(0.85))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(COLOR_TEXT_PRIMARY)
                                .child(persona_name.to_string()),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    Button::new("sidebar-toggle-gender")
                                        .secondary()
                                        .outline()
                                        .with_size(Size::XSmall)
                                        .label(gender.label_ru())
                                        .tooltip("Нажмите, чтобы переключить пол ASCII-арта")
                                        .on_click({
                                            let state_entity = state_entity.clone();
                                            move |_, _, cx| {
                                                state_entity.update(cx, |s, cx| {
                                                    s.toggle_active_persona_gender();
                                                    cx.notify();
                                                });
                                            }
                                        }),
                                ),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_1p5()
                                .child(
                                    div()
                                        .w(px(6.0))
                                        .h(px(6.0))
                                        .rounded_full()
                                        .bg(accent),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_size(rems(0.75))
                                        .text_color(accent)
                                        .child(status_text),
                                ),
                        ),
                ),
        );

    if is_generating {
        card.with_animation(
            "sidebar-avatar-pulse",
            Animation::new(DUR_AMBIENT)
                .repeat()
                .with_easing(bounce(ease_in_out)),
            move |el, delta| el.border_color(accent.opacity(0.45 + delta * 0.55)),
        )
        .into_any_element()
    } else {
        card.into_any_element()
    }
}

/// Renders the hero ASCII persona stage for an empty chat session.
pub fn render_hero_avatar(
    persona_name: &str,
    gender: PersonaGender,
    emotion: AvatarEmotion,
    thought: Option<&str>,
    state_entity: &Entity<AppState>,
    cx: &App,
) -> impl IntoElement {
    let art = face_art(gender, emotion);
    let (accent, accent_bg) = emotion.colors();
    let mono = cx.theme().mono_font_family.clone();

    v_flex()
        .items_center()
        .justify_center()
        .gap_3()
        .child(
            v_flex()
                .items_center()
                .justify_center()
                .px_5()
                .py_3p5()
                .rounded_xl()
                .bg(accent_bg)
                .border_1()
                .border_color(accent.opacity(0.55))
                .child(render_face_grid(&art, px(15.0), px(18.0), accent, mono))
                .with_animation(
                    "hero-ascii-breathe",
                    Animation::new(DUR_AMBIENT)
                        .repeat()
                        .with_easing(bounce(ease_in_out)),
                    move |el, delta| {
                        el.border_color(accent.opacity(0.35 + delta * 0.55))
                            .bg(accent.opacity(0.08 + delta * 0.08))
                    },
                ),
        )
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_size(rems(1.35))
                        .font_weight(FontWeight::BOLD)
                        .text_color(COLOR_TEXT_PRIMARY)
                        .child(persona_name.to_string()),
                )
                .child(
                    Button::new("hero-toggle-gender")
                        .secondary()
                        .outline()
                        .with_size(Size::XSmall)
                        .label(gender.label_ru())
                        .tooltip("Нажмите, чтобы переключить пол ASCII-арта")
                        .on_click({
                            let state_entity = state_entity.clone();
                            move |_, _, cx| {
                                state_entity.update(cx, |s, cx| {
                                    s.toggle_active_persona_gender();
                                    cx.notify();
                                });
                            }
                        }),
                )
                .child(
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_md()
                        .bg(COLOR_BG_ELEVATED)
                        .border_1()
                        .border_color(COLOR_BORDER)
                        .text_size(rems(0.72))
                        .text_color(accent)
                        .child(emotion.label_ru()),
                ),
        )
        .child(
            div()
                .text_size(rems(0.88))
                .text_color(COLOR_TEXT_MUTED)
                .child(
                    thought
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or("Напишите сообщение или перетащите вопрос в строку ввода.")
                        .to_string(),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn emotion_parser_and_faces_cover_both_genders_with_uniform_ascii_width() {
        let emotions = [
            ("happy", AvatarEmotion::Happy),
            ("thinking", AvatarEmotion::Thinking),
            ("focused", AvatarEmotion::Focused),
            ("curious", AvatarEmotion::Curious),
            ("surprised", AvatarEmotion::Surprised),
            ("skeptical", AvatarEmotion::Skeptical),
            ("playful", AvatarEmotion::Playful),
            ("proud", AvatarEmotion::Proud),
            ("warm", AvatarEmotion::Warm),
            ("alert", AvatarEmotion::Alert),
            ("unknown", AvatarEmotion::Neutral),
        ];

        for (raw, expected) in emotions {
            let parsed = AvatarEmotion::from_str(raw);
            assert_eq!(parsed, expected);
            let f_face = face_art(PersonaGender::Female, parsed);
            let m_face = face_art(PersonaGender::Male, parsed);
            assert_ne!(f_face.top, m_face.top);
            for line in f_face.lines().into_iter().chain(m_face.lines()) {
                assert!(line.is_ascii(), "non-ASCII char in line: {line:?}");
                assert_eq!(
                    line.chars().count(),
                    ASCII_FACE_WIDTH,
                    "line width mismatch in {line:?}"
                );
            }
        }
    }
}
