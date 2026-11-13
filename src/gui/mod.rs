pub mod anim;
pub mod commands;
pub mod state;
pub mod theme;
pub mod views;

use crate::config::Config;
use crate::log::Logger;
use crate::paths::AppPaths;
use gpui::*;
use state::AppState;
use std::io;
use views::MainWindow;

pub fn run_gui(paths: AppPaths, cfg: Config, logger: Logger) -> io::Result<()> {
    let _ = logger.info("starting snarta gpui desktop interface");

    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            theme::apply_graphite_pink_theme(cx);

            let default_w = cfg.ui.window_width.unwrap_or(1280.0).clamp(840.0, 7680.0);
            let default_h = cfg.ui.window_height.unwrap_or(720.0).clamp(560.0, 4320.0);
            let win_size = size(px(default_w), px(default_h));
            let base_bounds = match (cfg.ui.window_x, cfg.ui.window_y) {
                (Some(x), Some(y)) if x > -3000.0 && y > -3000.0 => Bounds {
                    origin: point(px(x), px(y)),
                    size: win_size,
                },
                _ => Bounds::centered(None, win_size, cx),
            };
            let window_bounds = if cfg.ui.window_maximized {
                WindowBounds::Maximized(base_bounds)
            } else {
                WindowBounds::Windowed(base_bounds)
            };

            let window_options = WindowOptions {
                window_bounds: Some(window_bounds),
                window_min_size: Some(size(px(840.0), px(560.0))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Snarta".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };

            let paths_clone = paths.clone();
            let cfg_clone = cfg.clone();
            let logger_clone = logger.clone();

            let _ = gpui_kit::open_window(window_options, cx, |window, cx| {
                let state = cx.new(|_| AppState::new(paths_clone, cfg_clone, logger_clone));
                cx.new(|cx| MainWindow::new(state, window, cx))
            });
        });

    Ok(())
}
