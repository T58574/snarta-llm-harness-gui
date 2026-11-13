use super::chat::ChatView;
use super::input::InputDock;
use super::modals::{ModelsModal, PersonasModal, PrepareModal, RuntimeModal, SettingsModal};
use super::sidebar::Sidebar;
use super::topbar::TopBar;
use crate::gui::state::{ActiveModal, AppState, ServerStatus};
use crate::gui::theme::{COLOR_GRAPHITE_CANVAS, COLOR_GRAPHITE_SIDEBAR, COLOR_TEXT_PRIMARY};
use gpui::*;
use gpui_component::{h_flex, v_flex};
use std::time::Duration;

pub struct MainWindow {
    pub state: Entity<AppState>,
    pub input_dock: InputDock,
    pub prepare_modal: PrepareModal,
    pub runtime_modal: RuntimeModal,
    pub settings_modal: SettingsModal,
    pub prev_settings_open: bool,
    pub needs_settings_sync: bool,
    pub focus_handle: FocusHandle,
    pub chat_scroll_handle: ScrollHandle,
}

impl MainWindow {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_dock = InputDock::new(window, cx);
        let cfg = state.read(cx).cfg.clone();
        let prepare_modal = PrepareModal::new(&cfg);
        let runtime_modal = RuntimeModal::new(&state.read(cx).paths);
        let settings_modal = SettingsModal::new(&cfg, window, cx);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);

        let state_for_bounds = state.clone();
        cx.observe_window_bounds(window, move |_this, window, cx| {
            let wb = window.window_bounds();
            state_for_bounds.update(cx, |s, _cx| {
                match wb {
                    WindowBounds::Windowed(b) => {
                        let w = b.size.width / px(1.0);
                        let h = b.size.height / px(1.0);
                        let x = b.origin.x / px(1.0);
                        let y = b.origin.y / px(1.0);
                        if w >= 600.0 && h >= 400.0 {
                            s.cfg.ui.window_width = Some(w);
                            s.cfg.ui.window_height = Some(h);
                        }
                        if x > -3000.0 && y > -3000.0 {
                            s.cfg.ui.window_x = Some(x);
                            s.cfg.ui.window_y = Some(y);
                        }
                        s.cfg.ui.window_maximized = false;
                        let _ = s.cfg.save(&s.paths.config_file());
                    }
                    WindowBounds::Maximized(b) | WindowBounds::Fullscreen(b) => {
                        let w = b.size.width / px(1.0);
                        let h = b.size.height / px(1.0);
                        if w >= 600.0 && h >= 400.0 {
                            s.cfg.ui.window_width = Some(w);
                            s.cfg.ui.window_height = Some(h);
                        }
                        s.cfg.ui.window_maximized = true;
                        let _ = s.cfg.save(&s.paths.config_file());
                    }
                }
            });
        })
        .detach();

        let chat_scroll_handle = ScrollHandle::new();
        chat_scroll_handle.scroll_to_bottom();
        window.on_next_frame({
            let sh = chat_scroll_handle.clone();
            move |_, _| {
                sh.scroll_to_bottom();
            }
        });

        // Background channel & worker for exact debounced tokenization
        let (tokenize_tx, tokenize_worker_rx) = std::sync::mpsc::channel::<(String, u16, String)>();
        let (tokenize_result_tx, tokenize_result_rx) = std::sync::mpsc::channel::<(String, usize)>();
        std::thread::spawn(move || {
            while let Ok((host, port, text)) = tokenize_worker_rx.recv() {
                let mut latest_text = text;
                while let Ok((_, _, newer)) = tokenize_worker_rx.try_recv() {
                    latest_text = newer;
                }
                if let Ok(count) = crate::chat::client::count_tokens_exact(&host, port, &latest_text) {
                    let _ = tokenize_result_tx.send((latest_text, count));
                }
            }
        });

        // Background channel & worker for non-blocking VRAM / spill telemetry
        let (vram_req_tx, vram_req_rx) = std::sync::mpsc::channel::<(Option<u32>, u64, u64)>();
        let (vram_res_tx, vram_res_rx) = std::sync::mpsc::channel::<(u64, u64, u64)>();
        std::thread::spawn(move || {
            while let Ok((mut srv_pid, mut spill_thresh, mut reserve_mib)) = vram_req_rx.recv() {
                while let Ok((p, t, r)) = vram_req_rx.try_recv() {
                    srv_pid = p;
                    spill_thresh = t;
                    reserve_mib = r;
                }
                let snap =
                    crate::gui::state::measure_vram_snapshot(srv_pid, spill_thresh, reserve_mib);
                if vram_res_tx.send(snap).is_err() {
                    break;
                }
            }
        });

        // Spawn background poller for stream events, token count and VRAM metrics
        let state_clone = state.clone();
        cx.spawn(async move |this, cx| {
            let mut last_vram_poll = std::time::Instant::now();
            let mut last_config_poll = std::time::Instant::now();
            let mut last_tokenize_poll = std::time::Instant::now();
            let mut last_tokenized_text = String::new();
            let mut was_server_running = false;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(20))
                    .await;

                let should_notify = this.update(cx, |this_view, cx| {
                    let mut updated = false;

                    while let Ok((tok_text, count)) = tokenize_result_rx.try_recv() {
                        let cur = this_view.input_dock.textarea.read(cx).value().to_string();
                        if tok_text == cur {
                            state_clone.update(cx, |s, _| {
                                s.draft_tokens = Some(count);
                            });
                            updated = true;
                        }
                    }

                    if last_tokenize_poll.elapsed() >= Duration::from_millis(150) {
                        let cur_input = this_view.input_dock.textarea.read(cx).value().to_string();
                        if cur_input != last_tokenized_text {
                            last_tokenize_poll = std::time::Instant::now();
                            last_tokenized_text = cur_input.clone();
                            if cur_input.trim().is_empty() {
                                state_clone.update(cx, |s, _| {
                                    s.draft_tokens = Some(0);
                                });
                                updated = true;
                            } else {
                                let st = state_clone.read(cx);
                                if let ServerStatus::Running { port, .. } = st.server_status {
                                    let host = st.cfg.server.host.clone();
                                    let _ = tokenize_tx.send((host, port, cur_input));
                                } else {
                                    state_clone.update(cx, |s, _| {
                                        s.draft_tokens = None;
                                    });
                                    updated = true;
                                }
                            }
                        }
                    }

                    let is_running_now = matches!(
                        state_clone.read(cx).server_status,
                        ServerStatus::Running { .. }
                    );
                    if is_running_now && !was_server_running {
                        last_tokenized_text.clear();
                    }
                    was_server_running = is_running_now;

                    while let Ok((total, used, spill)) = vram_res_rx.try_recv() {
                        state_clone.update(cx, |s, _| {
                            if s.vram.total_mib != total
                                || s.vram.used_mib != used
                                || s.vram.spill_mib != spill
                            {
                                s.vram.total_mib = total;
                                s.vram.used_mib = used;
                                s.vram.spill_mib = spill;
                                updated = true;
                            }
                        });
                    }

                    if last_vram_poll.elapsed() >= Duration::from_millis(500) {
                        last_vram_poll = std::time::Instant::now();
                        let st = state_clone.read(cx);
                        let srv_pid = match st.server_status {
                            ServerStatus::Running { pid, .. } => Some(pid),
                            _ => None,
                        };
                        let _ = vram_req_tx.send((
                            srv_pid,
                            st.cfg.vram.shared_spill_threshold_mib,
                            st.cfg.vram.reserve_mib,
                        ));
                    }

                    if last_config_poll.elapsed() >= Duration::from_millis(1000) {
                        last_config_poll = std::time::Instant::now();
                        let (reloaded, settings_is_open) =
                            state_clone.update(cx, |state, _cx| {
                                (state.reload_config_if_changed(), state.settings_open)
                            });
                        if reloaded {
                            if settings_is_open {
                                this_view.needs_settings_sync = true;
                            }
                            updated = true;
                        }
                    }

                    let is_near_bottom = {
                        let offset = this_view.chat_scroll_handle.offset();
                        let max_offset = this_view.chat_scroll_handle.max_offset();
                        (max_offset.y + offset.y) <= px(64.0)
                    };

                    state_clone.update(cx, |state, _cx| {
                        if this_view.input_dock.poll_file_picker(state) {
                            updated = true;
                        }

                        let prev_status = state.server_status.clone();
                        state.refresh_server_status();
                        if state.server_status != prev_status {
                            updated = true;
                        }

                        if state.poll_agent_events(is_near_bottom) {
                            updated = true;
                        }
                    });

                    let should_scroll = state_clone.update(cx, |s, _| {
                        if s.needs_scroll_to_bottom {
                            s.needs_scroll_to_bottom = false;
                            true
                        } else {
                            false
                        }
                    });
                    if should_scroll {
                        this_view.chat_scroll_handle.scroll_to_bottom();
                    }

                    if updated {
                        cx.notify();
                    }
                    updated
                });

                if should_notify.is_err() {
                    break;
                }
            }
        })
        .detach();

        Self {
            state,
            input_dock,
            prepare_modal,
            runtime_modal,
            settings_modal,
            prev_settings_open: false,
            needs_settings_sync: false,
            focus_handle,
            chat_scroll_handle,
        }
    }
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_modal = self.state.read(cx).active_modal;
        let sidebar_open = self.state.read(cx).sidebar_open;
        let settings_open = self.state.read(cx).settings_open;

        if settings_open && (!self.prev_settings_open || self.needs_settings_sync) {
            let (cfg_snap, models_snap) = self.state.update(cx, |s, _| {
                if !self.prev_settings_open {
                    s.reload_config_if_changed();
                    s.rescan_models();
                }
                (s.cfg.clone(), s.available_models.clone())
            });
            self.settings_modal
                .sync_from_state(&cfg_snap, &models_snap, window, cx);
            self.needs_settings_sync = false;
        }
        self.prev_settings_open = settings_open;

        let focus_handle = self.focus_handle.clone();
        let state_for_keys = self.state.clone();

        v_flex()
            .track_focus(&focus_handle)
            .size_full()
            .bg(COLOR_GRAPHITE_CANVAS)
            .text_color(COLOR_TEXT_PRIMARY)
            .capture_key_down(move |event, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key.eq_ignore_ascii_case("escape") || key.eq_ignore_ascii_case("esc") {
                    let has_modal = state_for_keys.read(cx).active_modal.is_some();
                    let is_settings = state_for_keys.read(cx).settings_open;
                    if has_modal {
                        cx.stop_propagation();
                        state_for_keys.update(cx, |s, cx| {
                            s.active_modal = None;
                            cx.notify();
                        });
                    } else if is_settings {
                        cx.stop_propagation();
                        state_for_keys.update(cx, |s, cx| {
                            s.settings_open = false;
                            cx.notify();
                        });
                    }
                }
            })
            .child(TopBar::render(&self.state, window, cx))
            .child(
                h_flex()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .children(if sidebar_open {
                        Some(Sidebar::render(&self.state, window, cx))
                    } else {
                        None
                    })
                    .child(if settings_open {
                        v_flex()
                            .flex_1()
                            .h_full()
                            .overflow_hidden()
                            .child(self.settings_modal.render(
                                &self.state,
                                &mut self.runtime_modal,
                                window,
                                cx,
                            ))
                    } else {
                        v_flex()
                            .flex_1()
                            .h_full()
                            .overflow_hidden()
                            .child(ChatView::render(
                                &self.state,
                                &self.chat_scroll_handle,
                                window,
                                cx,
                            ))
                            .child(self.input_dock.render(&self.state, window, cx))
                    }),
            )
            .children(active_modal.map(|modal| {
                div()
                    .id("modal-backdrop")
                    .absolute()
                    .inset_0()
                    .bg(COLOR_GRAPHITE_SIDEBAR.opacity(0.82))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_default()
                    .capture_key_down({
                        let state_entity = self.state.clone();
                        move |event, _window, cx| {
                            let key = event.keystroke.key.as_str();
                            if key.eq_ignore_ascii_case("escape") || key.eq_ignore_ascii_case("esc")
                            {
                                cx.stop_propagation();
                                state_entity.update(cx, |s, cx| {
                                    s.active_modal = None;
                                    cx.notify();
                                });
                            }
                        }
                    })
                    .on_mouse_down(MouseButton::Left, {
                        let state_entity = self.state.clone();
                        move |_, _, cx| {
                            state_entity.update(cx, |s, cx| {
                                s.active_modal = None;
                                cx.notify();
                            });
                        }
                    })
                    .child(
                        div()
                            .id("modal-card-wrapper")
                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation();
                            })
                            .on_click(|_, _, cx| {
                                cx.stop_propagation();
                            })
                            .child(match modal {
                                ActiveModal::Prepare => self
                                    .prepare_modal
                                    .render(&self.state, window, cx)
                                    .into_any_element(),
                                ActiveModal::Models => {
                                    ModelsModal::render(&self.state, window, cx).into_any_element()
                                }
                                ActiveModal::Personas => {
                                    PersonasModal::render(&self.state, window, cx)
                                        .into_any_element()
                                }
                            }),
                    )
            }))
    }
}
