//! Diálogos de canción y chart: propiedades de la canción, propiedades del chart,
//! nuevo chart, borrar chart y nuevo simfile a partir de un audio.

use std::path::Path;

use egui::{Color32, RichText};

use super::{style_color, text_input_active, App, UndoEntry};
use crate::edit;
use crate::simfile::style::{self, PlayerMode, Style};
use crate::simfile::{is_timing_key, Chart, Song};

/// Dificultades de StepMania (en el orden en que se muestran).
pub const DIFFICULTIES: [&str; 6] = ["Beginner", "Easy", "Medium", "Hard", "Challenge", "Edit"];

/// Datos del diálogo "Nuevo chart".
#[derive(Clone)]
pub struct NewChartForm {
    pub style_id: &'static str,
    pub difficulty: String,
    pub meter: i32,
    pub description: String,
    pub credit: String,
    pub players: u8,
    pub copy_notes: bool,
    pub copy_timing: bool,
}

/// Campos de texto de la canción: (tag, etiqueta).
const SONG_TEXT: &[(&str, &str)] = &[
    ("TITLE", "Título"),
    ("SUBTITLE", "Subtítulo"),
    ("ARTIST", "Artista"),
    ("TITLETRANSLIT", "Título (transliterado)"),
    ("SUBTITLETRANSLIT", "Subtítulo (transliterado)"),
    ("ARTISTTRANSLIT", "Artista (transliterado)"),
    ("GENRE", "Género"),
    ("CREDIT", "Crédito (autor del simfile)"),
];

const AUDIO_EXT: &[&str] = &["ogg", "mp3", "wav", "flac"];
const IMAGE_EXT: &[&str] = &["png", "jpg", "jpeg", "bmp", "gif"];

/// Campos de archivo de la canción: (tag, etiqueta, extensiones).
const SONG_FILES: &[(&str, &str, &[&str])] = &[
    ("MUSIC", "Música", AUDIO_EXT),
    ("BANNER", "Banner", IMAGE_EXT),
    ("BACKGROUND", "Fondo", IMAGE_EXT),
    ("CDTITLE", "CD title", IMAGE_EXT),
];

/// Tags que el diálogo ya muestra con su propio control (no van en "otros tags").
fn is_known_song_key(key: &str) -> bool {
    SONG_TEXT.iter().any(|(k, _)| *k == key)
        || SONG_FILES.iter().any(|(k, _, _)| *k == key)
        || matches!(key, "DISPLAYBPM" | "SAMPLESTART" | "SAMPLELENGTH" | "VERSION")
        || is_timing_key(key)
}

/// Archivos de la carpeta de la canción con alguna de esas extensiones.
fn folder_files(dir: Option<&Path>, exts: &[&str]) -> Vec<String> {
    let Some(dir) = dir else { return Vec::new() };
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                .filter(|n| {
                    let l = n.to_ascii_lowercase();
                    exts.iter().any(|x| l.ends_with(&format!(".{}", x)))
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|s| s.to_ascii_lowercase());
    v
}

/// Modo del Display BPM: (0 real, 1 fijo, 2 rango, 3 aleatorio, valor bajo, valor alto).
fn parse_display_bpm(params: &[String], lo: f64, hi: f64) -> (u8, f64, f64) {
    let p: Vec<&str> = params.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    match p.as_slice() {
        [] => (0, lo, hi),
        ["*"] => (3, lo, hi),
        [a] => {
            let v = a.parse().unwrap_or(lo);
            (1, v, v)
        }
        [a, b, ..] => (2, a.parse().unwrap_or(lo), b.parse().unwrap_or(hi)),
    }
}

fn fmt_num(v: f64) -> String {
    let s = format!("{:.3}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

impl App {
    /// Foto de la canción completa para deshacer (cambios de propiedades, charts nuevos o borrados).
    pub(super) fn song_snapshot(&self) -> Option<UndoEntry> {
        let s = self.song.as_ref()?;
        Some(UndoEntry::Song { header: s.header.clone(), timing: s.timing.clone(), charts: s.charts.clone(), chart_idx: self.chart_idx })
    }

    /// Antes de cambiar propiedades: guarda una foto para deshacer, una sola vez por edición
    /// (escribir un título entero cuenta como un solo paso).
    pub(super) fn meta_begin(&mut self) {
        if !self.meta_edit_active {
            if let Some(e) = self.song_snapshot() {
                self.push_entry(e);
            }
            self.meta_edit_active = true;
        }
        self.meta_changed = true;
    }

    /// Al final de cada cuadro: si ya no se está editando, el próximo cambio es otro paso de deshacer.
    pub(super) fn meta_end_frame(&mut self, ctx: &egui::Context) {
        if !self.meta_changed && !text_input_active(ctx) && !ctx.input(|i| i.pointer.any_down()) {
            self.meta_edit_active = false;
        }
        self.meta_changed = false;
    }

    // ------------------------------------------------------------------
    // Nuevo simfile desde audio
    // ------------------------------------------------------------------

    pub(super) fn new_song_from_audio_dialog(&mut self) {
        let mut dlg = rfd::FileDialog::new().add_filter("Audio", AUDIO_EXT);
        if let Some(dir) = self.song.as_ref().and_then(|s| s.dir()).or_else(|| self.recent.first().and_then(|p| p.parent().map(|d| d.to_path_buf()))) {
            dlg = dlg.set_directory(dir);
        }
        if let Some(audio) = dlg.pick_file() {
            self.new_song_from_audio(&audio);
        }
    }

    pub(super) fn new_song_from_audio(&mut self, audio: &Path) {
        let song = Song::new_for_audio(audio);
        if let Some(p) = song.path.clone().filter(|p| p.exists()) {
            self.msg(format!("Ya existe {}; se abre ese simfile.", p.file_name().and_then(|n| n.to_str()).unwrap_or("")));
            self.open_path(&p);
            return;
        }
        self.close_song();
        self.song = Some(song);
        self.chart_idx = 0;
        self.rebuild_timing();
        self.set_cursor_beat(0.0);
        self.load_media(true, true);
        self.dirty = true;
        self.msg("Simfile nuevo: agrega un chart y guarda con Ctrl+S.");
        self.open_new_chart();
    }

    // ------------------------------------------------------------------
    // Nuevo chart
    // ------------------------------------------------------------------

    pub(super) fn open_new_chart(&mut self) {
        if self.song.is_none() {
            self.msg("Abre o crea un simfile primero (Archivo → Nuevo desde audio).");
            return;
        }
        let cur = self.chart();
        let style_id = cur.map(|c| c.style).filter(|s| s.game == style::Game::Pump).map(|s| s.id).unwrap_or("pump-single");
        let own_timing = cur.map(|c| c.timing.is_some()).unwrap_or(false);
        let credit = cur.map(|c| c.credit()).unwrap_or_default();
        let st = style::find(style_id).copied().unwrap_or(style::STYLES[0]);
        self.new_chart = Some(NewChartForm {
            style_id,
            difficulty: "Edit".into(),
            meter: 1,
            description: String::new(),
            credit,
            players: st.default_players.max(1),
            copy_notes: false,
            copy_timing: own_timing,
        });
    }

    pub(super) fn new_chart_window(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.new_chart.clone() else { return };
        let mut open = true;
        let mut create = false;
        let mut cancel = false;
        let cur = self.song.as_ref().and_then(|s| s.charts.get(self.chart_idx));
        let cur_cols = cur.map(|c| c.num_cols);
        let cur_own_timing = cur.map(|c| c.timing.is_some()).unwrap_or(false);
        let cur_name = cur.map(|c| c.short_name()).unwrap_or_default();
        egui::Window::new("Nuevo chart").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            egui::Grid::new("new_chart").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label("Tipo");
                let st = style::find(form.style_id).copied().unwrap_or(style::STYLES[0]);
                egui::ComboBox::from_id_salt("nc_style").selected_text(format!("{} ({} col.)", st.name, st.num_cols())).show_ui(ui, |ui| {
                    for s in style::pump_styles() {
                        if ui.selectable_label(s.id == form.style_id, format!("{}  ({} col.)", s.name, s.num_cols())).clicked() {
                            form.style_id = s.id;
                            form.players = s.default_players.max(1);
                        }
                    }
                });
                ui.end_row();
                ui.label("Dificultad");
                egui::ComboBox::from_id_salt("nc_diff").selected_text(&form.difficulty).show_ui(ui, |ui| {
                    for d in DIFFICULTIES {
                        ui.selectable_value(&mut form.difficulty, d.to_string(), d);
                    }
                });
                ui.end_row();
                ui.label("Nivel");
                ui.add(egui::DragValue::new(&mut form.meter).range(0..=999));
                ui.end_row();
                ui.label("Descripción");
                ui.add(egui::TextEdit::singleline(&mut form.description).hint_text("p. ej. S17").desired_width(200.0));
                ui.end_row();
                ui.label("Autor (step artist)");
                ui.add(egui::TextEdit::singleline(&mut form.credit).desired_width(200.0));
                ui.end_row();
                if st.mode == PlayerMode::Shared {
                    ui.label("Jugadores");
                    ui.add(egui::DragValue::new(&mut form.players).range(1..=9));
                    ui.end_row();
                }
                let same_cols = cur_cols == Some(st.num_cols());
                ui.label("");
                ui.add_enabled(same_cols, egui::Checkbox::new(&mut form.copy_notes, if cur_name.is_empty() { "Copiar las notas del chart actual".to_string() } else { format!("Copiar las notas de {}", cur_name) }))
                    .on_disabled_hover_text("El chart actual tiene otro número de columnas");
                ui.end_row();
                ui.label("");
                ui.add_enabled(cur_own_timing, egui::Checkbox::new(&mut form.copy_timing, "Usar el timing propio del chart actual"))
                    .on_disabled_hover_text("El chart actual usa el timing de la canción");
                ui.end_row();
                if !same_cols {
                    form.copy_notes = false;
                }
                if !cur_own_timing {
                    form.copy_timing = false;
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Crear").strong()).clicked() {
                    create = true;
                }
                if ui.button("Cancelar").clicked() {
                    cancel = true;
                }
            });
        });
        if !open || cancel {
            self.new_chart = None;
            return;
        }
        if !create {
            self.new_chart = Some(form);
            return;
        }
        self.new_chart = None;
        let Some(st) = style::find(form.style_id).copied() else { return };
        let mut chart = Chart::new(st);
        chart.set("DIFFICULTY", &form.difficulty);
        chart.set("METER", &form.meter.to_string());
        chart.set("DESCRIPTION", form.description.trim());
        chart.set("CREDIT", form.credit.trim());
        if st.mode == PlayerMode::Shared && st.id.ends_with("-p") {
            chart.set_players(form.players);
        }
        if let Some(cur) = self.chart() {
            if form.copy_timing {
                chart.timing = cur.timing.clone();
            }
            if form.copy_notes && cur.num_cols == chart.num_cols {
                let mut notes = cur.notes.clone();
                // Ajustar jugadores si el chart nuevo no es co-op (o al revés).
                let (players, separated) = (chart.players, chart.separated);
                for n in notes.iter_mut() {
                    if players <= 1 && (n.player != 0 || n.block != 0) {
                        edit::set_note_player(n, 0, false, false);
                    } else if players > 1 && n.player > players {
                        edit::set_note_player(n, players, separated, n.token.is_some());
                    }
                }
                chart.notes = notes;
            }
        }
        if let Some(e) = self.song_snapshot() {
            self.push_entry(e);
        }
        let name = chart.short_name();
        let song = self.song.as_mut().unwrap();
        song.charts.push(chart);
        let idx = song.charts.len() - 1;
        self.dirty = true;
        self.select_chart_force(idx);
        self.msg(format!("Chart nuevo: {}", name));
    }

    // ------------------------------------------------------------------
    // Borrar chart
    // ------------------------------------------------------------------

    pub(super) fn ask_delete_chart(&mut self) {
        if self.chart().is_some() {
            self.confirm_delete_chart = Some(self.chart_idx);
        }
    }

    pub(super) fn delete_chart_window(&mut self, ctx: &egui::Context) {
        let Some(idx) = self.confirm_delete_chart else { return };
        let Some(name) = self.song.as_ref().and_then(|s| s.charts.get(idx)).map(|c| c.display_name()) else {
            self.confirm_delete_chart = None;
            return;
        };
        let mut delete = false;
        let mut cancel = false;
        egui::Window::new("Borrar chart").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.label(format!("¿Borrar el chart «{}»?", name));
            ui.weak("Se puede deshacer con Ctrl+Z.");
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Borrar").color(Color32::from_rgb(240, 110, 90))).clicked() {
                    delete = true;
                }
                if ui.button("Cancelar").clicked() {
                    cancel = true;
                }
            });
        });
        if cancel {
            self.confirm_delete_chart = None;
        }
        if delete {
            self.confirm_delete_chart = None;
            if let Some(e) = self.song_snapshot() {
                self.push_entry(e);
            }
            let song = self.song.as_mut().unwrap();
            song.charts.remove(idx);
            let new_idx = idx.min(song.charts.len().saturating_sub(1));
            self.dirty = true;
            self.select_chart_force(new_idx);
            self.msg(format!("Chart borrado: {}", name));
        }
    }

    // ------------------------------------------------------------------
    // Propiedades del chart
    // ------------------------------------------------------------------

    pub(super) fn chart_props_window(&mut self, ctx: &egui::Context) {
        if !self.show_chart_props {
            return;
        }
        let mut open = true;
        let mut sets: Vec<(&'static str, String)> = Vec::new();
        let mut new_style: Option<Style> = None;
        let mut new_players: Option<u8> = None;
        let mut delete = false;
        egui::Window::new("Propiedades del chart").open(&mut open).resizable(false).default_width(340.0).show(ctx, |ui| {
            let Some(chart) = self.song.as_ref().and_then(|s| s.charts.get(self.chart_idx)) else {
                ui.label("No hay chart. Crea uno con Chart → Nuevo chart (Shift+N).");
                return;
            };
            ui.label(RichText::new(chart.display_name()).color(style_color(chart)).strong());
            ui.separator();
            egui::Grid::new("chart_props").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label("Tipo");
                egui::ComboBox::from_id_salt("cp_style").selected_text(format!("{} ({} col.)", chart.style.name, chart.num_cols)).show_ui(ui, |ui| {
                    for s in style::STYLES.iter().filter(|s| s.num_cols() == chart.num_cols) {
                        if ui.selectable_label(s.id == chart.style.id, s.name).clicked() && s.id != chart.style.id {
                            new_style = Some(*s);
                        }
                    }
                })
                .response
                .on_hover_text("Sólo entre tipos con el mismo número de columnas");
                ui.end_row();

                ui.label("Dificultad");
                let mut diff = chart.difficulty();
                let shown = if diff.is_empty() { "(sin dificultad)".to_string() } else { diff.clone() };
                egui::ComboBox::from_id_salt("cp_diff").selected_text(shown).show_ui(ui, |ui| {
                    for d in DIFFICULTIES {
                        if ui.selectable_value(&mut diff, d.to_string(), d).clicked() {
                            sets.push(("DIFFICULTY", d.to_string()));
                        }
                    }
                });
                ui.end_row();

                ui.label("Nivel");
                let mut meter = chart.meter();
                if ui.add(egui::DragValue::new(&mut meter).range(0..=999)).changed() {
                    sets.push(("METER", meter.to_string()));
                }
                ui.end_row();

                for (key, label) in [("DESCRIPTION", "Descripción"), ("CHARTNAME", "Nombre del chart"), ("CREDIT", "Autor (step artist)")] {
                    ui.label(label);
                    let mut v = chart.get(key).unwrap_or_default();
                    if ui.add(egui::TextEdit::singleline(&mut v).desired_width(200.0)).changed() {
                        sets.push((key, v));
                    }
                    ui.end_row();
                }

                if chart.style.mode == PlayerMode::Shared && chart.style.id.ends_with("-p") {
                    ui.label("Jugadores");
                    let mut p = chart.players;
                    if ui.add(egui::DragValue::new(&mut p).range(1..=9)).changed() {
                        new_players = Some(p);
                    }
                    ui.end_row();
                }
            });
            ui.separator();
            let c = chart.counts();
            ui.weak(format!(
                "{} pasos · {} saltos · {} holds · {} rolls · {} minas{}",
                c.steps,
                c.jumps,
                c.holds,
                c.rolls,
                c.mines,
                if chart.timing.is_some() { " · timing propio" } else { "" }
            ));
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Borrar este chart…").color(Color32::from_rgb(240, 110, 90))).clicked() {
                    delete = true;
                }
            });
        });
        self.show_chart_props = open;
        if delete {
            self.ask_delete_chart();
        }
        if sets.is_empty() && new_style.is_none() && new_players.is_none() {
            return;
        }
        self.meta_begin();
        let idx = self.chart_idx;
        let Some(chart) = self.song.as_mut().and_then(|s| s.charts.get_mut(idx)) else { return };
        for (k, v) in sets {
            chart.set(k, &v);
        }
        let style_failed = new_style.map(|s| !chart.set_style(s)).unwrap_or(false);
        if let Some(p) = new_players {
            chart.set_players(p);
        }
        let players = chart.players.max(1);
        if style_failed {
            self.msg("Ese tipo tiene otro número de columnas.");
        }
        self.current_player = self.current_player.clamp(1, players);
        self.dirty = true;
        self.refresh_player_skins();
    }

    // ------------------------------------------------------------------
    // Propiedades de la canción
    // ------------------------------------------------------------------

    pub(super) fn song_props_window(&mut self, ctx: &egui::Context) {
        if !self.show_song_props {
            return;
        }
        let mut open = true;
        let mut sets: Vec<(String, Vec<String>)> = Vec::new();
        let mut preview_play = false;
        let row_sel = self.selection.as_ref().map(|s| (s.r0, s.r1));
        let cursor_time = self.cursor_time;
        let table = &self.table;
        egui::Window::new("Propiedades de la canción").open(&mut open).default_width(440.0).show(ctx, |ui| {
            let Some(song) = self.song.as_ref() else {
                ui.label("Abre o crea un simfile primero.");
                return;
            };
            let dir = song.dir();
            egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                egui::Grid::new("song_props").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
                    for (key, label) in SONG_TEXT {
                        ui.label(*label);
                        let mut v = song.get(key).unwrap_or_default();
                        if ui.add(egui::TextEdit::singleline(&mut v).desired_width(260.0)).changed() {
                            sets.push((key.to_string(), vec![v]));
                        }
                        ui.end_row();
                    }
                    for (key, label, exts) in SONG_FILES {
                        ui.label(*label);
                        ui.horizontal(|ui| {
                            let original = song.get(key).unwrap_or_default();
                            let mut v = original.clone();
                            let missing = !original.trim().is_empty() && song.file_resource(key).is_none();
                            let color = missing.then_some(Color32::from_rgb(240, 150, 90));
                            let resp = ui.add(egui::TextEdit::singleline(&mut v).desired_width(200.0).text_color_opt(color));
                            let resp = if missing { resp.on_hover_text("No se encontró ese archivo en la carpeta de la canción") } else { resp };
                            if resp.changed() {
                                sets.push((key.to_string(), vec![v.clone()]));
                            }
                            let files = folder_files(dir.as_deref(), exts);
                            egui::ComboBox::from_id_salt(("sp_file", *key)).selected_text("…").width(28.0).show_ui(ui, |ui| {
                                if files.is_empty() {
                                    ui.weak("(no hay archivos de ese tipo en la carpeta)");
                                }
                                for f in &files {
                                    if ui.selectable_label(v == *f, f).clicked() {
                                        sets.push((key.to_string(), vec![f.clone()]));
                                    }
                                }
                                if ui.selectable_label(false, RichText::new("(ninguno)").weak()).clicked() {
                                    sets.push((key.to_string(), vec![String::new()]));
                                }
                            });
                        });
                        ui.end_row();
                    }

                    // Display BPM
                    ui.label("Display BPM");
                    ui.horizontal(|ui| {
                        let (lo, hi) = song.timing.bpm_range();
                        let params: Vec<String> = song.header.iter().rev().find(|t| t.key() == "DISPLAYBPM").map(|t| t.params.clone()).unwrap_or_default();
                        let (mut mode, mut a, mut b) = parse_display_bpm(&params, lo, hi);
                        let before = (mode, a, b);
                        let names = ["Real (automático)", "Fijo", "Rango", "Aleatorio (*)"];
                        egui::ComboBox::from_id_salt("sp_dbpm").selected_text(names[mode as usize]).show_ui(ui, |ui| {
                            for (i, n) in names.iter().enumerate() {
                                ui.selectable_value(&mut mode, i as u8, *n);
                            }
                        });
                        if mode == 1 {
                            ui.add(egui::DragValue::new(&mut a).speed(0.5).range(0.0..=10000.0).max_decimals(3));
                        }
                        if mode == 2 {
                            ui.add(egui::DragValue::new(&mut a).speed(0.5).range(0.0..=10000.0).max_decimals(3));
                            ui.label("a");
                            ui.add(egui::DragValue::new(&mut b).speed(0.5).range(0.0..=10000.0).max_decimals(3));
                        }
                        if (mode, a, b) != before {
                            let p = match mode {
                                0 => vec![String::new()],
                                1 => vec![fmt_num(a)],
                                2 => vec![fmt_num(a), fmt_num(b)],
                                _ => vec!["*".to_string()],
                            };
                            sets.push(("DISPLAYBPM".into(), p));
                        }
                    });
                    ui.end_row();

                    // Preview
                    let start: f64 = song.get_or("SAMPLESTART").parse().unwrap_or(0.0);
                    let len: f64 = song.get_or("SAMPLELENGTH").parse().unwrap_or(0.0);
                    ui.label("Preview: inicio");
                    let mut s0 = start;
                    if ui.add(egui::DragValue::new(&mut s0).speed(0.05).range(0.0..=36000.0).fixed_decimals(3).suffix(" s")).changed() {
                        sets.push(("SAMPLESTART".into(), vec![format!("{:.6}", s0)]));
                    }
                    ui.end_row();
                    ui.label("Preview: duración");
                    let mut l0 = len;
                    if ui.add(egui::DragValue::new(&mut l0).speed(0.05).range(0.0..=600.0).fixed_decimals(3).suffix(" s")).changed() {
                        sets.push(("SAMPLELENGTH".into(), vec![format!("{:.6}", l0)]));
                    }
                    ui.end_row();
                    ui.label("");
                    ui.horizontal(|ui| {
                        if ui.add_enabled(row_sel.is_some(), egui::Button::new("Desde la selección")).on_hover_text("Inicio y duración según la región seleccionada").clicked() {
                            if let Some((r0, r1)) = row_sel {
                                let (t0, t1) = (table.time_at_row(r0), table.time_at_row(r1));
                                sets.push(("SAMPLESTART".into(), vec![format!("{:.6}", t0.max(0.0))]));
                                sets.push(("SAMPLELENGTH".into(), vec![format!("{:.6}", (t1 - t0).max(0.0))]));
                            }
                        }
                        if ui.button("Inicio en el cursor").clicked() {
                            sets.push(("SAMPLESTART".into(), vec![format!("{:.6}", cursor_time.max(0.0))]));
                        }
                        if ui.button("▶ Escuchar").clicked() {
                            preview_play = true;
                        }
                    });
                    ui.end_row();
                });

                // Otros tags (los de XSanity/StepF2 y demás).
                let others: Vec<(String, Vec<String>)> = song
                    .header
                    .iter()
                    .filter(|t| !is_known_song_key(&t.key()))
                    .map(|t| (t.name.clone(), t.params.clone()))
                    .collect();
                if !others.is_empty() {
                    ui.add_space(4.0);
                    egui::CollapsingHeader::new(format!("Otros tags ({}) — avanzado", others.len())).default_open(false).show(ui, |ui| {
                        egui::Grid::new("song_other").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                            for (name, params) in others {
                                ui.label(RichText::new(&name).monospace().small());
                                let mut v = params.join(":");
                                let multi = v.contains('\n') || v.len() > 60;
                                let edit = if multi {
                                    egui::TextEdit::multiline(&mut v).desired_rows(2).desired_width(260.0).font(egui::TextStyle::Monospace)
                                } else {
                                    egui::TextEdit::singleline(&mut v).desired_width(260.0)
                                };
                                if ui.add(edit).changed() {
                                    // Si el tag tenía varios parámetros, ':' los separa.
                                    let p = if params.len() > 1 { v.split(':').map(|s| s.to_string()).collect() } else { vec![v] };
                                    sets.push((name.to_ascii_uppercase(), p));
                                }
                                ui.end_row();
                            }
                        });
                    });
                }
            });
        });
        self.show_song_props = open;

        if preview_play {
            self.play_preview();
        }
        if sets.is_empty() {
            return;
        }
        self.meta_begin();
        let mut reload_music = false;
        let mut reload_bg = false;
        if let Some(song) = self.song.as_mut() {
            for (k, p) in sets {
                let old = song.header.iter().rev().find(|t| t.key() == k).map(|t| t.params.clone());
                if old.as_ref() == Some(&p) {
                    continue;
                }
                reload_music |= k == "MUSIC";
                reload_bg |= k == "BACKGROUND";
                song.set_params(&k, p);
            }
        }
        self.dirty = true;
        if reload_music || reload_bg {
            self.load_media(reload_music, reload_bg);
        }
    }

    /// Reproduce el preview (#SAMPLESTART / #SAMPLELENGTH) y lo detiene al terminar.
    pub(super) fn play_preview(&mut self) {
        let Some(song) = self.song.as_ref() else { return };
        let start: f64 = song.get_or("SAMPLESTART").parse().unwrap_or(0.0);
        let len: f64 = song.get_or("SAMPLELENGTH").parse().unwrap_or(0.0);
        if self.music.is_none() {
            self.msg("El audio todavía no está cargado.");
            return;
        }
        let len = if len > 0.0 { len } else { 12.0 };
        self.placing.clear();
        self.audio.play(start);
        self.preview_stop = Some(start + len);
        self.msg(format!("Preview: {:.2} s → {:.2} s", start, start + len));
    }
}
