// Memoria GPUI — note data is owned by Mundus Engine.
#![windows_subsystem = "windows"]
mod a11y;
mod app;
mod theme;

#[cfg(test)]
mod ui_tests;

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

/// `memoria-gpui --export <dir>` (M8) — headless Obsidian export for
/// verification while the M4 export UI isn't on this branch. All file IO runs
/// through Engine `filesystem.vault.*` grants; the app writes nothing itself.
fn run_export(dir: &str) -> ! {
    let engine = memoria_gpui::store::Engine::default();
    let mut entries = memoria_gpui::store::EntryApi::new(engine.clone());
    let notes = memoria_gpui::store::NoteTypeApi::new(engine.clone());
    let result = (|| -> Result<u64, String> {
        let entries = entries.list_all_entries().map_err(|e| format!("{e:?}"))?;
        let note_types = notes.list_note_types().map_err(|e| format!("{e:?}"))?;
        let body_markdown_lookup = entries
            .iter()
            .map(|e| {
                let value =
                    serde_json::from_str(&e.content_json).unwrap_or(serde_json::Value::Null);
                (
                    e.id.clone(),
                    memoria_gpui::content::read_entry_markdown(&value),
                )
            })
            .collect();
        let title_lookup = entries
            .iter()
            .map(|e| (e.id.clone(), e.title.clone()))
            .collect();
        memoria_gpui::obsidian::export_obsidian_vault_dir(
            &engine,
            entries,
            &note_types,
            &body_markdown_lookup,
            &title_lookup,
            dir,
        )
    })();
    match result {
        Ok(count) => {
            println!("exported {count} file(s) into {dir}");
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("export failed: {error}");
            std::process::exit(1);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--export") {
        match args.get(pos + 1) {
            Some(dir) => run_export(dir),
            None => {
                eprintln!("usage: memoria-gpui --export <dir>");
                std::process::exit(2);
            }
        }
    }
    gpui::application().run(|cx: &mut App| {
        gpui_component::init(cx);
        imago_gpui::theme::apply(cx);
        cx.bind_keys(memoria_editor_gpui::key_bindings());

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
