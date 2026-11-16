use crate::gpu;
use crate::gui::icons::icons;
use crate::gui::state::AppState;
use crate::gui::theme::{
    COLOR_ACCENT_BLUE, COLOR_ACCENT_ORANGE, COLOR_BG_ELEVATED, COLOR_BORDER, COLOR_TEXT_MUTED,
    COLOR_TEXT_PRIMARY,
};
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{Sizable, Size, h_flex, v_flex};

pub struct DoctorModal;

static CACHED_DOCTOR_GPUS: std::sync::OnceLock<Vec<gpu::GpuInfo>> = std::sync::OnceLock::new();

impl DoctorModal {
    pub fn render(
        state_entity: &Entity<AppState>,
        _window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let state = state_entity.read(cx);
        let gpus = CACHED_DOCTOR_GPUS.get_or_init(|| gpu::detect_gpus().unwrap_or_default());
        let amd_gpu = gpus.iter().find(|g| g.vendor_id == gpu::VENDOR_AMD);
        let models_dir = state.paths.models_dir();
        let life_dir = state.paths.life_dir();
        let config_file = state.paths.config_file();
        let logs_dir = state.paths.logs_dir();

        v_flex()
            .w_full()
            .gap_3()
            .child(
                v_flex()
                    .p_3p5()
                    .rounded_lg()
                    .bg(COLOR_BG_ELEVATED)
                    .border_1()
                    .border_color(COLOR_BORDER)
                    .gap_2()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(icons::cpu().with_size(Size::Small).text_color(COLOR_ACCENT_ORANGE))
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(COLOR_TEXT_PRIMARY)
                                    .child("Графический ускоритель"),
                            ),
                    )
                    .child(Self::info_row(
                        "Видеокарта",
                        amd_gpu
                            .map(|g| g.name.clone())
                            .unwrap_or_else(|| "Совместимый AMD GPU не обнаружен".to_string()),
                    ))
                    .child(Self::info_row(
                        "Выделенная VRAM",
                        amd_gpu
                            .map(|g| format!("{} МБ", g.dedicated_vram_mib()))
                            .unwrap_or_else(|| "Нет данных".to_string()),
                    ))
                    .child(Self::info_row(
                        "Архитектура GFX",
                        amd_gpu
                            .and_then(|g| g.gfx_target.clone())
                            .unwrap_or_else(|| "Не определена".to_string()),
                    )),
            )
            .child(
                v_flex()
                    .p_3p5()
                    .rounded_lg()
                    .bg(COLOR_BG_ELEVATED)
                    .border_1()
                    .border_color(COLOR_BORDER)
                    .gap_2p5()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(icons::folder().with_size(Size::Small).text_color(COLOR_ACCENT_BLUE))
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(COLOR_TEXT_PRIMARY)
                                    .child("Рабочие папки и файлы"),
                            ),
                    )
                    .child(Self::path_row("doc-open-models", "Модели", models_dir, false))
                    .child(Self::path_row("doc-open-life", "Память и сессии", life_dir, false))
                    .child(Self::path_row("doc-open-cfg", "Конфигурация", config_file, true))
                    .child(Self::path_row("doc-open-logs", "Журналы и логи", logs_dir, false)),
            )
    }

    fn info_row(label: &'static str, value: String) -> impl IntoElement {
        h_flex()
            .justify_between()
            .items_center()
            .text_size(rems(0.84))
            .child(div().text_color(COLOR_TEXT_MUTED).child(label))
            .child(div().font_weight(FontWeight::MEDIUM).text_color(COLOR_TEXT_PRIMARY).child(value))
    }

    fn path_row(
        id: &'static str,
        title: &'static str,
        path: std::path::PathBuf,
        is_file: bool,
    ) -> impl IntoElement {
        let display = path.to_string_lossy().to_string();
        h_flex()
            .justify_between()
            .items_center()
            .gap_3()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        div()
                            .text_size(rems(0.84))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(COLOR_TEXT_PRIMARY)
                            .child(title),
                    )
                    .child(
                        div()
                            .w_full()
                            .truncate()
                            .text_size(rems(0.75))
                            .text_color(COLOR_TEXT_MUTED)
                            .child(display),
                    ),
            )
            .child(
                Button::new(id)
                    .secondary()
                    .outline()
                    .with_size(Size::XSmall)
                    .icon(if is_file {
                        icons::folder_open()
                    } else {
                        icons::folder()
                    })
                    .label(if is_file { "Показать" } else { "Открыть" })
                    .on_click(move |_, _, _cx| {
                        crate::vision::open_in_explorer(&path);
                    }),
            )
    }
}
