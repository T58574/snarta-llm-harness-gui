use crate::gpu;
use crate::gui::state::AppState;
use crate::gui::theme::{
    COLOR_GRAPHITE_BORDER, COLOR_GRAPHITE_ELEVATED, COLOR_PINK_PRIMARY,
};
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{IconName, Sizable, Size, h_flex, v_flex};

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
                    .p_3()
                    .rounded_lg()
                    .bg(COLOR_GRAPHITE_ELEVATED)
                    .border_1()
                    .border_color(COLOR_GRAPHITE_BORDER)
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(COLOR_PINK_PRIMARY)
                            .child("Графический ускоритель"),
                    )
                    .child(h_flex().justify_between().child("Видеокарта:").child(
                        if let Some(g) = amd_gpu {
                            g.name.clone()
                        } else {
                            "Не обнаружен совместимый AMD GPU".to_string()
                        },
                    ))
                    .child(h_flex().justify_between().child("Выделенная видеопамять:").child(
                        if let Some(g) = amd_gpu {
                            format!("{} МБ", g.dedicated_vram_mib())
                        } else {
                            "Н/Д".to_string()
                        },
                    ))
                    .child(h_flex().justify_between().child("Архитектура GFX:").child(
                        if let Some(g) = amd_gpu {
                            if let Some(ref arch) = g.gfx_target {
                                arch.clone()
                            } else {
                                "Неизвестно".to_string()
                            }
                        } else {
                            "Н/Д".to_string()
                        },
                    )),
            )
            .child(
                v_flex()
                    .p_3()
                    .rounded_lg()
                    .bg(COLOR_GRAPHITE_ELEVATED)
                    .border_1()
                    .border_color(COLOR_GRAPHITE_BORDER)
                    .gap_2p5()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(COLOR_PINK_PRIMARY)
                            .child("Рабочее окружение и пути"),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .gap_2()
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Директория моделей"))
                                    .child(
                                        div()
                                            .text_size(rems(0.75))
                                            .child(models_dir.to_string_lossy().to_string()),
                                    ),
                            )
                            .child(
                                Button::new("doc-open-models")
                                    .secondary()
                                    .with_size(Size::XSmall)
                                    .icon(IconName::Folder)
                                    .label("Открыть")
                                    .on_click(move |_, _, _cx| {
                                        crate::vision::open_in_explorer(&models_dir);
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .gap_2()
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Память и сессии"))
                                    .child(
                                        div()
                                            .text_size(rems(0.75))
                                            .child(life_dir.to_string_lossy().to_string()),
                                    ),
                            )
                            .child(
                                Button::new("doc-open-life")
                                    .secondary()
                                    .with_size(Size::XSmall)
                                    .icon(IconName::Folder)
                                    .label("Открыть")
                                    .on_click(move |_, _, _cx| {
                                        crate::vision::open_in_explorer(&life_dir);
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .gap_2()
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Конфигурация"))
                                    .child(
                                        div()
                                            .text_size(rems(0.75))
                                            .child(config_file.to_string_lossy().to_string()),
                                    ),
                            )
                            .child(
                                Button::new("doc-open-cfg")
                                    .secondary()
                                    .with_size(Size::XSmall)
                                    .icon(IconName::FolderOpen)
                                    .label("Показать")
                                    .on_click(move |_, _, _cx| {
                                        crate::vision::open_in_explorer(&config_file);
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .gap_2()
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Журналы и логи"))
                                    .child(
                                        div()
                                            .text_size(rems(0.75))
                                            .child(logs_dir.to_string_lossy().to_string()),
                                    ),
                            )
                            .child(
                                Button::new("doc-open-logs")
                                    .secondary()
                                    .with_size(Size::XSmall)
                                    .icon(IconName::Folder)
                                    .label("Открыть")
                                    .on_click(move |_, _, _cx| {
                                        crate::vision::open_in_explorer(&logs_dir);
                                    }),
                            ),
                    ),
            )
    }
}
