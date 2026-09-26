// Memoria GPUI — note data is owned by Kosmos Engine.
#![windows_subsystem = "windows"]
mod app;
mod pages;
mod theme;

use gpui::{px, size, App, AppContext, Bounds, WindowBounds, WindowOptions};

/// `MEMORIA_OFFSCREEN=1` parks the window far outside the desktop so automated
/// runs (gates, benchmarks, screenshots) don't pop a window on the screen.
fn window_bounds(cx: &mut App) -> Bounds<gpui::Pixels> {
    if std::env::var("MEMORIA_OFFSCREEN").is_ok() {
        gpui::bounds(
            gpui::point(px(-20000.), px(-20000.)),
            size(px(1440.), px(900.)),
        )
    } else {
        Bounds::centered(None, size(px(1440.), px(900.)), cx)
    }
}

fn main() {
    gpui::application().run(|cx: &mut App| {
        gpui_component::init(cx);
        imago_gpui::theme::apply(cx);

        let bounds = window_bounds(cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some(gpui::SharedString::from("Memoria")),
                    appears_transparent: true,
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(app::Memoria::new);
                // gpui-component widgets (Input, menus) require a ui::Root window layer.
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            },
        )
        .unwrap();
        if std::env::var("MEMORIA_OFFSCREEN").is_err() {
            cx.activate(true);
        }
    });
}
