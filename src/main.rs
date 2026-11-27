// En release no abrir consola en Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod check;
mod edit;
mod minimap;
mod notefield;
mod noteskin;
mod simfile;

/// Nombre de la app: título de la ventana y carpeta de datos
/// (`%APPDATA%\Step Charter\data` en Windows).
pub const APP_NAME: &str = "Step Charter";
/// Nombre anterior del proyecto: de su carpeta de datos se copian los ajustes la primera vez.
const OLD_APP_NAME: &str = "ArrowVortex-RS";

/// Copia los ajustes guardados con el nombre anterior si todavía no hay ajustes propios.
fn migrate_settings() {
    let (Some(new), Some(old)) = (eframe::storage_dir(APP_NAME), eframe::storage_dir(OLD_APP_NAME)) else {
        return;
    };
    let (new_file, old_file) = (new.join("app.ron"), old.join("app.ron"));
    if !new_file.exists() && old_file.is_file() && std::fs::create_dir_all(&new).is_ok() {
        let _ = std::fs::copy(&old_file, &new_file);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "--check" {
        std::process::exit(check::run(&args[2]));
    }
    if args.len() >= 3 && args[1] == "--debug-regen" {
        std::process::exit(check::debug_regen(&args[2]));
    }

    let start = app::StartOptions::from_args(&args);
    migrate_settings();

    let icon = image::load_from_memory(include_bytes!("../assets/icon.png")).ok().map(|img| {
        let img = img.to_rgba8();
        egui::IconData { width: img.width(), height: img.height(), rgba: img.into_raw() }
    });
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(APP_NAME)
        .with_inner_size([1280.0, 720.0])
        .with_maximized(true)
        .with_min_inner_size([640.0, 400.0])
        .with_drag_and_drop(true);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    if let Err(e) = eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, start)))),
    ) {
        eprintln!("Error al iniciar la ventana: {}", e);
    }
}
