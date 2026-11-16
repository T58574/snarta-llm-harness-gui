use super::DUR_AMBIENT;
use crate::gui::theme::{
    COLOR_ACCENT_BLUE, COLOR_ACCENT_BLUE_MUTED, COLOR_ACCENT_GREEN, COLOR_ACCENT_GREEN_MUTED,
    COLOR_ACCENT_ORANGE, COLOR_ACCENT_ORANGE_MUTED, COLOR_BG_CANVAS, COLOR_BG_ELEVATED,
    COLOR_BORDER, COLOR_DANGER, COLOR_DANGER_MUTED, COLOR_TEXT_MUTED, COLOR_TEXT_PRIMARY,
    COLOR_WARNING,
};
use crate::persona::PersonaGender;
use gpui::*;
use gpui_component::{ActiveTheme, h_flex, v_flex};

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

pub(crate) struct AsciiFace {
    pub top: &'static str,
    pub brows: &'static str,
    pub eyes: &'static str,
    pub mouth: &'static str,
    pub chin: &'static str,
}

pub(crate) fn face_art(gender: PersonaGender, emotion: AvatarEmotion) -> AsciiFace {
    match (gender, emotion) {
        // Female persona faces
        (PersonaGender::Female, AvatarEmotion::Neutral) => AsciiFace {
            top: " .~\"\"\"~.",
            brows: "/  _   _  \\",
            eyes: "| (o) (o) |",
            mouth: "\\   ._.   /",
            chin: " '.___.'",
        },
        (PersonaGender::Female, AvatarEmotion::Happy) => AsciiFace {
            top: " .~\"\"\"~.",
            brows: "/  ^   ^  \\",
            eyes: "| (^) (^) |",
            mouth: "\\  \\___/  /",
            chin: " '.___.'",
        },
        (PersonaGender::Female, AvatarEmotion::Thinking) => AsciiFace {
            top: " .~\"\"\"~.",
            brows: "/  ~   _  \\",
            eyes: "| (o) (.) |",
            mouth: "\\   .o.   /",
            chin: " '.___.'",
        },
        (PersonaGender::Female, AvatarEmotion::Focused) => AsciiFace {
            top: " .~\"\"\"~.",
            brows: "/  \\   /  \\",
            eyes: "| [•] [•] |",
            mouth: "\\   ---   /",
            chin: " '.___.'",
        },
        (PersonaGender::Female, AvatarEmotion::Curious) => AsciiFace {
            top: " .~\"\"\"~.",
            brows: "/  ^   _  \\",
            eyes: "| (O) (o) |",
            mouth: "\\   .?.   /",
            chin: " '.___.'",
        },
        (PersonaGender::Female, AvatarEmotion::Surprised) => AsciiFace {
            top: " .~\"\"\"~.",
            brows: "/  ^   ^  \\",
            eyes: "| (O) (O) |",
            mouth: "\\   (o)   /",
            chin: " '.___.'",
        },
        (PersonaGender::Female, AvatarEmotion::Skeptical) => AsciiFace {
            top: " .~\"\"\"~.",
            brows: "/  ~   \\  \\",
            eyes: "| (o) (-) |",
            mouth: "\\   ._/   /",
            chin: " '.___.'",
        },
        (PersonaGender::Female, AvatarEmotion::Playful) => AsciiFace {
            top: " .~\"\"\"~.",
            brows: "/  ^   ~  \\",
            eyes: "| (o) (>) |",
            mouth: "\\   \\_/   /",
            chin: " '.___.'",
        },
        (PersonaGender::Female, AvatarEmotion::Proud) => AsciiFace {
            top: " .~\"\"\"~.",
            brows: "/  ^   ^  \\",
            eyes: "| (*) (*) |",
            mouth: "\\  \\===/  /",
            chin: " '.___.'",
        },
        (PersonaGender::Female, AvatarEmotion::Warm) => AsciiFace {
            top: " .~\"\"\"~.",
            brows: "/  ~   ~  \\",
            eyes: "| (•) (•) |",
            mouth: "\\  `---'  /",
            chin: " '.___.'",
        },
        (PersonaGender::Female, AvatarEmotion::Alert) => AsciiFace {
            top: " .~\"\"\"~.",
            brows: "/  \\   /  \\",
            eyes: "| (!) (!) |",
            mouth: "\\   ___   /",
            chin: " '.___.'",
        },

        // Male persona faces
        (PersonaGender::Male, AvatarEmotion::Neutral) => AsciiFace {
            top: " ._____.",
            brows: "|  _   _  |",
            eyes: "| [o] [o] |",
            mouth: "|   ___   |",
            chin: " \\_______/",
        },
        (PersonaGender::Male, AvatarEmotion::Happy) => AsciiFace {
            top: " ._____.",
            brows: "|  ^   ^  |",
            eyes: "| [^] [^] |",
            mouth: "|  \\___/  |",
            chin: " \\_______/",
        },
        (PersonaGender::Male, AvatarEmotion::Thinking) => AsciiFace {
            top: " ._____.",
            brows: "|  ~   _  |",
            eyes: "| [o] [.] |",
            mouth: "|   .~.   |",
            chin: " \\_______/",
        },
        (PersonaGender::Male, AvatarEmotion::Focused) => AsciiFace {
            top: " ._____.",
            brows: "|  \\   /  |",
            eyes: "| [=] [=] |",
            mouth: "|   ---   |",
            chin: " \\_______/",
        },
        (PersonaGender::Male, AvatarEmotion::Curious) => AsciiFace {
            top: " ._____.",
            brows: "|  ^   _  |",
            eyes: "| [O] [o] |",
            mouth: "|   .o.   |",
            chin: " \\_______/",
        },
        (PersonaGender::Male, AvatarEmotion::Surprised) => AsciiFace {
            top: " ._____.",
            brows: "|  ^   ^  |",
            eyes: "| [O] [O] |",
            mouth: "|   (O)   |",
            chin: " \\_______/",
        },
        (PersonaGender::Male, AvatarEmotion::Skeptical) => AsciiFace {
            top: " ._____.",
            brows: "|  ~   \\  |",
            eyes: "| [o] [-] |",
            mouth: "|   ._/   |",
            chin: " \\_______/",
        },
        (PersonaGender::Male, AvatarEmotion::Playful) => AsciiFace {
            top: " ._____.",
            brows: "|  ^   ~  |",
            eyes: "| [o] [>] |",
            mouth: "|   \\_/   |",
            chin: " \\_______/",
        },
        (PersonaGender::Male, AvatarEmotion::Proud) => AsciiFace {
            top: " ._____.",
            brows: "|  ^   ^  |",
            eyes: "| [*] [*] |",
            mouth: "|  \\===/  |",
            chin: " \\_______/",
        },
        (PersonaGender::Male, AvatarEmotion::Warm) => AsciiFace {
            top: " ._____.",
            brows: "|  ~   ~  |",
            eyes: "| [•] [•] |",
            mouth: "|  `---'  |",
            chin: " \\_______/",
        },
        (PersonaGender::Male, AvatarEmotion::Alert) => AsciiFace {
            top: " ._____.",
            brows: "|  \\   /  |",
            eyes: "| [!] [!] |",
            mouth: "|   ___   |",
            chin: " \\_______/",
        },
    }
}

/// Renders the compact live ASCII persona card for the sidebar HUD.
pub fn render_sidebar_avatar(
    persona_name: &str,
    gender: PersonaGender,
    emotion: AvatarEmotion,
    thought: Option<&str>,
    is_generating: bool,
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
        .p_2p5()
        .rounded_lg()
        .bg(COLOR_BG_CANVAS)
        .border_1()
        .border_color(if is_generating { accent } else { COLOR_BORDER })
        .gap_2()
        .child(
            h_flex()
                .items_center()
                .gap_2p5()
                .child(
                    v_flex()
                        .items_center()
                        .justify_center()
                        .px_2()
                        .py_1p5()
                        .rounded_md()
                        .bg(accent_bg)
                        .border_1()
                        .border_color(accent.opacity(0.45))
                        .font_family(mono)
                        .text_size(px(10.5))
                        .line_height(px(11.5))
                        .text_color(accent)
                        .child(art.top)
                        .child(art.brows)
                        .child(art.eyes)
                        .child(art.mouth)
                        .child(art.chin),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(
                            h_flex()
                                .items_center()
                                .justify_between()
                                .gap_1()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_size(rems(0.85))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(COLOR_TEXT_PRIMARY)
                                        .child(persona_name.to_string()),
                                )
                                .child(
                                    div()
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(COLOR_BG_ELEVATED)
                                        .text_size(rems(0.65))
                                        .text_color(COLOR_TEXT_MUTED)
                                        .child(gender.label_ru()),
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
                .px_6()
                .py_4()
                .rounded_xl()
                .bg(accent_bg)
                .border_1()
                .border_color(accent.opacity(0.55))
                .font_family(mono)
                .text_size(px(16.0))
                .line_height(px(19.0))
                .text_color(accent)
                .child(art.top)
                .child(art.brows)
                .child(art.eyes)
                .child(art.mouth)
                .child(art.chin)
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
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_md()
                        .bg(COLOR_BG_ELEVATED)
                        .border_1()
                        .border_color(COLOR_BORDER)
                        .text_size(rems(0.72))
                        .text_color(COLOR_TEXT_MUTED)
                        .child(format!("{} · {}", gender.label_ru(), emotion.label_ru())),
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
    fn emotion_parser_and_faces_cover_both_genders() {
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
            assert!(!f_face.eyes.is_empty());
            assert!(!m_face.eyes.is_empty());
            assert_ne!(f_face.top, m_face.top);
        }
    }
}
