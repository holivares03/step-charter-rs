//! En Windows incrusta el ícono y los datos de versión en el `.exe`, para que el Explorador,
//! la barra de tareas y el Administrador de tareas muestren "Step Charter" con su ícono.
//! (El ícono de la ventana abierta lo pone `main.rs` con `assets/icon.png`.)
//!
//! `assets/icon.ico` se genera desde `assets/icon.png` con todos los tamaños de Windows:
//! `python -c "from PIL import Image; Image.open('assets/icon.png').save('assets/icon.ico',
//! sizes=[(s, s) for s in (16, 20, 24, 32, 40, 48, 64, 128, 256)])"`

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/icon.ico");
    // Lo que muestra el Administrador de tareas como nombre del proceso.
    res.set("FileDescription", "Step Charter");
    res.set("ProductName", "Step Charter");
    res.set("LegalCopyright", "GPL-3.0-or-later");
    if let Err(e) = res.compile() {
        // Sin compilador de recursos (rc.exe / windres) el .exe sigue funcionando, sólo sin ícono.
        println!("cargo:warning=No se pudo incrustar el ícono en el .exe: {}", e);
    }
}
