//! Diálogo "Importar noteskins": busca simuladores instalados (XSanity, StepF2, StepMania…)
//! y copia sus noteskins pump a la carpeta de datos de la app. Step Charter sólo trae el
//! noteskin pump de ArrowVortex; los demás los aporta el simulador de cada usuario.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

use egui::Color32;

use super::App;
use crate::noteskin;

enum Msg {
    /// Simuladores encontrados: (carpeta del simulador, carpeta de noteskins pump, cantidad).
    Found(Vec<(PathBuf, PathBuf, usize)>),
    /// Fin de una copia: mensaje para el usuario.
    Done(Result<String, String>),
}

pub struct ImportSkins {
    /// `None` mientras se busca.
    found: Option<Vec<(PathBuf, PathBuf, usize)>>,
    /// Noteskins que ya hay en la carpeta de importados.
    imported: usize,
    copying: bool,
    status: Option<Result<String, String>>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

fn fmt_size(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb >= 1.0 {
        format!("{:.0} MB", mb)
    } else {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    }
}

fn imported_count() -> usize {
    noteskin::imported_skins_dir().map(|d| noteskin::list_skins(&d).len()).unwrap_or(0)
}

impl App {
    /// Abre el diálogo y busca simuladores en segundo plano (recorre varias unidades).
    pub(super) fn open_import_skins(&mut self) {
        let (tx, rx) = channel();
        let mut songs: Vec<PathBuf> = self.song.as_ref().and_then(|s| s.path.clone()).into_iter().collect();
        songs.extend(self.recent.iter().cloned());
        let (t, ctx) = (tx.clone(), self.egui_ctx.clone());
        std::thread::spawn(move || {
            let _ = t.send(Msg::Found(noteskin::find_simulators(&songs)));
            ctx.request_repaint();
        });
        self.import = Some(ImportSkins { found: None, imported: imported_count(), copying: false, status: None, tx, rx });
    }

    /// Copia en segundo plano los noteskins de la carpeta elegida a la de importados.
    fn start_import(&mut self, picked: PathBuf) {
        let ctx = self.egui_ctx.clone();
        let Some(imp) = self.import.as_mut() else { return };
        let Some(dest) = noteskin::imported_skins_dir() else {
            imp.status = Some(Err("No se encontró la carpeta de datos de la app.".to_string()));
            return;
        };
        let Some((base, names)) = noteskin::import_plan(&picked) else {
            imp.status = Some(Err(format!(
                "No hay noteskins pump en {}. Elige la carpeta del simulador (la que contiene NoteSkins y Songs) o la de un noteskin.",
                picked.display()
            )));
            return;
        };
        imp.copying = true;
        imp.status = None;
        let t = imp.tx.clone();
        std::thread::spawn(move || {
            let r = noteskin::import_skins(&base, &names, &dest)
                .map(|r| format!("Se importaron {} noteskins ({}).", r.skins, fmt_size(r.bytes)));
            let _ = t.send(Msg::Done(r));
            ctx.request_repaint();
        });
    }

    pub(super) fn import_skins_window(&mut self, ctx: &egui::Context) {
        let Some(imp) = self.import.as_mut() else { return };
        let mut imported_now = None;
        while let Ok(m) = imp.rx.try_recv() {
            match m {
                Msg::Found(v) => imp.found = Some(v),
                Msg::Done(r) => {
                    imp.copying = false;
                    if let Ok(s) = &r {
                        imported_now = Some(s.clone());
                        imp.imported = imported_count();
                    }
                    imp.status = Some(r);
                }
            }
        }

        let dest = noteskin::imported_skins_dir();
        let mut open = true;
        let mut pick: Option<PathBuf> = None;
        egui::Window::new("Importar noteskins")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(560.0)
            .show(ctx, |ui| {
                ui.label(
                    "Step Charter trae sólo el noteskin pump de ArrowVortex. Los noteskins de tu simulador \
                     (XSanity, StepF2, StepMania…) se copian una vez a la carpeta de la app:",
                );
                if let Some(d) = &dest {
                    ui.weak(format!("{}  ({} noteskins)", d.display(), imp.imported));
                }
                ui.separator();
                match &imp.found {
                    None => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Buscando simuladores...");
                        });
                    }
                    Some(v) if v.is_empty() => {
                        ui.label("No se encontró ningún simulador. Elige su carpeta a mano.");
                    }
                    Some(v) => {
                        ui.label("Simuladores encontrados:");
                        egui::Grid::new("sims").num_columns(3).striped(true).spacing([12.0, 4.0]).show(ui, |ui| {
                            for (root, skins, n) in v {
                                ui.label(root.display().to_string());
                                ui.label(format!("{} noteskins", n));
                                if ui.add_enabled(!imp.copying, egui::Button::new("Importar")).clicked() {
                                    pick = Some(skins.clone());
                                }
                                ui.end_row();
                            }
                        });
                    }
                }
                ui.add_space(6.0);
                if ui.add_enabled(!imp.copying, egui::Button::new("Elegir la carpeta del simulador o de un noteskin...")).clicked() {
                    if let Some(d) = rfd::FileDialog::new().set_title("Carpeta del simulador").pick_folder() {
                        pick = Some(d);
                    }
                }
                if imp.copying {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Copiando noteskins...");
                    });
                }
                match &imp.status {
                    Some(Ok(s)) => {
                        ui.colored_label(Color32::from_rgb(120, 220, 120), s);
                    }
                    Some(Err(e)) => {
                        ui.colored_label(Color32::from_rgb(240, 110, 110), e);
                    }
                    None => {}
                }
                ui.add_space(4.0);
                ui.weak("Si un noteskin ya estaba importado, se reemplaza por la versión del simulador.");
            });

        if let Some(s) = imported_now {
            let path = self.song.as_ref().and_then(|s| s.path.clone());
            let dirs = self.compute_skin_dirs(path.as_deref());
            self.set_skin_dirs(dirs);
            self.msg(s);
        }
        if let Some(p) = pick {
            self.start_import(p);
        }
        // Mientras copia no se cierra (el resultado se mostraría en ningún lado).
        if !open && !self.import.as_ref().map(|i| i.copying).unwrap_or(false) {
            self.import = None;
        }
    }
}
