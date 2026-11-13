use crate::gui::state::AppState;
use crate::runtime::install::{
    self, InstallProgress, InstalledBuild, download_and_install_with_reporter, list_installed,
    remove_old_builds,
};
use crate::runtime::releases::{ReleaseCandidate, fetch_candidates};
use gpui::InteractiveElement as _;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{ActiveTheme, IconName, Sizable, Size, h_flex, v_flex};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct RuntimeState {
    pub installed: Vec<InstalledBuild>,
    pub candidates: Vec<ReleaseCandidate>,
    pub is_fetching: bool,
    pub fetch_status: Option<String>,
    pub is_installing: bool,
    pub install_status: Option<String>,
    pub clean_status: Option<String>,
}

#[derive(Clone)]
pub struct RuntimeModal {
    pub inner: Arc<Mutex<RuntimeState>>,
}

impl RuntimeModal {
    pub fn new(paths: &crate::paths::AppPaths) -> Self {
        let installed = list_installed(&paths.runtime_dir()).unwrap_or_default();
        Self {
            inner: Arc::new(Mutex::new(RuntimeState {
                installed,
                candidates: Vec::new(),
                is_fetching: false,
                fetch_status: None,
                is_installing: false,
                install_status: None,
                clean_status: None,
            })),
        }
    }

    pub fn render(
        &mut self,
        state_entity: &Entity<AppState>,
        _window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let state = state_entity.read(cx);
        let active_build = state.cfg.runtime.active_build.clone();
        let last_good_build = state.cfg.runtime.last_good_build.clone();

        let (
            installed,
            candidates,
            is_fetching,
            fetch_status,
            is_installing,
            install_status,
            clean_status,
        ) = {
            let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            guard.installed = list_installed(&state.paths.runtime_dir()).unwrap_or_default();
            (
                guard.installed.clone(),
                guard.candidates.clone(),
                guard.is_fetching,
                guard.fetch_status.clone(),
                guard.is_installing,
                guard.install_status.clone(),
                guard.clean_status.clone(),
            )
        };

        v_flex()
            .w_full()
            .gap_4()
            // Installed builds section
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Установленные сборки:"),
                            )
                            .child(
                                Button::new("clean-old-builds-btn")
                                    .ghost()
                                    .with_size(Size::Small)
                                    .icon(IconName::Delete)
                                    .label("Очистить старые")
                                    .on_click({
                                        let inner = self.inner.clone();
                                        let state_entity = state_entity.clone();
                                        move |_, _, cx| {
                                            let (runtime_dir, active_build) = {
                                                let s = state_entity.read(cx);
                                                (
                                                    s.paths.runtime_dir(),
                                                    s.cfg.runtime.active_build.clone(),
                                                )
                                            };
                                            match remove_old_builds(
                                                &runtime_dir,
                                                active_build.as_deref(),
                                            ) {
                                                Ok((count, freed)) => {
                                                    let freed_str = install::format_bytes(freed);
                                                    let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                                                    guard.clean_status = Some(format!(
                                                        "Очищено {count} старых сборок (освобождено {freed_str})"
                                                    ));
                                                    guard.installed =
                                                        list_installed(&runtime_dir)
                                                            .unwrap_or_default();
                                                    state_entity.update(cx, |s, cx| {
                                                        if s.cfg.runtime.last_good_build.as_deref()
                                                            != s.cfg.runtime.active_build.as_deref()
                                                        {
                                                            s.cfg.runtime.last_good_build =
                                                                s.cfg.runtime.active_build.clone();
                                                            let _ = s.cfg.save(&s.paths.config_file());
                                                        }
                                                        cx.notify();
                                                    });
                                                }
                                                Err(e) => {
                                                    let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                                                    guard.clean_status =
                                                        Some(format!("Ошибка очистки: {e}"));
                                                    state_entity.update(cx, |_, cx| cx.notify());
                                                }
                                            }
                                        }
                                    }),
                            ),
                    )
                    .children(clean_status.map(|msg| {
                        div()
                            .text_size(rems(0.8))
                            .text_color(cx.theme().primary)
                            .child(msg)
                    }))
                    .child(
                        v_flex()
                            .id("installed-scroll")
                            .max_h(rems(9.0))
                            .overflow_y_scroll()
                            .gap_1()
                            .children(if installed.is_empty() {
                                vec![div()
                                    .p_2()
                                    .rounded_md()
                                    .bg(cx.theme().muted)
                                    .text_size(rems(0.85))
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Нет установленных сборок llama.cpp.")]
                            } else {
                                installed
                                    .iter()
                                    .map(|b| {
                                        let is_active =
                                            active_build.as_deref() == Some(&b.build_id);
                                        let is_last_good =
                                            last_good_build.as_deref() == Some(&b.build_id);
                                        let bid = b.build_id.clone();
                                        let backend_str = b.backend.as_str();
                                        let size_str = install::format_bytes(b.size_bytes);

                                        h_flex()
                                            .justify_between()
                                            .items_center()
                                            .p_2()
                                            .rounded_md()
                                            .bg(if is_active {
                                                cx.theme().accent
                                            } else {
                                                cx.theme().muted
                                            })
                                            .child(
                                                v_flex()
                                                    .gap_0p5()
                                                    .child(
                                                        div()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .child(b.build_id.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(rems(0.75))
                                                            .text_color(
                                                                cx.theme().muted_foreground,
                                                            )
                                                            .child(format!(
                                                                "{backend_str} • {size_str}"
                                                            )),
                                                    ),
                                            )
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .items_center()
                                                    .children(if is_last_good {
                                                        Some(
                                                            div()
                                                                .text_size(rems(0.75))
                                                                .text_color(
                                                                    cx.theme().muted_foreground,
                                                                )
                                                                .child("[Последняя рабочая]"),
                                                        )
                                                    } else {
                                                        None
                                                    })
                                                    .child(if is_active {
                                                        div()
                                                            .px_2()
                                                            .py_0p5()
                                                            .rounded_md()
                                                            .bg(cx.theme().primary.opacity(0.15))
                                                            .text_color(cx.theme().primary)
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_size(rems(0.75))
                                                            .child("Активная")
                                                            .into_any_element()
                                                    } else {
                                                        Button::new(format!("set-active-{}", bid))
                                                            .ghost()
                                                            .with_size(Size::Small)
                                                            .label("Сделать активным")
                                                            .on_click({
                                                                let state_entity =
                                                                    state_entity.clone();
                                                                let bid = bid.clone();
                                                                move |_, _, cx| {
                                                                    state_entity.update(
                                                                        cx,
                                                                        |s, cx| {
                                                                            s.cfg
                                                                                .runtime
                                                                                .active_build =
                                                                                Some(bid.clone());
                                                                            let _ = s.cfg.save(
                                                                                &s.paths
                                                                                    .config_file(),
                                                                            );
                                                                            cx.notify();
                                                                        },
                                                                    );
                                                                }
                                                            })
                                                            .into_any_element()
                                                    }),
                                            )
                                    })
                                    .collect()
                            }),
                    ),
            )
            // Remote releases section
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Доступные релизы (GitHub):"),
                            )
                            .child(
                                Button::new("fetch-updates-btn")
                                    .primary()
                                    .with_size(Size::Small)
                                    .icon(IconName::RotateCw)
                                    .label(if is_fetching {
                                        "Поиск..."
                                    } else {
                                        "Проверить обновления"
                                    })
                                    .on_click({
                                        let inner = self.inner.clone();
                                        let state_entity = state_entity.clone();
                                        move |_, _, cx| {
                                            {
                                                let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                                                if guard.is_fetching {
                                                    return;
                                                }
                                                guard.is_fetching = true;
                                                guard.fetch_status =
                                                    Some("Запрос релизов с GitHub...".to_string());
                                            }
                                            let (rocm_repo, vulkan_repo) = {
                                                let s = state_entity.read(cx);
                                                (
                                                    s.cfg.runtime.rocm_repo.clone(),
                                                    s.cfg.runtime.vulkan_repo.clone(),
                                                )
                                            };
                                            let inner_bg = inner.clone();
                                            std::thread::spawn(move || {
                                                let gfx = crate::runtime::detect_preferred_gfx();
                                                let res = fetch_candidates(
                                                    &rocm_repo,
                                                    &vulkan_repo,
                                                    gfx.as_deref(),
                                                );
                                                let mut guard = inner_bg.lock().unwrap_or_else(|e| e.into_inner());
                                                guard.is_fetching = false;
                                                match res {
                                                    Ok(candidates) => {
                                                        guard.fetch_status = Some(format!(
                                                            "Найдено {} релизов",
                                                            candidates.len()
                                                        ));
                                                        guard.candidates = candidates;
                                                    }
                                                    Err(e) => {
                                                        guard.fetch_status = Some(format!(
                                                            "Ошибка поиска обновлений: {e}"
                                                        ));
                                                    }
                                                }
                                            });
                                            state_entity.update(cx, |_, cx| cx.notify());
                                        }
                                    }),
                            ),
                    )
                    .children(fetch_status.map(|msg| {
                        div()
                            .text_size(rems(0.8))
                            .text_color(cx.theme().muted_foreground)
                            .child(msg)
                    }))
                    .children(install_status.map(|msg| {
                        div()
                            .text_size(rems(0.8))
                            .text_color(cx.theme().primary)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(msg)
                    }))
                    .child(
                        v_flex()
                            .id("candidates-scroll")
                            .max_h(rems(10.0))
                            .overflow_y_scroll()
                            .gap_1()
                            .children(if candidates.is_empty() {
                                vec![div()
                                    .p_2()
                                    .rounded_md()
                                    .bg(cx.theme().muted)
                                    .text_size(rems(0.85))
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Нажмите «Проверить обновления», чтобы найти свежие сборки ROCm HIP и Vulkan.")]
                            } else {
                                candidates
                                    .into_iter()
                                    .map(|cand| {
                                        let already_installed =
                                            installed.iter().any(|b| b.build_id == cand.build_id);
                                        let backend_str = cand.backend.as_str();
                                        let size_str = install::format_bytes(cand.size_bytes);
                                        let repo_str = cand.repo.clone();
                                        let tag_str = cand.tag.clone();

                                        h_flex()
                                            .justify_between()
                                            .items_center()
                                            .p_2()
                                            .rounded_md()
                                            .bg(cx.theme().muted)
                                            .child(
                                                v_flex()
                                                    .gap_0p5()
                                                    .child(
                                                        div()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .child(cand.build_id.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(rems(0.75))
                                                            .text_color(
                                                                cx.theme().muted_foreground,
                                                            )
                                                            .child(format!(
                                                                "{backend_str} • {size_str} ({repo_str} @ {tag_str})"
                                                            )),
                                                    ),
                                            )
                                            .child(if already_installed {
                                                div()
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_md()
                                                    .bg(cx.theme().muted)
                                                    .text_color(cx.theme().muted_foreground)
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_size(rems(0.75))
                                                    .child("Установлена")
                                                    .into_any_element()
                                            } else {
                                                Button::new(format!("install-{}", cand.build_id))
                                                    .outline()
                                                    .with_size(Size::Small)
                                                    .label(if is_installing {
                                                        "Установка..."
                                                    } else {
                                                        "Установить"
                                                    })
                                                    .on_click({
                                                        let inner = self.inner.clone();
                                                        let state_entity = state_entity.clone();
                                                        let cand = cand.clone();
                                                        move |_, _, cx| {
                                                            {
                                                                let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                                                                if guard.is_installing {
                                                                    return;
                                                                }
                                                                guard.is_installing = true;
                                                                guard.install_status = Some(format!(
                                                                    "Подготовка установки {}...",
                                                                    cand.build_id
                                                                ));
                                                            }
                                                            let runtime_dir =
                                                                state_entity.read(cx).paths.runtime_dir();
                                                            let inner_bg = inner.clone();
                                                            let cand_clone = cand.clone();
                                                            std::thread::spawn(move || {
                                                                let inner_prog = inner_bg.clone();
                                                                let res = download_and_install_with_reporter(
                                                                    &runtime_dir,
                                                                    &cand_clone,
                                                                    move |p| {
                                                                        if let Ok(mut g) = inner_prog.lock() {
                                                                            match p {
                                                                                InstallProgress::Downloading {
                                                                                    downloaded,
                                                                                    total,
                                                                                    pct,
                                                                                } => {
                                                                                    let dl_mb = downloaded as f64 / (1024.0 * 1024.0);
                                                                                    let tot_mb = total as f64 / (1024.0 * 1024.0);
                                                                                    g.install_status = Some(format!(
                                                                                        "Загрузка {pct}% ({dl_mb:.1}/{tot_mb:.1} MB)..."
                                                                                    ));
                                                                                }
                                                                                InstallProgress::Extracting => {
                                                                                    g.install_status = Some(
                                                                                        "Распаковка архива...".to_string(),
                                                                                    );
                                                                                }
                                                                                InstallProgress::Verifying => {
                                                                                    g.install_status = Some(
                                                                                        "Проверка DLL и бинарников...".to_string(),
                                                                                    );
                                                                                }
                                                                                InstallProgress::Done => {
                                                                                    g.install_status = Some(
                                                                                        "Установка завершена!".to_string(),
                                                                                    );
                                                                                }
                                                                            }
                                                                        }
                                                                    },
                                                                );

                                                                let mut guard = inner_bg.lock().unwrap_or_else(|e| e.into_inner());
                                                                guard.is_installing = false;
                                                                match res {
                                                                    Ok((installed_build, _report)) => {
                                                                        guard.install_status = Some(format!(
                                                                            "Сборка {} успешно установлена!",
                                                                            installed_build.build_id
                                                                        ));
                                                                        guard.installed =
                                                                            list_installed(&runtime_dir)
                                                                                .unwrap_or_default();
                                                                    }
                                                                    Err(e) => {
                                                                        guard.install_status = Some(format!(
                                                                            "Ошибка установки: {e}"
                                                                        ));
                                                                    }
                                                                }
                                                            });
                                                            state_entity.update(cx, |_, cx| cx.notify());
                                                        }
                                                    })
                                                    .into_any_element()
                                            })
                                    })
                                    .collect()
                            }),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::AppPaths;

    #[core::prelude::v1::test]
    fn runtime_modal_initializes_empty_cleanly() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("snarta_runtime_modal_test_{nanos}"));
        let _ = std::fs::create_dir_all(&dir);

        let paths = AppPaths::resolve_from(&dir, None);
        paths.ensure_dirs().unwrap();

        let modal = RuntimeModal::new(&paths);
        let guard = modal.inner.lock().unwrap_or_else(|e| e.into_inner());
        assert!(guard.installed.is_empty());
        assert!(guard.candidates.is_empty());
        assert!(!guard.is_fetching);
        assert!(!guard.is_installing);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
