//! Aplicación principal: estado del editor, entrada de teclado/mouse y paneles.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Instant;

use egui::{Color32, Event, Key, Modifiers, RichText};

use crate::audio::{self, AudioData, AudioPlayer, Waveform};
use crate::edit::{self, Clip, Convert, NoteFilter, SegKind, Selection};
use crate::minimap::{self, MinimapInput};
use crate::notefield::{self, FieldInput, Layout, ViewSettings};
use crate::noteskin::{self, ExternalSkin, Noteskin};
use crate::simfile::notes::{self, Note, NoteKind};
use crate::simfile::style::{Game, PlayerMode};
use crate::simfile::timing::{beat_to_row, row_to_beat, ScrollMap, TimingData, TimingTable, ROWS_PER_BEAT, ROWS_PER_MEASURE};
use crate::simfile::{self, Chart, SimFormat, Song, Tag};

mod dialogs;
mod skins_import;

const SNAPS: [i32; 10] = [4, 8, 12, 16, 24, 32, 48, 64, 96, 192];
const MAX_RECENT: usize = 12;

enum Loaded {
    Audio(u64, Result<(Arc<AudioData>, Arc<Waveform>), String>),
    Background(u64, Result<egui::ColorImage, String>),
}

#[derive(Clone)]
enum Pending {
    Open(PathBuf),
    OpenDialog,
    NewFromAudio,
    Close,
    Quit,
}

/// Estado guardado para deshacer: las notas de un chart o un timing completo.
enum UndoEntry {
    Notes { chart: usize, notes: Vec<Note> },
    /// `chart = None`: timing de la canción; `Some(i)`: timing propio del chart `i`.
    Timing { chart: Option<usize>, timing: Option<TimingData> },
    /// La canción completa (propiedades, charts nuevos o borrados).
    Song { header: Vec<Tag>, timing: TimingData, charts: Vec<Chart>, chart_idx: usize },
}

/// Acciones del editor de tempo (se aplican después de dibujar la ventana).
enum TempoAction {
    Offset(f64),
    Set(SegKind, i32, f64),
    Remove(SegKind, i32),
    Jump(i32),
    CreateOwn,
    DropOwn,
    /// Insertar (filas > 0) o borrar (filas < 0) en el cursor; `true` = en todos los charts.
    InsertRows(i32, bool),
}

/// ¿Hay un campo de texto con el foco? (entonces los atajos no se procesan)
fn text_input_active(ctx: &egui::Context) -> bool {
    let focused = ctx.memory(|m| m.focused());
    focused.map(|id| egui::text_edit::TextEditState::load(ctx, id).is_some()).unwrap_or(false)
}

/// Opciones de línea de comandos: `archivo [--chart N] [--beat B] [--view time|rows|scroll]`.
#[derive(Default)]
pub struct StartOptions {
    pub file: Option<PathBuf>,
    pub chart: Option<usize>,
    pub beat: Option<f64>,
    pub view: Option<String>,
    /// `--select A:B`: región de beats seleccionada al abrir.
    pub select: Option<(f64, f64)>,
    /// `--mirror h|v|hv`: aplica un espejo a la selección (para pruebas).
    pub mirror: Option<String>,
    /// `--tempo`: abre el editor de tempo.
    pub tempo: bool,
    /// `--minimap notas|densidad`: modo del minimapa al abrir.
    pub minimap: Option<String>,
    /// `--noteskin <nombre>`: noteskin importado o del simulador a usar ("" = integrado).
    pub noteskin: Option<String>,
    /// `--dialog cancion|chart|nuevo|importar`: abre ese diálogo al iniciar.
    pub dialog: Option<String>,
}

impl StartOptions {
    pub fn from_args(args: &[String]) -> Self {
        let mut o = StartOptions::default();
        let mut i = 1;
        while i < args.len() {
            let a = &args[i];
            let next = args.get(i + 1);
            match a.as_str() {
                "--chart" => {
                    o.chart = next.and_then(|v| v.parse().ok());
                    i += 1;
                }
                "--beat" => {
                    o.beat = next.and_then(|v| v.parse().ok());
                    i += 1;
                }
                "--view" => {
                    o.view = next.cloned();
                    i += 1;
                }
                "--select" => {
                    o.select = next.and_then(|v| {
                        let (a, b) = v.split_once(':')?;
                        Some((a.parse().ok()?, b.parse().ok()?))
                    });
                    i += 1;
                }
                "--mirror" => {
                    o.mirror = next.cloned();
                    i += 1;
                }
                "--tempo" => o.tempo = true,
                "--minimap" => {
                    o.minimap = next.cloned();
                    i += 1;
                }
                "--noteskin" => {
                    o.noteskin = next.cloned();
                    i += 1;
                }
                "--dialog" => {
                    o.dialog = next.cloned();
                    i += 1;
                }
                _ if !a.starts_with("--") && o.file.is_none() => o.file = Some(PathBuf::from(a)),
                _ => {}
            }
            i += 1;
        }
        o
    }
}

pub struct App {
    song: Option<Song>,
    chart_idx: usize,
    table: TimingTable,
    scroll_map: Arc<ScrollMap>,
    view: ViewSettings,
    cursor_time: f64,
    cursor_beat: f64,
    snap_idx: usize,
    audio: AudioPlayer,
    music: Option<Arc<AudioData>>,
    waveform: Option<Arc<Waveform>>,
    audio_status: String,
    load_gen: u64,
    tx: Sender<Loaded>,
    rx: Receiver<Loaded>,
    background: Option<egui::TextureHandle>,
    skin: Option<Noteskin>,
    messages: Vec<(Instant, String)>,
    dirty: bool,
    undo: Vec<UndoEntry>,
    redo: Vec<UndoEntry>,
    /// Teclas de columna mantenidas: columna -> (fila inicial, bloque, roll).
    placing: HashMap<usize, (i32, u8, bool)>,
    current_player: u8,
    show_shortcuts: bool,
    show_side: bool,
    recent: Vec<PathBuf>,
    wheel_accum: f32,
    pending: Option<Pending>,
    confirm: Option<Pending>,
    allow_close: bool,
    layout: Option<Layout>,
    last_title: String,
    play_end: f64,
    /// Región seleccionada.
    selection: Option<Selection>,
    /// Fila donde empezó una selección con Tab (se cierra con otro Tab).
    sel_anchor: Option<i32>,
    /// Inicio de una selección con el mouse: (beat, columna).
    drag_start: Option<(f64, usize)>,
    clipboard: Option<Clip>,
    tab_presses: u32,
    tab_down: bool,
    show_tempo: bool,
    /// Hay una edición de tempo en curso (para agrupar un arrastre en un solo deshacer).
    tempo_edit_active: bool,
    egui_ctx: egui::Context,
    /// Noteskin externo elegido (importado o del simulador) y caché de los ya cargados.
    ext_skin: Option<Arc<ExternalSkin>>,
    skin_cache: HashMap<String, Arc<ExternalSkin>>,
    /// Noteskins por jugador del chart co-op actual (`#PRELOADNOTESKIN`).
    player_skins: Vec<Arc<ExternalSkin>>,
    /// Carpetas de noteskins, en orden de prioridad (ver `compute_skin_dirs`).
    skin_dirs: Vec<PathBuf>,
    /// Noteskins disponibles: (nombre, carpeta de origen).
    skin_list: Vec<(String, PathBuf)>,
    /// Diálogo para importar los noteskins de un simulador.
    import: Option<skins_import::ImportSkins>,
    show_song_props: bool,
    show_chart_props: bool,
    new_chart: Option<dialogs::NewChartForm>,
    confirm_delete_chart: Option<usize>,
    /// Edición de propiedades en curso (agrupa un texto escrito en un solo deshacer).
    meta_edit_active: bool,
    meta_changed: bool,
    /// Tiempo en que se detiene la reproducción del preview.
    preview_stop: Option<f64>,
    /// Editor de tempo → insertar / borrar beats: cantidad y si se aplica a todos los charts.
    insert_beats: f64,
    insert_all_charts: bool,
}

fn setup_fonts(ctx: &egui::Context) {
    // Fuentes del sistema como respaldo para títulos en coreano/japonés/chino.
    let mut fonts = egui::FontDefinitions::default();
    let dir = std::env::var("WINDIR").map(|w| PathBuf::from(w).join("Fonts")).unwrap_or_else(|_| PathBuf::from(r"C:\Windows\Fonts"));
    for (name, file) in [("malgun", "malgun.ttf"), ("meiryo", "meiryo.ttc"), ("msgothic", "msgothic.ttc"), ("msyh", "msyh.ttc")] {
        if let Ok(bytes) = std::fs::read(dir.join(file)) {
            fonts.font_data.insert(name.to_owned(), Arc::new(egui::FontData::from_owned(bytes)));
            for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                if let Some(list) = fonts.families.get_mut(&fam) {
                    list.push(name.to_owned());
                }
            }
        }
    }
    ctx.set_fonts(fonts);
}

/// Consume las pulsaciones de `key` con exactamente estos modificadores.
fn take_key(i: &mut egui::InputState, mods: Modifiers, key: Key) -> usize {
    let mut n = 0;
    i.events.retain(|e| match e {
        Event::Key { key: k, pressed: true, modifiers, .. } if *k == key && modifiers.matches_exact(mods) => {
            n += 1;
            false
        }
        _ => true,
    });
    n
}

/// "1 beat", "2 beats", "0.5 beats".
fn fmt_beats(beats: f64) -> String {
    let n = format!("{:.3}", beats);
    let n = n.trim_end_matches('0').trim_end_matches('.');
    format!("{} {}", n, if n == "1" { "beat" } else { "beats" })
}

fn fmt_time(t: f64) -> String {
    let sign = if t < 0.0 { "-" } else { "" };
    let t = t.abs();
    let m = (t / 60.0).floor();
    let s = t - m * 60.0;
    format!("{}{:02}:{:06.3}", sign, m as i64, s)
}

/// Multiplicador con 2 decimales sin ceros sobrantes: 1, 0.5, 2.25.
fn fmt_ratio(v: f64) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

fn style_color(chart: &Chart) -> Color32 {
    if chart.players > 1 || chart.style.mode != PlayerMode::Single {
        return Color32::from_rgb(240, 190, 60);
    }
    match chart.style.short {
        "S" => Color32::from_rgb(240, 110, 80),
        "D" => Color32::from_rgb(90, 200, 110),
        "HD" => Color32::from_rgb(200, 130, 230),
        _ => Color32::from_rgb(170, 170, 170),
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, start: StartOptions) -> Self {
        setup_fonts(&cc.egui_ctx);
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        cc.egui_ctx.set_visuals(egui::Visuals::dark());

        let (view, recent, snap_idx, volume) = match cc.storage {
            Some(st) => (
                eframe::get_value::<ViewSettings>(st, "view").unwrap_or_default(),
                eframe::get_value::<Vec<PathBuf>>(st, "recent").unwrap_or_default(),
                eframe::get_value::<usize>(st, "snap").unwrap_or(3).min(SNAPS.len() - 1),
                eframe::get_value::<f32>(st, "volume").unwrap_or(0.8),
            ),
            None => (ViewSettings::default(), Vec::new(), 3, 0.8),
        };

        let audio = AudioPlayer::new();
        audio.set_volume(volume);
        let (tx, rx) = channel();
        let mut app = App {
            song: None,
            chart_idx: 0,
            table: TimingData { bpms: vec![simfile::timing::BpmSeg { row: 0, bpm: 120.0 }], ..Default::default() }.build_table(),
            scroll_map: Arc::new(ScrollMap::new(&TimingData::default(), &TimingData::default().build_table())),
            view,
            cursor_time: 0.0,
            cursor_beat: 0.0,
            snap_idx,
            audio,
            music: None,
            waveform: None,
            audio_status: String::new(),
            load_gen: 0,
            tx,
            rx,
            background: None,
            skin: Noteskin::load(&cc.egui_ctx),
            messages: Vec::new(),
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            placing: HashMap::new(),
            current_player: 1,
            show_shortcuts: false,
            show_side: true,
            recent,
            wheel_accum: 0.0,
            pending: None,
            confirm: None,
            allow_close: false,
            layout: None,
            last_title: String::new(),
            play_end: 0.0,
            selection: None,
            sel_anchor: None,
            drag_start: None,
            clipboard: None,
            tab_presses: 0,
            tab_down: false,
            show_tempo: false,
            tempo_edit_active: false,
            egui_ctx: cc.egui_ctx.clone(),
            ext_skin: None,
            skin_cache: HashMap::new(),
            player_skins: Vec::new(),
            skin_dirs: Vec::new(),
            skin_list: Vec::new(),
            import: None,
            show_song_props: false,
            show_chart_props: false,
            new_chart: None,
            confirm_delete_chart: None,
            meta_edit_active: false,
            meta_changed: false,
            preview_stop: None,
            insert_beats: 1.0,
            insert_all_charts: false,
        };
        if let Some(n) = start.noteskin.clone() {
            app.view.noteskin = n;
            app.view.vector_skin = false;
        }
        let dirs = app.compute_skin_dirs(start.file.as_deref());
        app.set_skin_dirs(dirs);
        if let Some(e) = app.audio.error.clone() {
            app.msg(format!("Audio no disponible: {}", e));
        }
        match start.view.as_deref() {
            Some("time") => app.view.time_based = true,
            Some("rows") => {
                app.view.time_based = false;
                app.view.apply_scrolls = false;
            }
            Some("scroll") | Some("game") => {
                app.view.time_based = false;
                app.view.apply_scrolls = true;
            }
            _ => {}
        }
        if let Some(p) = start.file {
            app.open_path(&p);
            if let Some(c) = start.chart {
                app.select_chart(c);
            }
            if let Some(b) = start.beat {
                app.set_cursor_beat(b);
            }
            if let Some((a, b)) = start.select {
                let cols = app.num_cols();
                app.selection = Some(Selection::rows(beat_to_row(a), beat_to_row(b), cols));
            }
            match start.mirror.as_deref() {
                Some("h") => app.mirror_selection(true, false),
                Some("v") => app.mirror_selection(false, true),
                Some("hv") => app.mirror_selection(true, true),
                _ => {}
            }
        }
        app.show_tempo = start.tempo;
        match start.dialog.as_deref() {
            Some("cancion") | Some("song") => app.show_song_props = true,
            Some("chart") => app.show_chart_props = true,
            Some("nuevo") | Some("new") => app.open_new_chart(),
            Some("importar") | Some("import") => app.open_import_skins(),
            _ => {}
        }
        match start.minimap.as_deref() {
            Some("densidad") | Some("density") => app.view.minimap_density = true,
            Some("notas") | Some("notes") => app.view.minimap_density = false,
            _ => {}
        }
        app
    }

    fn msg(&mut self, s: impl Into<String>) {
        let s = s.into();
        eprintln!("{}", s);
        self.messages.push((Instant::now(), s));
        if self.messages.len() > 6 {
            self.messages.remove(0);
        }
    }

    // ------------------------------------------------------------------
    // Archivos
    // ------------------------------------------------------------------

    fn request(&mut self, p: Pending) {
        if self.dirty {
            self.confirm = Some(p);
        } else {
            self.pending = Some(p);
        }
    }

    fn run_pending(&mut self, ctx: &egui::Context) {
        let Some(p) = self.pending.take() else { return };
        match p {
            Pending::Open(path) => self.open_path(&path),
            Pending::OpenDialog => {
                let mut dlg = rfd::FileDialog::new()
                    .add_filter("Simfiles", &["ssc", "sm"])
                    .add_filter("Audio (simfile nuevo)", &["ogg", "mp3", "wav", "flac"])
                    .add_filter("Todos", &["*"]);
                if let Some(dir) = self.song.as_ref().and_then(|s| s.dir()).or_else(|| self.recent.first().and_then(|p| p.parent().map(|d| d.to_path_buf()))) {
                    dlg = dlg.set_directory(dir);
                }
                if let Some(path) = dlg.pick_file() {
                    self.open_path(&path);
                }
            }
            Pending::NewFromAudio => self.new_song_from_audio_dialog(),
            Pending::Close => self.close_song(),
            Pending::Quit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn close_song(&mut self) {
        self.audio.stop();
        self.audio.set_music(None);
        self.song = None;
        self.music = None;
        self.waveform = None;
        self.background = None;
        self.dirty = false;
        self.undo.clear();
        self.redo.clear();
        self.clear_selection();
        self.new_chart = None;
        self.confirm_delete_chart = None;
        self.preview_stop = None;
        self.audio_status.clear();
        self.load_gen += 1;
    }

    fn open_path(&mut self, path: &Path) {
        let is_audio = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| matches!(e.to_ascii_lowercase().as_str(), "ogg" | "mp3" | "wav" | "flac"))
            .unwrap_or(false);
        if is_audio {
            self.new_song_from_audio(path);
            return;
        }
        let song = match simfile::load(path) {
            Ok(s) => s,
            Err(e) => {
                self.msg(e);
                return;
            }
        };
        self.close_song();
        for w in &song.warnings {
            self.messages.push((Instant::now(), w.clone()));
        }
        let first_pump = song.charts.iter().position(|c| c.style.game == Game::Pump).unwrap_or(0);
        let n_charts = song.charts.len();
        self.song = Some(song);
        self.chart_idx = first_pump;
        self.current_player = 1;
        self.rebuild_timing();
        // La canción puede estar en otro simulador con más noteskins.
        let dirs = self.compute_skin_dirs(Some(path));
        if dirs != self.skin_dirs {
            self.set_skin_dirs(dirs);
        } else {
            self.refresh_player_skins();
        }
        self.set_cursor_beat(0.0);

        self.add_recent(path);

        self.msg(format!("Abierto: {} ({} charts)", path.file_name().and_then(|n| n.to_str()).unwrap_or(""), n_charts));

        // Audio y fondo en segundo plano.
        self.load_media(true, true);
    }

    /// Carga en segundo plano el audio (#MUSIC) y/o el fondo (#BACKGROUND) de la canción.
    fn load_media(&mut self, music: bool, background: bool) {
        let Some(song) = self.song.as_ref() else { return };
        let music_path = song.music_path();
        let bg_path = song.file_resource("BACKGROUND");
        let gen = self.load_gen;
        if music {
            match music_path {
                Some(mp) => {
                    self.audio_status = format!("Cargando {}...", mp.file_name().and_then(|n| n.to_str()).unwrap_or(""));
                    let tx = self.tx.clone();
                    std::thread::spawn(move || {
                        let r = audio::decode_file(&mp).map(|a| {
                            let wf = Waveform::build(&a);
                            (Arc::new(a), Arc::new(wf))
                        });
                        let _ = tx.send(Loaded::Audio(gen, r));
                    });
                }
                None => {
                    self.audio_status = "Sin audio (no se encontró #MUSIC)".into();
                    self.audio.stop();
                    self.audio.set_music(None);
                    self.music = None;
                    self.waveform = None;
                }
            }
        }
        if background {
            match bg_path {
                Some(bp) => {
                    let tx = self.tx.clone();
                    std::thread::spawn(move || {
                        let r = image::open(&bp).map_err(|e| e.to_string()).map(|img| {
                            let img = if img.width() > 1920 || img.height() > 1080 { img.thumbnail(1920, 1080) } else { img };
                            let rgba = img.to_rgba8();
                            egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw())
                        });
                        let _ = tx.send(Loaded::Background(gen, r));
                    });
                }
                None => self.background = None,
            }
        }
    }

    fn poll_loaders(&mut self, ctx: &egui::Context) {
        while let Ok(l) = self.rx.try_recv() {
            match l {
                Loaded::Audio(gen, r) if gen == self.load_gen => match r {
                    Ok((a, wf)) => {
                        self.audio_status = format!("{:.0} Hz, {}", a.rate, fmt_time(a.duration()));
                        self.audio.set_music(Some(a.clone()));
                        self.music = Some(a);
                        self.waveform = Some(wf);
                        self.update_ticks();
                    }
                    Err(e) => {
                        self.audio_status = format!("Error: {}", e);
                        self.msg(format!("Audio: {}", e));
                    }
                },
                Loaded::Background(gen, r) if gen == self.load_gen => {
                    if let Ok(img) = r {
                        self.background = Some(ctx.load_texture("background", img, egui::TextureOptions::LINEAR));
                    }
                }
                _ => {}
            }
        }
    }

    /// Pone el archivo al principio de los recientes.
    fn add_recent(&mut self, path: &Path) {
        let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let abs = PathBuf::from(abs.to_string_lossy().trim_start_matches(r"\\?\").to_string());
        self.recent.retain(|p| p != &abs);
        self.recent.insert(0, abs);
        self.recent.truncate(MAX_RECENT);
    }

    fn save(&mut self, save_as: bool) {
        let Some(song) = self.song.as_ref() else { return };
        let target = match (&song.path, song.format, save_as) {
            (Some(p), SimFormat::Ssc, false) => Some(p.clone()),
            (Some(p), SimFormat::Sm, false) => Some(p.with_extension("ssc")),
            _ => {
                let mut dlg = rfd::FileDialog::new().add_filter("StepMania 5 (.ssc)", &["ssc"]);
                if let Some(p) = &song.path {
                    if let Some(d) = p.parent() {
                        dlg = dlg.set_directory(d);
                    }
                    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                        dlg = dlg.set_file_name(format!("{}.ssc", stem));
                    }
                }
                dlg.save_file()
            }
        };
        let Some(target) = target else { return };
        match simfile::save_ssc(song, &target) {
            Ok(()) => {
                let reload = simfile::load(&target);
                let song = self.song.as_mut().unwrap();
                song.path = Some(target.clone());
                song.format = SimFormat::Ssc;
                // Las notas guardadas pasan a ser el estado "original".
                if let Ok(saved) = reload {
                    for (c, s) in song.charts.iter_mut().zip(saved.charts.into_iter()) {
                        c.notes_raw = s.notes_raw;
                        c.notes_orig = s.notes_orig;
                    }
                }
                self.dirty = false;
                // Un simfile nuevo (desde un audio) o un "guardar como" también va a recientes.
                self.add_recent(&target);
                self.msg(format!("Guardado: {}", target.display()));
            }
            Err(e) => self.msg(e),
        }
    }

    // ------------------------------------------------------------------
    // Chart / timing / cursor
    // ------------------------------------------------------------------

    fn chart(&self) -> Option<&Chart> {
        self.song.as_ref().and_then(|s| s.charts.get(self.chart_idx))
    }

    fn timing(&self) -> Option<&TimingData> {
        self.song.as_ref().map(|s| s.chart_timing(self.chart_idx))
    }

    fn select_chart(&mut self, idx: usize) {
        let Some(song) = self.song.as_ref() else { return };
        if song.charts.is_empty() {
            return;
        }
        let idx = idx.min(song.charts.len() - 1);
        if idx == self.chart_idx {
            return;
        }
        let beat = self.cursor_beat;
        self.chart_idx = idx;
        self.placing.clear();
        self.clear_selection();
        self.refresh_player_skins();
        let max_p = self.chart().map(|c| c.players).unwrap_or(1).max(1);
        self.current_player = self.current_player.clamp(1, max_p);
        self.rebuild_timing();
        self.set_cursor_beat(beat);
        if self.audio.is_playing() {
            self.audio.play(self.cursor_time);
        }
    }

    /// Selecciona un chart aunque sea el mismo índice (tras crear o borrar charts).
    fn select_chart_force(&mut self, idx: usize) {
        let beat = self.cursor_beat;
        let n = self.song.as_ref().map(|s| s.charts.len()).unwrap_or(0);
        self.chart_idx = idx.min(n.saturating_sub(1));
        self.placing.clear();
        self.clear_selection();
        let max_p = self.chart().map(|c| c.players).unwrap_or(1).max(1);
        self.current_player = self.current_player.clamp(1, max_p);
        self.refresh_player_skins();
        self.rebuild_timing();
        self.set_cursor_beat(beat);
    }

    fn rebuild_timing(&mut self) {
        if let Some(td) = self.timing() {
            let table = td.build_table();
            let map = Arc::new(ScrollMap::new(td, &table));
            self.table = table;
            self.scroll_map = map;
        }
        self.update_ticks();
    }

    fn update_ticks(&mut self) {
        let Some(chart) = self.chart() else {
            self.audio.set_ticks(Vec::new(), Vec::new());
            return;
        };
        let mut rows: Vec<i32> = chart
            .notes
            .iter()
            .filter(|n| !n.fake && matches!(n.kind, NoteKind::Tap | NoteKind::Hold | NoteKind::Roll | NoteKind::Lift))
            .map(|n| n.row)
            .collect();
        rows.dedup();
        let note_ticks: Vec<f64> = rows.iter().map(|&r| self.table.time_at_row(r)).collect();
        let last_beat = row_to_beat(chart.last_row());
        let audio_end = self.music.as_ref().map(|m| self.table.beat_at_time(m.duration())).unwrap_or(0.0);
        let end = last_beat.max(audio_end).ceil() as i64 + 4;
        let beat_ticks: Vec<f64> = (0..end.min(20000)).map(|b| self.table.time_at_beat(b as f64)).collect();
        self.play_end = note_ticks.last().copied().unwrap_or(0.0).max(self.music.as_ref().map(|m| m.duration()).unwrap_or(0.0)) + 2.0;
        self.audio.set_ticks(note_ticks, beat_ticks);
    }

    /// Mueve el cursor a un beat. Al navegar a mano el cursor no baja del beat 0
    /// (compás 0); sólo la reproducción puede estar antes, si la música empieza antes.
    fn set_cursor_beat(&mut self, beat: f64) {
        let beat = beat.max(0.0);
        self.cursor_beat = beat;
        self.cursor_time = self.table.time_at_beat(beat);
    }

    fn set_cursor_time(&mut self, t: f64) {
        self.cursor_time = t;
        self.cursor_beat = self.table.beat_at_time(t);
    }

    fn snap(&self) -> i32 {
        SNAPS[self.snap_idx]
    }

    fn snap_rows(&self) -> i32 {
        (ROWS_PER_MEASURE / self.snap()).max(1)
    }

    fn cursor_row(&self) -> i32 {
        beat_to_row(self.cursor_beat)
    }

    /// Mueve el cursor `steps` líneas de snap (negativo = hacia atrás).
    fn move_snap(&mut self, steps: i32) {
        if steps == 0 {
            return;
        }
        let step = self.snap_rows() as f64;
        let cur = self.cursor_beat * ROWS_PER_BEAT as f64 / step;
        let target = if steps > 0 { (cur + 1e-6).floor() + steps as f64 } else { (cur - 1e-6).ceil() + steps as f64 };
        self.jump_to_row((target * step).round() as i32);
    }

    /// Mueve el cursor al siguiente/anterior múltiplo de `rows` filas (beat o compás).
    fn move_rows(&mut self, rows: i32) {
        let unit = rows.abs().max(1);
        let r = self.cursor_row();
        let target = if rows > 0 { r.div_euclid(unit) * unit + unit } else { (r + unit - 1).div_euclid(unit) * unit - unit };
        self.jump_to_row(target);
    }

    fn jump_to_row(&mut self, row: i32) {
        let was_playing = self.audio.is_playing();
        self.set_cursor_beat(row_to_beat(row));
        if was_playing {
            self.audio.play(self.cursor_time);
        }
        if let Some(a) = self.sel_anchor {
            self.selection = Some(Selection::rows(a, self.cursor_row(), self.num_cols()));
        }
        self.update_placing();
    }

    fn toggle_play(&mut self) {
        self.preview_stop = None;
        if self.audio.is_playing() {
            self.audio.stop();
            // Al detener, el cursor vuelve a la fila más cercana del snap.
            let step = self.snap_rows();
            let r = ((self.cursor_beat * ROWS_PER_BEAT as f64) / step as f64).round() as i32 * step;
            self.set_cursor_beat(row_to_beat(r));
        } else if self.song.is_some() {
            self.placing.clear();
            self.audio.play(self.cursor_time);
        }
    }

    fn zoom(&mut self, factor: f32) {
        self.view.zoom = (self.view.zoom * factor).clamp(10.0, 3000.0);
    }

    // ------------------------------------------------------------------
    // Edición
    // ------------------------------------------------------------------

    fn push_entry(&mut self, e: UndoEntry) {
        self.undo.push(e);
        if self.undo.len() > 300 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Guarda las notas del chart actual para poder deshacer.
    fn push_undo(&mut self) {
        if let Some(c) = self.chart() {
            let e = UndoEntry::Notes { chart: self.chart_idx, notes: c.notes.clone() };
            self.push_entry(e);
        }
    }

    /// Guarda el timing que se está editando (el del chart si tiene propio, si no el de la canción).
    fn push_undo_timing(&mut self) {
        let Some(song) = self.song.as_ref() else { return };
        let own = song.charts.get(self.chart_idx).map(|c| c.timing.is_some()).unwrap_or(false);
        let e = if own {
            UndoEntry::Timing { chart: Some(self.chart_idx), timing: song.charts[self.chart_idx].timing.clone() }
        } else {
            UndoEntry::Timing { chart: None, timing: Some(song.timing.clone()) }
        };
        self.push_entry(e);
    }

    fn undo_redo(&mut self, undo: bool) {
        let entry = if undo { self.undo.pop() } else { self.redo.pop() };
        let Some(entry) = entry else { return };
        let Some(song) = self.song.as_mut() else { return };
        let back = match entry {
            UndoEntry::Notes { chart, notes } => {
                let Some(c) = song.charts.get_mut(chart) else { return };
                let current = std::mem::replace(&mut c.notes, notes);
                if self.chart_idx != chart {
                    self.chart_idx = chart;
                    self.clear_selection();
                }
                UndoEntry::Notes { chart, notes: current }
            }
            UndoEntry::Timing { chart: None, timing } => {
                let current = std::mem::replace(&mut song.timing, timing.unwrap_or_default());
                UndoEntry::Timing { chart: None, timing: Some(current) }
            }
            UndoEntry::Song { header, timing, charts, chart_idx } => {
                let back = UndoEntry::Song {
                    header: std::mem::replace(&mut song.header, header),
                    timing: std::mem::replace(&mut song.timing, timing),
                    charts: std::mem::replace(&mut song.charts, charts),
                    chart_idx: self.chart_idx,
                };
                self.chart_idx = chart_idx;
                self.clear_selection();
                back
            }
            UndoEntry::Timing { chart: Some(i), timing } => {
                let Some(c) = song.charts.get_mut(i) else { return };
                let current = std::mem::replace(&mut c.timing, timing);
                if self.chart_idx != i {
                    self.chart_idx = i;
                    self.clear_selection();
                }
                UndoEntry::Timing { chart: Some(i), timing: current }
            }
        };
        if undo {
            self.redo.push(back);
        } else {
            self.undo.push(back);
        }
        self.dirty = true;
        self.meta_edit_active = false;
        let n = self.song.as_ref().map(|s| s.charts.len()).unwrap_or(0);
        self.chart_idx = self.chart_idx.min(n.saturating_sub(1));
        self.refresh_player_skins();
        self.after_timing_change();
        self.msg(if undo { "Deshacer" } else { "Rehacer" });
    }

    fn chart_mut(&mut self) -> Option<&mut Chart> {
        let idx = self.chart_idx;
        self.song.as_mut().and_then(|s| s.charts.get_mut(idx))
    }

    fn num_cols(&self) -> usize {
        self.chart().map(|c| c.num_cols).unwrap_or(1)
    }

    fn notes_changed(&mut self) {
        self.dirty = true;
        self.update_ticks();
    }

    // ------------------------------------------------------------------
    // Noteskins externos (importados o del simulador)
    // ------------------------------------------------------------------

    /// Carpetas de noteskins, en orden de prioridad: la elegida a mano, la de los importados
    /// de un simulador, `noteskins/pump` junto a la app (portátil) y la del simulador de la
    /// canción abierta (hermana de su carpeta `Songs`).
    fn compute_skin_dirs(&self, song: Option<&Path>) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = Vec::new();
        if !self.view.skins_dir.is_empty() {
            let p = PathBuf::from(&self.view.skins_dir);
            if p.is_dir() {
                dirs.push(p);
            }
        }
        dirs.extend(noteskin::imported_skins_dir().filter(|d| d.is_dir()));
        dirs.extend(noteskin::portable_skins_dir());
        dirs.extend(noteskin::guess_skins_dir(song));
        let mut seen: Vec<PathBuf> = Vec::new();
        dirs.retain(|d| {
            let key = d.canonicalize().unwrap_or_else(|_| d.clone());
            if seen.contains(&key) {
                false
            } else {
                seen.push(key);
                true
            }
        });
        dirs
    }

    fn set_skin_dirs(&mut self, dirs: Vec<PathBuf>) {
        let mut list: Vec<(String, PathBuf)> = Vec::new();
        for d in &dirs {
            for n in noteskin::list_skins(d) {
                if !list.iter().any(|(m, _)| m.eq_ignore_ascii_case(&n)) {
                    list.push((n, d.clone()));
                }
            }
        }
        list.sort_by_key(|(n, _)| n.to_ascii_lowercase());
        self.skin_list = list;
        self.skin_dirs = dirs;
        self.skin_cache.clear();
        self.reload_skins();
    }

    /// Etiqueta de una carpeta de noteskins para el menú.
    fn skin_dir_label(&self, dir: &Path) -> &'static str {
        if !self.view.skins_dir.is_empty() && Path::new(&self.view.skins_dir) == dir {
            "Carpeta elegida"
        } else if noteskin::imported_skins_dir().as_deref() == Some(dir) {
            "Importados"
        } else if noteskin::portable_skins_dir().as_deref() == Some(dir) {
            "Junto a la app"
        } else {
            "Simulador"
        }
    }

    /// Noteskin por nombre (desde la caché o cargándolo de la carpeta).
    fn get_skin(&mut self, name: &str) -> Option<Arc<ExternalSkin>> {
        if let Some(s) = self.skin_cache.get(name) {
            return Some(s.clone());
        }
        let base = self.skin_list.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, d)| d.clone())?;
        let dir = base.join(name);
        if !dir.is_dir() {
            return None;
        }
        match noteskin::load_external(&self.egui_ctx, &dir) {
            Ok(s) => {
                let s = Arc::new(s);
                self.skin_cache.insert(name.to_string(), s.clone());
                Some(s)
            }
            Err(e) => {
                self.msg(e);
                None
            }
        }
    }

    fn reload_skins(&mut self) {
        self.ext_skin = if self.view.noteskin.is_empty() {
            None
        } else {
            let n = self.view.noteskin.clone();
            let s = self.get_skin(&n);
            if s.is_none() {
                self.msg(format!("No se encontró el noteskin \"{}\"; se usa el integrado.", n));
            }
            s
        };
        self.refresh_player_skins();
    }

    /// En charts co-op, cada jugador usa el noteskin que declara `#PRELOADNOTESKIN`.
    fn refresh_player_skins(&mut self) {
        self.player_skins.clear();
        if !self.view.coop_skins || self.skin_list.is_empty() {
            return;
        }
        let names: Vec<String> = match self.chart() {
            Some(c) if c.players > 1 => c
                .get_or("PRELOADNOTESKIN")
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            _ => Vec::new(),
        };
        for n in names {
            match self.get_skin(&n) {
                Some(s) => self.player_skins.push(s),
                None => break,
            }
        }
    }

    fn choose_skin(&mut self, name: Option<String>, vector: bool) {
        self.view.vector_skin = vector;
        self.view.noteskin = name.unwrap_or_default();
        self.reload_skins();
        let label = if vector {
            "vectorial".to_string()
        } else if self.view.noteskin.is_empty() {
            "ArrowVortex pump (integrado)".to_string()
        } else {
            self.view.noteskin.clone()
        };
        self.msg(format!("Noteskin: {}", label));
    }

    // ------------------------------------------------------------------
    // Selección, portapapeles y espejo
    // ------------------------------------------------------------------

    fn clear_selection(&mut self) {
        self.selection = None;
        self.sel_anchor = None;
        self.drag_start = None;
    }

    fn need_selection(&mut self) -> Option<Selection> {
        if self.selection.is_none() {
            self.msg("Primero selecciona una región (Tab, arrastrar con el mouse o Ctrl+A).");
        }
        self.selection.clone()
    }

    /// Región de todo el chart (todas las filas con notas y todas las columnas).
    fn whole_chart(&self) -> Option<Selection> {
        let c = self.chart()?;
        let first = c.notes.iter().map(|n| n.row).min().unwrap_or(0).min(0);
        Some(Selection::rows(first, c.last_row(), c.num_cols))
    }

    /// Selecciona por tipo de nota: dentro de la región actual o, si no hay, en todo el chart.
    fn select_filtered(&mut self, make: impl FnOnce(&Chart, &TimingData) -> NoteFilter) {
        let region = match &self.selection {
            Some(s) => Selection { filter: None, ..s.clone() },
            None => match self.whole_chart() {
                Some(r) => r,
                None => return,
            },
        };
        let (sel, n) = {
            let Some(song) = self.song.as_ref() else { return };
            let Some(chart) = song.charts.get(self.chart_idx) else { return };
            let filter = make(chart, song.chart_timing(self.chart_idx));
            let sel = region.with_filter(filter);
            let n = edit::count_selected(&chart.notes, &sel);
            (sel, n)
        };
        let name = sel.filter.as_ref().map(|f| f.name()).unwrap_or_default();
        self.sel_anchor = None;
        self.selection = Some(sel);
        self.msg(format!("Seleccionadas {} {}", n, name));
    }

    /// Tab: empieza una región en el cursor; el segundo Tab la cierra.
    fn region_key(&mut self) {
        let cur = self.cursor_row();
        let cols = self.num_cols();
        match self.sel_anchor.take() {
            Some(a) => {
                let sel = Selection::rows(a, cur, cols);
                let n = self.chart().map(|c| edit::count_selected(&c.notes, &sel)).unwrap_or(0);
                self.selection = Some(sel);
                self.msg(format!("Región seleccionada: {} notas", n));
            }
            None => {
                self.sel_anchor = Some(cur);
                self.selection = Some(Selection::rows(cur, cur, cols));
            }
        }
    }

    fn select_all(&mut self) {
        self.sel_anchor = None;
        self.selection = self.whole_chart();
    }

    /// Selecciona desde el inicio hasta el cursor (`before`) o desde el cursor hasta el final.
    fn select_from_cursor(&mut self, before: bool) {
        let Some(c) = self.chart() else { return };
        let cur = self.cursor_row();
        let (a, b) = if before {
            (c.notes.iter().map(|n| n.row).min().unwrap_or(0).min(0), cur)
        } else {
            (cur, c.last_row().max(cur))
        };
        let cols = c.num_cols;
        self.sel_anchor = None;
        self.selection = Some(Selection::rows(a, b, cols));
    }

    fn copy_selection(&mut self, ctx: &egui::Context, cut: bool) {
        let Some(sel) = self.need_selection() else { return };
        let Some(chart) = self.chart() else { return };
        let clip = edit::copy(&chart.notes, &sel);
        let n = clip.notes.len();
        ctx.copy_text(clip.to_text());
        self.clipboard = Some(clip);
        if cut && n > 0 && !self.audio.is_playing() {
            self.push_undo();
            if let Some(chart) = self.chart_mut() {
                edit::delete(&mut chart.notes, &sel);
            }
            self.notes_changed();
            self.msg(format!("Cortadas {} notas", n));
        } else {
            self.msg(format!("Copiadas {} notas", n));
        }
    }

    fn delete_selection(&mut self) {
        let Some(sel) = self.need_selection() else { return };
        if self.audio.is_playing() {
            return;
        }
        let n = self.chart().map(|c| edit::count_selected(&c.notes, &sel)).unwrap_or(0);
        if n == 0 {
            return;
        }
        self.push_undo();
        if let Some(chart) = self.chart_mut() {
            edit::delete(&mut chart.notes, &sel);
        }
        self.notes_changed();
        self.msg(format!("Borradas {} notas", n));
    }

    /// Pega en el cursor. `text` es el portapapeles del sistema (si trae notas de
    /// Step Charter se usa; si no, el portapapeles interno).
    fn paste(&mut self, text: Option<String>) {
        if self.audio.is_playing() {
            return;
        }
        let clip = text.as_deref().and_then(Clip::from_text).or_else(|| self.clipboard.clone());
        let Some(mut clip) = clip else {
            self.msg("No hay notas copiadas.");
            return;
        };
        let Some(chart) = self.chart() else { return };
        // Adaptar los jugadores al chart de destino.
        let (target_player, _, use_tokens) = self.new_note_target(chart);
        let players = chart.players;
        let separated = chart.separated;
        for n in clip.notes.iter_mut() {
            if players <= 1 {
                if n.player != 0 || n.block != 0 {
                    edit::set_note_player(n, 0, false, false);
                }
            } else if n.player == 0 || n.player > players {
                edit::set_note_player(n, target_player, separated, use_tokens);
            } else {
                n.block = if separated { n.player - 1 } else { 0 };
            }
        }
        let at = self.cursor_row();
        let num_cols = chart.num_cols;
        self.push_undo();
        let Some(chart) = self.chart_mut() else { return };
        let (sel, added) = edit::paste(&mut chart.notes, &clip, at, num_cols);
        self.selection = Some(sel);
        self.sel_anchor = None;
        self.notes_changed();
        self.msg(format!("Pegadas {} notas", added));
    }

    /// Códigos de los menús de selección por tipo: 0..8 tipos, 50+p jugador, 100+q cuantización.
    fn select_by_code(&mut self, code: u8) {
        match code {
            0 => self.select_filtered(|_, _| NoteFilter::Steps),
            1 => self.select_filtered(|c, _| NoteFilter::jumps(&c.notes)),
            2 => self.select_filtered(|_, _| NoteFilter::Mines),
            3 => self.select_filtered(|_, _| NoteFilter::Holds),
            4 => self.select_filtered(|_, _| NoteFilter::Rolls),
            5 => self.select_filtered(|_, _| NoteFilter::Lifts),
            6 => self.select_filtered(|_, _| NoteFilter::Fakes),
            7 => self.select_filtered(|c, td| NoteFilter::warped(&c.notes, td)),
            8 => self.select_filtered(|_, _| NoteFilter::Specials),
            51..=59 => self.select_filtered(move |_, _| NoteFilter::Player(code - 50)),
            100.. => {
                let q = (code - 100) as i32;
                let q = if q == 155 { 192 } else { q };
                self.select_filtered(move |_, _| NoteFilter::Quant(q));
            }
            _ => {}
        }
    }

    /// Espejo de la selección: horizontal (izq↔der), vertical (arriba↔abajo) o ambos.
    fn mirror_selection(&mut self, horizontal: bool, vertical: bool) {
        let Some(chart) = self.chart() else { return };
        let mut perm: Vec<usize> = (0..chart.num_cols).collect();
        for (on, h) in [(horizontal, true), (vertical, false)] {
            if on {
                let p = chart.style.mirror_perm(h);
                perm = perm.iter().map(|&c| p.get(c).copied().unwrap_or(c)).collect();
            }
        }
        let what = match (horizontal, vertical) {
            (true, true) => "Espejo completo",
            (true, false) => "Espejo horizontal",
            _ => "Espejo vertical",
        };
        self.permute_selection(perm, what);
    }

    /// Pasa las notas seleccionadas a la otra mitad del pad (en double, de P1 a P2) sin espejarlas.
    fn switch_side(&mut self) {
        let perm = edit::side_perm(self.num_cols());
        self.permute_selection(perm, "Cambio de lado");
    }

    /// Mueve de columna las notas seleccionadas según `perm` (espejos y cambio de lado).
    fn permute_selection(&mut self, perm: Vec<usize>, what: &str) {
        let Some(sel) = self.need_selection() else { return };
        if self.audio.is_playing() {
            return;
        }
        let Some(chart) = self.chart() else { return };
        if edit::count_selected(&chart.notes, &sel) == 0 {
            self.msg("No hay notas en la selección.");
            return;
        }
        self.push_undo();
        let Some(chart) = self.chart_mut() else { return };
        let n = edit::mirror(&mut chart.notes, &sel, &perm);
        // Las columnas de la región también se mueven.
        let mapped: Vec<usize> = (sel.c0..=sel.c1).map(|c| perm.get(c).copied().unwrap_or(c)).collect();
        let c0 = mapped.iter().copied().min().unwrap_or(sel.c0);
        let c1 = mapped.iter().copied().max().unwrap_or(sel.c1);
        self.selection = Some(Selection { c0, c1, ..sel });
        self.notes_changed();
        self.msg(format!("{}: {} notas", what, n));
    }

    /// Convierte el tipo de las notas seleccionadas (*Editar → Convertir*).
    fn convert_selection(&mut self, conv: Convert) {
        let Some(sel) = self.need_selection() else { return };
        if self.audio.is_playing() {
            return;
        }
        let Some(chart) = self.chart() else { return };
        // Se prueba sobre una copia para no dejar un paso de deshacer vacío.
        let mut notes = chart.notes.clone();
        let n = edit::convert(&mut notes, &sel, conv);
        if n == 0 {
            self.msg(format!("{}: no hay notas de ese tipo en la selección.", conv.label()));
            return;
        }
        self.push_undo();
        if let Some(chart) = self.chart_mut() {
            chart.notes = notes;
        }
        // Si la selección filtraba por el tipo de origen, pasa a seguir a las notas convertidas.
        if let Some(f) = sel.filter.as_ref().and_then(|f| edit::filter_after_convert(f, conv)) {
            self.selection = Some(Selection { filter: Some(f), ..sel });
        }
        self.notes_changed();
        self.msg(format!("{}: {} notas", conv.label(), n));
    }

    /// Escala la selección por `num/den` (*Editar → Escalar*).
    fn scale_selection(&mut self, num: i32, den: i32) {
        let Some(sel) = self.need_selection() else { return };
        if self.audio.is_playing() {
            return;
        }
        let Some(chart) = self.chart() else { return };
        if edit::count_selected(&chart.notes, &sel) == 0 {
            self.msg("No hay notas en la selección.");
            return;
        }
        self.push_undo();
        let Some(chart) = self.chart_mut() else { return };
        let (n, truncated) = edit::scale(&mut chart.notes, &sel, num, den);
        self.notes_changed();
        let verb = if num > den { "Expandidas" } else { "Comprimidas" };
        let cut = if truncated { "; las que salían de la región se eliminaron" } else { "" };
        self.msg(format!("{} {}:{} {} notas{}", verb, num, den, n, cut));
    }

    /// Tecla `/`: con notas seleccionadas en un chart co-op, las pasa al jugador siguiente;
    /// si no, cambia el jugador de las notas nuevas (como en ArrowVortex).
    fn slash_key(&mut self) {
        let Some(chart) = self.chart() else { return };
        let players = chart.players.max(1);
        if players <= 1 {
            self.msg("Este chart tiene un solo jugador.");
            return;
        }
        let sel = self.selection.clone().filter(|s| edit::count_selected(&chart.notes, s) > 0);
        let Some(sel) = sel else {
            self.current_player = self.current_player % players + 1;
            let p = self.current_player;
            self.msg(format!("Jugador para notas nuevas: P{}", p));
            return;
        };
        if self.audio.is_playing() {
            return;
        }
        let separated = chart.separated;
        let (_, _, use_tokens) = self.new_note_target(chart);
        self.push_undo();
        let Some(chart) = self.chart_mut() else { return };
        let (n, to) = edit::cycle_players(&mut chart.notes, &sel, players, separated, use_tokens);
        // Una selección por jugador sigue a las notas al jugador nuevo.
        if let (Some(NoteFilter::Player(_)), Some(p)) = (&sel.filter, to) {
            self.selection = Some(Selection { filter: Some(NoteFilter::Player(p)), ..sel });
        }
        self.notes_changed();
        self.msg(match to {
            Some(p) => format!("{} notas pasadas a P{}", n, p),
            None => format!("Jugador cambiado en {} notas", n),
        });
    }

    /// Inserta (`rows > 0`) o borra (`rows < 0`) filas en el cursor, desplazando notas y
    /// segmentos de timing: en este chart (y el timing que usa) o en todos los charts.
    fn insert_rows(&mut self, rows: i32, all_charts: bool) {
        if rows == 0 || self.audio.is_playing() {
            return;
        }
        let at = self.cursor_row();
        let Some(snap) = self.song_snapshot() else { return };
        self.push_entry(snap);
        self.tempo_edit_active = false;
        let idx = self.chart_idx;
        let Some(song) = self.song.as_mut() else { return };
        let mut removed = 0;
        if all_charts {
            edit::insert_rows_timing(&mut song.timing, at, rows);
            for c in song.charts.iter_mut() {
                if let Some(td) = c.timing.as_mut() {
                    edit::insert_rows_timing(td, at, rows);
                }
                removed += edit::insert_rows_notes(&mut c.notes, at, rows);
            }
        } else if idx < song.charts.len() {
            match song.charts[idx].timing.as_mut() {
                Some(td) => edit::insert_rows_timing(td, at, rows),
                None => edit::insert_rows_timing(&mut song.timing, at, rows),
            }
            removed += edit::insert_rows_notes(&mut song.charts[idx].notes, at, rows);
        }
        self.selection = None;
        self.sel_anchor = None;
        self.notes_changed();
        let beats = rows.abs() as f64 / ROWS_PER_BEAT as f64;
        let what = match (rows > 0, rows.abs() == ROWS_PER_BEAT) {
            (true, true) => "Insertado",
            (true, false) => "Insertados",
            (false, true) => "Borrado",
            (false, false) => "Borrados",
        };
        let scope = if all_charts { "todos los charts" } else { "este chart" };
        let gone = if removed > 0 { format!("; {} notas eliminadas", removed) } else { String::new() };
        self.msg(format!("{} {} en el beat {:.3} ({}){}", what, fmt_beats(beats), row_to_beat(at), scope, gone));
    }

    // ------------------------------------------------------------------
    // Edición de tempo
    // ------------------------------------------------------------------

    /// Timing que edita el editor de tempo: el propio del chart o el de la canción.
    fn edit_timing_mut(&mut self) -> Option<&mut TimingData> {
        let idx = self.chart_idx;
        let song = self.song.as_mut()?;
        if song.charts.get(idx).map(|c| c.timing.is_some()).unwrap_or(false) {
            return song.charts[idx].timing.as_mut();
        }
        Some(&mut song.timing)
    }

    /// Tras cambiar el timing: recalcular tablas y ticks, conservando el beat del cursor.
    fn after_timing_change(&mut self) {
        let beat = self.cursor_beat;
        let playing = self.audio.is_playing();
        self.rebuild_timing();
        self.set_cursor_beat(beat);
        if playing {
            self.audio.play(self.cursor_time);
        }
    }

    fn apply_tempo_actions(&mut self, ctx: &egui::Context, actions: Vec<TempoAction>) {
        if actions.is_empty() {
            if !ctx.input(|i| i.pointer.any_down()) && !text_input_active(ctx) {
                self.tempo_edit_active = false;
            }
            return;
        }
        let mut changed = false;
        for a in actions {
            match a {
                TempoAction::Jump(r) => self.jump_to_row(r),
                TempoAction::InsertRows(rows, all) => {
                    self.insert_rows(rows, all);
                    changed = true;
                }
                TempoAction::CreateOwn | TempoAction::DropOwn => {
                    let idx = self.chart_idx;
                    let Some(song) = self.song.as_ref() else { continue };
                    let prev = song.charts.get(idx).map(|c| c.timing.clone());
                    let Some(prev) = prev else { continue };
                    self.push_entry(UndoEntry::Timing { chart: Some(idx), timing: prev });
                    let song = self.song.as_mut().unwrap();
                    let new = if matches!(a, TempoAction::CreateOwn) { Some(song.timing.clone()) } else { None };
                    song.charts[idx].timing = new;
                    self.tempo_edit_active = true;
                    changed = true;
                }
                TempoAction::Offset(_) | TempoAction::Set(..) | TempoAction::Remove(..) => {
                    if !self.tempo_edit_active {
                        self.push_undo_timing();
                        self.tempo_edit_active = true;
                    }
                    let Some(td) = self.edit_timing_mut() else { continue };
                    match a {
                        TempoAction::Offset(v) => td.offset = v,
                        TempoAction::Set(k, r, v) => edit::seg_set(td, k, r, v),
                        TempoAction::Remove(k, r) => {
                            edit::seg_remove(td, k, r);
                        }
                        _ => {}
                    }
                    changed = true;
                }
            }
        }
        if changed {
            self.dirty = true;
            self.after_timing_change();
        }
    }

    fn tempo_window(&mut self, ctx: &egui::Context) {
        if !self.show_tempo {
            return;
        }
        let mut open = true;
        let mut actions: Vec<TempoAction> = Vec::new();
        let row = self.cursor_row();
        let screen = ctx.screen_rect();
        let (mut ins_beats, mut ins_all) = (self.insert_beats, self.insert_all_charts);
        egui::Window::new("Tempo y sincronía")
            .open(&mut open)
            .default_pos(egui::pos2(screen.right() - 360.0, screen.top() + 60.0))
            .default_width(330.0)
            .resizable(false)
            .show(ctx, |ui| {
                let Some(song) = self.song.as_ref() else {
                    ui.label("Abre un simfile primero.");
                    return;
                };
                let Some(chart) = song.charts.get(self.chart_idx) else {
                    ui.label("Esta canción no tiene charts.");
                    return;
                };
                let own = chart.timing.is_some();
                let td = song.chart_timing(self.chart_idx);
                if own {
                    ui.label(RichText::new("Editando el timing propio de este chart").strong());
                    if ui.button("Usar el timing de la canción").on_hover_text("Descarta el timing propio de este chart").clicked() {
                        actions.push(TempoAction::DropOwn);
                    }
                } else {
                    let users = song.charts.iter().filter(|c| c.timing.is_none()).count();
                    let who = if users == 1 { "1 chart lo usa".to_string() } else { format!("{} charts lo usan", users) };
                    ui.label(RichText::new(format!("Editando el timing de la canción ({})", who)).strong());
                    if ui.button("Crear timing propio para este chart").on_hover_text("Copia el timing de la canción a este chart (split timing)").clicked() {
                        actions.push(TempoAction::CreateOwn);
                    }
                }
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Offset");
                    let mut off = td.offset;
                    if ui
                        .add(egui::DragValue::new(&mut off).speed(0.001).fixed_decimals(3).suffix(" s"))
                        .on_hover_text("Inicio de la música respecto al primer beat, en segundos (#OFFSET)")
                        .changed()
                    {
                        actions.push(TempoAction::Offset(off));
                    }
                    for (lbl, d) in [("−10ms", -0.010), ("−1ms", -0.001), ("+1ms", 0.001), ("+10ms", 0.010)] {
                        if ui.small_button(lbl).clicked() {
                            actions.push(TempoAction::Offset(td.offset + d));
                        }
                    }
                });
                let first_row = td.bpms.first().map(|b| b.row).unwrap_or(0);
                let bpm0 = td.bpms.first().map(|b| b.bpm).unwrap_or(120.0);
                ui.horizontal(|ui| {
                    ui.label("BPM inicial");
                    let mut v = bpm0;
                    if ui
                        .add(egui::DragValue::new(&mut v).speed(0.1).range(0.001..=100000.0).fixed_decimals(3))
                        .on_hover_text("Tempo en el primer beat")
                        .changed()
                    {
                        actions.push(TempoAction::Set(SegKind::Bpm, first_row, v));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Mover primer beat");
                    let beat = 60.0 / bpm0.max(0.001);
                    for (lbl, k, tip) in [
                        ("−1 beat", -1.0, "Disminuye el offset de la música en un beat"),
                        ("−½", -0.5, "Disminuye el offset de la música en medio beat"),
                        ("+½", 0.5, "Aumenta el offset de la música en medio beat"),
                        ("+1 beat", 1.0, "Aumenta el offset de la música en un beat"),
                    ] {
                        if ui.small_button(lbl).on_hover_text(tip).clicked() {
                            actions.push(TempoAction::Offset(td.offset + k * beat));
                        }
                    }
                });
                ui.separator();

                ui.label(format!("En el cursor: beat {:.3}  (fila {})", row_to_beat(row), row));
                egui::Grid::new("tempo_at_cursor").num_columns(3).spacing([8.0, 4.0]).show(ui, |ui| {
                    for kind in SegKind::ALL {
                        let exact = edit::seg_at(td, kind, row);
                        let mut v = edit::seg_effective(td, kind, row);
                        let label = if exact.is_some() { RichText::new(kind.name()).strong() } else { RichText::new(kind.name()) };
                        ui.label(label);
                        let speed = match kind {
                            SegKind::Bpm => 0.1,
                            SegKind::Scroll | SegKind::Warp => 0.01,
                            _ => 0.001,
                        };
                        let unit = kind.unit();
                        let resp = ui.add(
                            egui::DragValue::new(&mut v)
                                .speed(speed)
                                .range(kind.min()..=100000.0)
                                .fixed_decimals(3)
                                .suffix(if unit.is_empty() { String::new() } else { format!(" {}", unit) }),
                        );
                        if resp.changed() {
                            actions.push(TempoAction::Set(kind, row, v));
                        }
                        ui.horizontal(|ui| {
                            if exact.is_some() {
                                let can = edit::seg_can_remove(td, kind, row);
                                if ui.add_enabled(can, egui::Button::new("✖").small()).on_hover_text("Quitar este segmento").clicked() {
                                    actions.push(TempoAction::Remove(kind, row));
                                }
                            } else {
                                ui.weak(if kind.inherits() { "heredado" } else { "—" });
                            }
                            if kind == SegKind::Bpm {
                                if ui.small_button("×2").on_hover_text("Doblar el BPM").clicked() {
                                    actions.push(TempoAction::Set(kind, row, v * 2.0));
                                }
                                if ui.small_button("÷2").on_hover_text("Mitad del BPM").clicked() {
                                    actions.push(TempoAction::Set(kind, row, v / 2.0));
                                }
                            }
                        });
                        ui.end_row();
                    }
                });
                ui.small("Si cambias un valor donde no hay segmento, se crea uno en el cursor.");
                ui.separator();

                ui.label(RichText::new("Insertar o borrar beats en el cursor").strong());
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut ins_beats).speed(0.25).range(0.0..=10000.0).max_decimals(3).suffix(" beats"));
                    egui::ComboBox::from_id_salt("insert_target")
                        .selected_text(if ins_all { "Todos los charts" } else { "Este chart" })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut ins_all, false, "Este chart");
                            ui.selectable_value(&mut ins_all, true, "Todos los charts");
                        })
                        .response
                        .on_hover_text("A qué notas y cambios de tempo se aplica");
                });
                let ins_rows = (ins_beats * ROWS_PER_BEAT as f64).round() as i32;
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(ins_rows > 0, egui::Button::new("Insertar"))
                        .on_hover_text("Desplaza hacia abajo las notas y los cambios de tempo desde el cursor")
                        .clicked()
                    {
                        actions.push(TempoAction::InsertRows(ins_rows, ins_all));
                    }
                    if ui
                        .add_enabled(ins_rows > 0, egui::Button::new("Borrar"))
                        .on_hover_text("Elimina las notas y los cambios de tempo de esa zona y sube lo que viene después")
                        .clicked()
                    {
                        actions.push(TempoAction::InsertRows(-ins_rows, ins_all));
                    }
                });
                if !ins_all && !own {
                    let others = song.charts.iter().filter(|c| c.timing.is_none()).count().saturating_sub(1);
                    if others > 0 {
                        ui.small(format!(
                            "Este chart usa el timing de la canción: sus cambios de tempo también se moverán para los otros {} charts que lo usan (sus notas no).",
                            others
                        ));
                    }
                }
                ui.separator();

                egui::CollapsingHeader::new("Todos los segmentos").default_open(false).show(ui, |ui| {
                    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                        for kind in SegKind::ALL {
                            let list = edit::seg_list(td, kind);
                            if list.is_empty() {
                                continue;
                            }
                            ui.strong(format!("{} ({})", kind.name(), list.len()));
                            for (r, v) in list {
                                ui.horizontal(|ui| {
                                    if ui.small_button("ir").clicked() {
                                        actions.push(TempoAction::Jump(r));
                                    }
                                    let can = edit::seg_can_remove(td, kind, r);
                                    if ui.add_enabled(can, egui::Button::new("✖").small()).clicked() {
                                        actions.push(TempoAction::Remove(kind, r));
                                    }
                                    let txt = RichText::new(edit::seg_label(kind, r, v)).monospace();
                                    ui.label(if r == row { txt.strong() } else { txt });
                                });
                            }
                        }
                    });
                });
            });
        self.show_tempo = open;
        self.insert_beats = ins_beats;
        self.insert_all_charts = ins_all;
        self.apply_tempo_actions(ctx, actions);
    }

    /// (jugador, bloque, usar tokens de StepF2) para notas nuevas en el chart actual.
    fn new_note_target(&self, chart: &Chart) -> (u8, u8, bool) {
        if chart.players > 1 {
            let p = self.current_player.clamp(1, chart.players);
            let block = if chart.separated { p - 1 } else { 0 };
            let has_tokens = chart.notes.iter().any(|n| n.token.as_deref().map(|t| t.starts_with('{')).unwrap_or(false));
            (p, block, has_tokens || !chart.separated)
        } else {
            (0, 0, false)
        }
    }

    fn toggle_note(&mut self, col: usize, kind: NoteKind, roll: bool) {
        if self.audio.is_playing() {
            return;
        }
        let Some(chart) = self.chart() else { return };
        if col >= chart.num_cols {
            return;
        }
        let row = self.cursor_row();
        let (player, block, tokens) = self.new_note_target(chart);
        let existing = chart.notes.iter().position(|n| n.row == row && n.col as usize == col && n.block == block);
        self.push_undo();
        let chart = &mut self.song.as_mut().unwrap().charts[self.chart_idx];
        if let Some(i) = existing {
            let same_kind = chart.notes[i].kind == kind;
            chart.notes.remove(i);
            if same_kind {
                self.dirty = true;
                self.update_ticks();
                return;
            }
        }
        let mut n = Note::new(row, col as u8, kind);
        n.player = player;
        n.block = block;
        if tokens {
            let ch = match kind {
                NoteKind::Mine => 'M',
                _ => '1',
            };
            n.token = Some(format!("{{{}{}0}}", ch, player));
        }
        chart.notes.push(n);
        notes::sort_notes(&mut chart.notes);
        if kind == NoteKind::Tap {
            self.placing.insert(col, (row, block, roll));
        }
        self.dirty = true;
        self.update_ticks();
    }

    /// Mientras se mantiene una tecla de columna y se mueve el cursor, la nota se convierte en hold/roll.
    fn update_placing(&mut self) {
        if self.placing.is_empty() {
            return;
        }
        let cur = self.cursor_row();
        let placing: Vec<(usize, (i32, u8, bool))> = self.placing.iter().map(|(k, v)| (*k, *v)).collect();
        let Some(song) = self.song.as_mut() else { return };
        let chart = &mut song.charts[self.chart_idx];
        let mut changed = false;
        for (col, (start, block, roll)) in placing {
            if let Some(n) = chart.notes.iter_mut().find(|n| n.row == start && n.col as usize == col && n.block == block) {
                let player = n.player;
                if cur > start {
                    n.kind = if roll { NoteKind::Roll } else { NoteKind::Hold };
                    n.end_row = cur;
                    if let Some(t) = n.token.as_mut() {
                        let ch = if roll { '4' } else { '2' };
                        *t = format!("{{{}{}0}}", ch, player);
                        n.tail_token = Some(format!("{{3{}0}}", player));
                    }
                } else {
                    n.kind = NoteKind::Tap;
                    n.end_row = start;
                    if let Some(t) = n.token.as_mut() {
                        *t = format!("{{1{}0}}", player);
                        n.tail_token = None;
                    }
                }
                changed = true;
            }
        }
        if changed {
            // Quitar notas tapadas por el hold recién extendido.
            let holds: Vec<(usize, i32, i32, u8)> = chart
                .notes
                .iter()
                .filter(|n| n.is_long())
                .map(|n| (n.col as usize, n.row, n.end_row, n.block))
                .collect();
            chart.notes.retain(|n| !holds.iter().any(|&(c, a, b, bl)| n.col as usize == c && n.block == bl && n.row > a && n.row <= b));
            self.dirty = true;
            self.update_ticks();
        }
    }

    // ------------------------------------------------------------------
    // Entrada
    // ------------------------------------------------------------------

    fn handle_input(&mut self, ctx: &egui::Context) {
        if text_input_active(ctx) {
            self.tab_presses = 0;
            return;
        }
        for _ in 0..std::mem::take(&mut self.tab_presses) {
            self.region_key();
        }
        let none = Modifiers::NONE;
        let ctrl = Modifiers::COMMAND;
        let shift = Modifiers::SHIFT;
        let alt = Modifiers::ALT;
        let ctrl_shift = Modifiers { shift: true, ..Modifiers::COMMAND };

        let mut actions: Vec<Box<dyn FnOnce(&mut App)>> = Vec::new();
        ctx.input_mut(|i| {
            macro_rules! on {
                ($m:expr, $k:expr, $f:expr) => {
                    let n = take_key(i, $m, $k);
                    for _ in 0..n {
                        actions.push(Box::new($f));
                    }
                };
            }
            on!(ctrl, Key::O, |a: &mut App| a.request(Pending::OpenDialog));
            on!(ctrl_shift, Key::S, |a: &mut App| a.save(true));
            on!(ctrl, Key::S, |a: &mut App| a.save(false));
            on!(ctrl, Key::Z, |a: &mut App| a.undo_redo(true));
            on!(ctrl_shift, Key::Z, |a: &mut App| a.undo_redo(false));
            on!(ctrl, Key::Y, |a: &mut App| a.undo_redo(false));
            on!(none, Key::Space, |a: &mut App| a.toggle_play());
            on!(none, Key::ArrowUp, |a: &mut App| a.move_snap(-1));
            on!(none, Key::ArrowDown, |a: &mut App| a.move_snap(1));
            on!(none, Key::ArrowLeft, |a: &mut App| a.snap_idx = a.snap_idx.saturating_sub(1));
            on!(none, Key::ArrowRight, |a: &mut App| a.snap_idx = (a.snap_idx + 1).min(SNAPS.len() - 1));
            on!(shift, Key::PageUp, |a: &mut App| a.move_rows(-ROWS_PER_BEAT));
            on!(shift, Key::PageDown, |a: &mut App| a.move_rows(ROWS_PER_BEAT));
            on!(none, Key::PageUp, |a: &mut App| a.move_rows(-ROWS_PER_MEASURE));
            on!(none, Key::PageDown, |a: &mut App| a.move_rows(ROWS_PER_MEASURE));
            on!(none, Key::Home, |a: &mut App| a.jump_to_row(0));
            on!(none, Key::End, |a: &mut App| {
                let r = a.chart().map(|c| c.last_row()).unwrap_or(0);
                a.jump_to_row(r)
            });
            on!(none, Key::F5, |a: &mut App| a.select_chart(a.chart_idx.saturating_sub(1)));
            on!(none, Key::F6, |a: &mut App| a.select_chart(a.chart_idx + 1));
            on!(none, Key::F1, |a: &mut App| a.show_shortcuts = !a.show_shortcuts);
            on!(none, Key::F3, |a: &mut App| {
                let (b, n) = a.audio.ticks_enabled();
                a.audio.set_ticks_enabled(!b, n);
            });
            on!(none, Key::F4, |a: &mut App| {
                let (b, n) = a.audio.ticks_enabled();
                a.audio.set_ticks_enabled(b, !n);
            });
            on!(shift, Key::ArrowUp, |a: &mut App| a.audio.set_volume(a.audio.volume() + 0.05));
            on!(shift, Key::ArrowDown, |a: &mut App| a.audio.set_volume(a.audio.volume() - 0.05));
            on!(shift, Key::ArrowRight, |a: &mut App| a.audio.set_speed(a.audio.speed() + 0.05));
            on!(shift, Key::ArrowLeft, |a: &mut App| a.audio.set_speed(a.audio.speed() - 0.05));
            on!(ctrl, Key::Plus, |a: &mut App| a.view.note_scale = (a.view.note_scale * 1.1).min(2.5));
            on!(ctrl, Key::Equals, |a: &mut App| a.view.note_scale = (a.view.note_scale * 1.1).min(2.5));
            on!(ctrl, Key::Minus, |a: &mut App| a.view.note_scale = (a.view.note_scale / 1.1).max(0.3));
            on!(none, Key::Plus, |a: &mut App| a.zoom(1.2));
            on!(none, Key::Equals, |a: &mut App| a.zoom(1.2));
            on!(none, Key::Minus, |a: &mut App| a.zoom(1.0 / 1.2));
            on!(none, Key::Slash, |a: &mut App| a.slash_key());
            // La tecla bajo Esc (` en inglés, º en español), como "accent" en ArrowVortex.
            on!(none, Key::Backtick, |a: &mut App| a.convert_selection(Convert::HoldsRolls));
            on!(shift, Key::R, |a: &mut App| a.view.reverse = !a.view.reverse);
            on!(shift, Key::V, |a: &mut App| a.view.time_based = !a.view.time_based);
            on!(shift, Key::M, |a: &mut App| a.view.show_minimap = !a.view.show_minimap);
            on!(shift, Key::T, |a: &mut App| a.show_tempo = !a.show_tempo);
            on!(shift, Key::P, |a: &mut App| a.show_song_props = !a.show_song_props);
            on!(shift, Key::C, |a: &mut App| a.show_chart_props = !a.show_chart_props);
            on!(shift, Key::N, |a: &mut App| a.open_new_chart());
            on!(ctrl, Key::Delete, |a: &mut App| a.ask_delete_chart());
            on!(shift, Key::S, |a: &mut App| a.show_tempo = true);

            // Selección y espejo (teclas de ArrowVortex: T vertical, Y horizontal, G completo).
            on!(none, Key::Escape, |a: &mut App| a.clear_selection());
            on!(none, Key::Delete, |a: &mut App| a.delete_selection());
            on!(none, Key::Backspace, |a: &mut App| a.delete_selection());
            on!(ctrl, Key::A, |a: &mut App| a.select_all());
            on!(ctrl, Key::Home, |a: &mut App| {
                if let Some(sel) = a.need_selection() {
                    a.jump_to_row(sel.r0);
                }
            });
            on!(ctrl, Key::End, |a: &mut App| {
                if let Some(sel) = a.need_selection() {
                    a.jump_to_row(sel.r1);
                }
            });
            on!(shift, Key::Home, |a: &mut App| a.select_from_cursor(true));
            on!(shift, Key::End, |a: &mut App| a.select_from_cursor(false));
            on!(none, Key::T, |a: &mut App| a.mirror_selection(false, true));
            on!(none, Key::Y, |a: &mut App| a.mirror_selection(true, false));
            on!(none, Key::G, |a: &mut App| a.mirror_selection(true, true));

            // Ctrl+C / Ctrl+X / Ctrl+V llegan como eventos propios, no como teclas.
            let mut clip_events: Vec<(u8, Option<String>)> = Vec::new();
            i.events.retain(|e| match e {
                Event::Copy => {
                    clip_events.push((0, None));
                    false
                }
                Event::Cut => {
                    clip_events.push((1, None));
                    false
                }
                Event::Paste(t) => {
                    clip_events.push((2, Some(t.clone())));
                    false
                }
                _ => true,
            });
            for (kind, text) in clip_events {
                let ctx2 = ctx.clone();
                actions.push(Box::new(move |a: &mut App| match kind {
                    0 => a.copy_selection(&ctx2, false),
                    1 => a.copy_selection(&ctx2, true),
                    _ => a.paste(text),
                }));
            }
            on!(shift, Key::G, |a: &mut App| {
                a.view.apply_scrolls = !a.view.apply_scrolls;
                if a.view.apply_scrolls {
                    a.view.time_based = false;
                }
            });

            // Teclas de columna: 1..9, 0.
            const COL_KEYS: [Key; 10] = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9, Key::Num0];
            let mut presses: Vec<(usize, Modifiers)> = Vec::new();
            let mut releases: Vec<usize> = Vec::new();
            i.events.retain(|e| match e {
                Event::Key { key, pressed, repeat, modifiers, .. } => {
                    if let Some(col) = COL_KEYS.iter().position(|k| k == key) {
                        if *pressed && !*repeat && !modifiers.command {
                            presses.push((col, *modifiers));
                        } else if !*pressed {
                            releases.push(col);
                        }
                        false
                    } else {
                        true
                    }
                }
                _ => true,
            });
            for (col, m) in presses {
                actions.push(Box::new(move |a: &mut App| {
                    if m.alt {
                        a.toggle_note(col, NoteKind::Mine, false);
                    } else {
                        a.toggle_note(col, NoteKind::Tap, m.shift);
                    }
                }));
            }
            for col in releases {
                actions.push(Box::new(move |a: &mut App| {
                    a.placing.remove(&col);
                }));
            }
            let _ = alt;
        });
        for f in actions {
            f(self);
        }
    }

    fn handle_field_mouse(&mut self, ui: &egui::Ui, response: &egui::Response, layout: &Layout) {
        if response.hovered() {
            let mut steps = 0i32;
            let mut zoom_steps = 0i32;
            let mut scale_steps = 0i32;
            ui.input(|i| {
                for e in &i.events {
                    if let Event::MouseWheel { unit, delta, modifiers } = e {
                        let d = match unit {
                            egui::MouseWheelUnit::Point => delta.y / 50.0,
                            egui::MouseWheelUnit::Line => delta.y,
                            egui::MouseWheelUnit::Page => delta.y * 4.0,
                        };
                        self.wheel_accum += d;
                        let whole = self.wheel_accum.trunc() as i32;
                        self.wheel_accum -= whole as f32;
                        if modifiers.command && modifiers.alt {
                            scale_steps += whole;
                        } else if modifiers.command {
                            zoom_steps += whole;
                        } else {
                            steps += whole;
                        }
                    }
                }
            });
            if zoom_steps != 0 {
                self.zoom(1.2f32.powi(zoom_steps));
            }
            if scale_steps != 0 {
                self.view.note_scale = (self.view.note_scale * 1.1f32.powi(scale_steps)).clamp(0.3, 2.5);
            }
            if steps != 0 {
                let dir = if self.view.reverse { 1 } else { -1 };
                self.move_snap(steps * dir);
            }
        }
        // Arrastrar: selección rectangular (filas × columnas).
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                let beat = layout.beat_at_y(&self.table, pos.y);
                self.drag_start = Some((beat, layout.col_at_x_clamped(pos.x)));
                self.sel_anchor = None;
            }
        }
        if let (Some((b0, c0)), true) = (self.drag_start, response.dragged_by(egui::PointerButton::Primary)) {
            if let Some(pos) = response.interact_pointer_pos() {
                let b1 = layout.beat_at_y(&self.table, pos.y);
                let c1 = layout.col_at_x_clamped(pos.x);
                let (lo, hi) = (b0.min(b1), b0.max(b1));
                let r0 = (lo * ROWS_PER_BEAT as f64 - 1e-6).ceil() as i32;
                let r1 = (hi * ROWS_PER_BEAT as f64 + 1e-6).floor() as i32;
                self.selection = if r1 >= r0 { Some(Selection::new(r0, r1, c0, c1)) } else { None };
            }
        }
        if response.drag_stopped() {
            self.drag_start = None;
        }

        // Click sobre el notefield: mover el cursor (con Shift, seleccionar desde el
        // cursor hasta ahí). Un click en el fondo, fuera de las columnas, sólo quita la selección.
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let margin = 12.0;
                if pos.x < layout.left - margin || pos.x > layout.right() + margin {
                    self.clear_selection();
                    return;
                }
                let beat = layout.beat_at_y(&self.table, pos.y);
                let step = self.snap_rows();
                let row = ((beat * ROWS_PER_BEAT as f64) / step as f64).round() as i32 * step;
                if ui.input(|i| i.modifiers.shift) {
                    let cur = self.cursor_row();
                    self.selection = Some(Selection::rows(cur, row, self.num_cols()));
                    self.sel_anchor = None;
                } else {
                    self.clear_selection();
                }
                self.jump_to_row(row);
            }
        }
    }

    // ------------------------------------------------------------------
    // Interfaz
    // ------------------------------------------------------------------

    fn menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Archivo", |ui| {
                    if ui.add(egui::Button::new("Abrir...").shortcut_text("Ctrl+O")).clicked() {
                        ui.close_menu();
                        self.request(Pending::OpenDialog);
                    }
                    if ui.button("Nuevo simfile desde audio...").on_hover_text("Elige un .ogg/.mp3/.wav y se crea un .ssc junto a él").clicked() {
                        ui.close_menu();
                        self.request(Pending::NewFromAudio);
                    }
                    ui.menu_button("Recientes", |ui| {
                        if self.recent.is_empty() {
                            ui.label("(vacío)");
                        }
                        let mut open = None;
                        for p in &self.recent {
                            let name = p.parent().and_then(|d| d.file_name()).and_then(|n| n.to_str()).unwrap_or("");
                            let file = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                            if ui.button(format!("{}  —  {}", name, file)).on_hover_text(p.display().to_string()).clicked() {
                                open = Some(p.clone());
                            }
                        }
                        if let Some(p) = open {
                            ui.close_menu();
                            self.request(Pending::Open(p));
                        }
                    });
                    ui.separator();
                    let has = self.song.is_some();
                    if ui.add_enabled(has, egui::Button::new("Guardar").shortcut_text("Ctrl+S")).clicked() {
                        ui.close_menu();
                        self.save(false);
                    }
                    if ui.add_enabled(has, egui::Button::new("Guardar como...").shortcut_text("Ctrl+Shift+S")).clicked() {
                        ui.close_menu();
                        self.save(true);
                    }
                    if ui.add_enabled(has, egui::Button::new("Propiedades de la canción...").shortcut_text("Shift+P")).clicked() {
                        ui.close_menu();
                        self.show_song_props = true;
                    }
                    if ui.add_enabled(has, egui::Button::new("Cerrar")).clicked() {
                        ui.close_menu();
                        self.request(Pending::Close);
                    }
                    ui.separator();
                    if ui.button("Salir").clicked() {
                        ui.close_menu();
                        self.request(Pending::Quit);
                    }
                });
                ui.menu_button("Editar", |ui| {
                    if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("Deshacer").shortcut_text("Ctrl+Z")).clicked() {
                        ui.close_menu();
                        self.undo_redo(true);
                    }
                    if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("Rehacer").shortcut_text("Ctrl+Y")).clicked() {
                        ui.close_menu();
                        self.undo_redo(false);
                    }
                    ui.separator();
                    let has_sel = self.selection.is_some();
                    if ui.add_enabled(has_sel, egui::Button::new("Cortar").shortcut_text("Ctrl+X")).clicked() {
                        ui.close_menu();
                        self.copy_selection(ctx, true);
                    }
                    if ui.add_enabled(has_sel, egui::Button::new("Copiar").shortcut_text("Ctrl+C")).clicked() {
                        ui.close_menu();
                        self.copy_selection(ctx, false);
                    }
                    if ui.add_enabled(self.clipboard.is_some(), egui::Button::new("Pegar en el cursor").shortcut_text("Ctrl+V")).clicked() {
                        ui.close_menu();
                        self.paste(None);
                    }
                    if ui.add_enabled(has_sel, egui::Button::new("Borrar selección").shortcut_text("Supr")).clicked() {
                        ui.close_menu();
                        self.delete_selection();
                    }
                    ui.separator();
                    if ui.add(egui::Button::new("Seleccionar región").shortcut_text("Tab")).clicked() {
                        ui.close_menu();
                        self.region_key();
                    }
                    if ui.add(egui::Button::new("Seleccionar todo").shortcut_text("Ctrl+A")).clicked() {
                        ui.close_menu();
                        self.select_all();
                    }
                    if ui.add(egui::Button::new("Seleccionar antes del cursor").shortcut_text("Shift+Inicio")).clicked() {
                        ui.close_menu();
                        self.select_from_cursor(true);
                    }
                    if ui.add(egui::Button::new("Seleccionar después del cursor").shortcut_text("Shift+Fin")).clicked() {
                        ui.close_menu();
                        self.select_from_cursor(false);
                    }
                    ui.menu_button("Seleccionar por tipo", |ui| {
                        ui.weak(if has_sel { "Dentro de la región actual" } else { "En todo el chart" });
                        let mut pick: Option<u8> = None;
                        for (i, label) in ["Pasos", "Saltos", "Minas", "Holds", "Rolls", "Lifts", "Fakes", "Notas en warps / fakes", "Notas especiales (StepF2)"].iter().enumerate() {
                            if ui.button(*label).clicked() {
                                pick = Some(i as u8);
                            }
                        }
                        ui.menu_button("Por cuantización", |ui| {
                            for q in [4, 8, 12, 16, 24, 32, 48, 64, 192] {
                                if ui.button(RichText::new(edit::quant_name(q)).color(noteskin::quant_color(q))).clicked() {
                                    pick = Some(100 + (q.min(155)) as u8);
                                }
                            }
                        });
                        let players = self.chart().map(|c| c.players).unwrap_or(1);
                        if players > 1 {
                            ui.menu_button("Por jugador", |ui| {
                                for p in 1..=players {
                                    if ui.button(RichText::new(format!("P{}", p)).color(noteskin::player_color(p))).clicked() {
                                        pick = Some(50 + p);
                                    }
                                }
                            });
                        }
                        if let Some(code) = pick {
                            ui.close_menu();
                            self.select_by_code(code);
                        }
                    });
                    if ui.add_enabled(has_sel, egui::Button::new("Quitar selección").shortcut_text("Esc")).clicked() {
                        ui.close_menu();
                        self.clear_selection();
                    }
                    ui.separator();
                    if ui.add_enabled(has_sel, egui::Button::new("Espejo horizontal").shortcut_text("Y")).clicked() {
                        ui.close_menu();
                        self.mirror_selection(true, false);
                    }
                    if ui.add_enabled(has_sel, egui::Button::new("Espejo vertical").shortcut_text("T")).clicked() {
                        ui.close_menu();
                        self.mirror_selection(false, true);
                    }
                    if ui.add_enabled(has_sel, egui::Button::new("Espejo completo").shortcut_text("G")).clicked() {
                        ui.close_menu();
                        self.mirror_selection(true, true);
                    }
                    let num_cols = self.num_cols();
                    if ui
                        .add_enabled(has_sel && num_cols >= 6, egui::Button::new("Cambiar de lado (izquierda ↔ derecha)"))
                        .on_hover_text("Pasa las notas a la otra mitad del pad sin espejarlas (en double, del pad de P1 al de P2)")
                        .clicked()
                    {
                        ui.close_menu();
                        self.switch_side();
                    }
                    ui.separator();
                    let mut conv: Option<Convert> = None;
                    let mut scale: Option<(i32, i32)> = None;
                    ui.add_enabled_ui(has_sel, |ui| {
                        ui.menu_button("Convertir", |ui| {
                            for (i, (a, b)) in Convert::TYPES.into_iter().enumerate() {
                                if i == 3 || i == 6 {
                                    ui.separator();
                                }
                                let c = Convert::Type(a, b);
                                if ui.button(c.label()).clicked() {
                                    conv = Some(c);
                                }
                            }
                            ui.separator();
                            for c in [Convert::HoldsToSteps, Convert::HoldsToMines] {
                                if ui.button(c.label()).clicked() {
                                    conv = Some(c);
                                }
                            }
                            if ui.add(egui::Button::new(Convert::HoldsRolls.label()).shortcut_text("º / `")).clicked() {
                                conv = Some(Convert::HoldsRolls);
                            }
                        });
                        ui.menu_button("Escalar", |ui| {
                            ui.weak("Respecto a la primera nota de la selección");
                            for (num, den, label) in [
                                (2, 1, "2:1  expandir al doble"),
                                (3, 2, "3:2  expandir ×1.5"),
                                (4, 3, "4:3  expandir ×1.33"),
                                (1, 2, "1:2  comprimir a la mitad"),
                                (2, 3, "2:3  comprimir ×0.67"),
                                (3, 4, "3:4  comprimir ×0.75"),
                            ] {
                                if num == 1 {
                                    ui.separator();
                                }
                                if ui.button(label).clicked() {
                                    scale = Some((num, den));
                                }
                            }
                        });
                    });
                    if let Some(c) = conv {
                        ui.close_menu();
                        self.convert_selection(c);
                    }
                    if let Some((num, den)) = scale {
                        ui.close_menu();
                        self.scale_selection(num, den);
                    }
                    if self.chart().map(|c| c.players).unwrap_or(1) > 1 {
                        if ui.add_enabled(has_sel, egui::Button::new("Pasar la selección al jugador siguiente").shortcut_text("/")).clicked() {
                            ui.close_menu();
                            self.slash_key();
                        }
                    }
                });
                ui.menu_button("Tempo", |ui| {
                    if ui.add(egui::Button::new("Editor de tempo y sincronía").shortcut_text("Shift+T")).clicked() {
                        ui.close_menu();
                        self.show_tempo = true;
                    }
                });
                ui.menu_button("Chart", |ui| {
                    if ui.add(egui::Button::new("Anterior").shortcut_text("F5")).clicked() {
                        self.select_chart(self.chart_idx.saturating_sub(1));
                    }
                    if ui.add(egui::Button::new("Siguiente").shortcut_text("F6")).clicked() {
                        self.select_chart(self.chart_idx + 1);
                    }
                    ui.separator();
                    let has_song = self.song.is_some();
                    let has_chart = self.chart().is_some();
                    if ui.add_enabled(has_song, egui::Button::new("Nuevo chart...").shortcut_text("Shift+N")).clicked() {
                        ui.close_menu();
                        self.open_new_chart();
                    }
                    if ui.add_enabled(has_chart, egui::Button::new("Propiedades del chart...").shortcut_text("Shift+C")).clicked() {
                        ui.close_menu();
                        self.show_chart_props = true;
                    }
                    if ui.add_enabled(has_chart, egui::Button::new("Borrar chart...").shortcut_text("Ctrl+Supr")).clicked() {
                        ui.close_menu();
                        self.ask_delete_chart();
                    }
                    ui.separator();
                    let mut select = None;
                    if let Some(song) = &self.song {
                        for (i, c) in song.charts.iter().enumerate() {
                            if ui.selectable_label(i == self.chart_idx, RichText::new(c.display_name()).color(style_color(c))).clicked() {
                                select = Some(i);
                            }
                        }
                    }
                    if let Some(i) = select {
                        ui.close_menu();
                        self.select_chart(i);
                    }
                });
                ui.menu_button("Vista", |ui| {
                    ui.checkbox(&mut self.view.time_based, "Vista basada en tiempo (Shift+V)");
                    let mut game = self.view.apply_scrolls && !self.view.time_based;
                    if ui
                        .checkbox(&mut game, "Vista de juego: #SCROLLS y #SPEEDS como en XSanity (Shift+G)")
                        .on_hover_text("Vista por filas con los scrolls aplicados y el zoom de los speeds según la posición del cursor. Al reproducir, los stops congelan la vista y los warps saltan, igual que en el juego.")
                        .changed()
                    {
                        self.view.apply_scrolls = game;
                        if game {
                            self.view.time_based = false;
                        }
                    }
                    ui.add_enabled_ui(game, |ui| {
                        ui.indent("speeds", |ui| ui.checkbox(&mut self.view.apply_speeds, "Aplicar #SPEEDS"));
                    });
                    ui.checkbox(&mut self.view.show_gimmick_zones, "Mostrar zonas de gimmicks (warps, fakes, stops, speeds, scrolls)");
                    ui.checkbox(&mut self.view.reverse, "Reverse scroll (Shift+R)");
                    ui.checkbox(&mut self.view.show_waveform, "Mostrar waveform");
                    ui.checkbox(&mut self.view.show_beat_lines, "Mostrar líneas de beat");
                    ui.checkbox(&mut self.view.show_tempo, "Mostrar cambios de tempo");
                    ui.checkbox(&mut self.view.show_background, "Mostrar fondo");
                    ui.checkbox(&mut self.view.color_players, "Color por jugador (co-op)");
                    ui.menu_button("Noteskin", |ui| {
                        let mut choose: Option<(Option<String>, bool)> = None;
                        let builtin = !self.view.vector_skin && self.view.noteskin.is_empty();
                        if ui.radio(builtin, "ArrowVortex pump (integrado)").clicked() {
                            choose = Some((None, false));
                        }
                        if ui.radio(self.view.vector_skin, "Vectorial").clicked() {
                            choose = Some((None, true));
                        }
                        ui.separator();
                        if ui.button("Importar noteskins de un simulador...").clicked() {
                            ui.close_menu();
                            self.open_import_skins();
                        }
                        if self.skin_list.is_empty() {
                            ui.weak("Sólo está el noteskin integrado: importa los de tu simulador (XSanity, StepF2, StepMania…).");
                        }
                        for d in &self.skin_dirs {
                            let n = self.skin_list.iter().filter(|(_, sd)| sd == d).count();
                            ui.weak(format!("{} ({}): {}", self.skin_dir_label(d), n, d.display()));
                        }
                        let first_dir = self.skin_dirs.first().cloned();
                        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                            for (name, dir) in &self.skin_list {
                                let on = !self.view.vector_skin && self.view.noteskin == *name;
                                let label = if Some(dir) != first_dir.as_ref() { format!("{}  ({})", name, self.skin_dir_label(dir)) } else { name.clone() };
                                if ui.radio(on, label).clicked() {
                                    choose = Some((Some(name.clone()), false));
                                }
                            }
                        });
                        if ui.button("Leer noteskins de otra carpeta (sin copiarlos)...").clicked() {
                            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                                self.view.skins_dir = dir.to_string_lossy().to_string();
                                let path = self.song.as_ref().and_then(|s| s.path.clone());
                                let dirs = self.compute_skin_dirs(path.as_deref());
                                self.set_skin_dirs(dirs);
                            }
                        }
                        ui.separator();
                        if ui.checkbox(&mut self.view.coop_skins, "Co-op: noteskin de cada jugador (#PRELOADNOTESKIN)").changed() {
                            self.refresh_player_skins();
                        }
                        ui.checkbox(&mut self.view.beat_anim, "Animar notas, holds y receptores con el beat");
                        if let Some((name, vector)) = choose {
                            ui.close_menu();
                            self.choose_skin(name, vector);
                        }
                    });
                    ui.checkbox(&mut self.show_side, "Panel lateral");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.view.show_minimap, "Minimapa (Shift+M)");
                        ui.add_enabled_ui(self.view.show_minimap, |ui| {
                            ui.radio_value(&mut self.view.minimap_density, false, "notas");
                            ui.radio_value(&mut self.view.minimap_density, true, "densidad");
                        });
                    });
                    ui.add_enabled_ui(self.view.show_minimap, |ui| {
                        let mode = if self.view.minimap_density { "densidad" } else { "notas" };
                        ui.weak(format!("Minimapa en modo {} (cada modo tiene su propio zoom):", mode));
                        ui.checkbox(self.view.minimap_fit_mut(), "Todo el chart en pantalla (sin desplazamiento)");
                        let fit = self.view.minimap_fit_active();
                        ui.add_enabled(
                            !fit,
                            egui::Slider::new(self.view.minimap_scale_mut(), 1.0..=150.0).logarithmic(true).text("Escala (Ctrl+rueda)"),
                        );
                    });
                    ui.separator();
                    ui.add(egui::Slider::new(&mut self.view.zoom, 10.0..=3000.0).logarithmic(true).text("Zoom"));
                    ui.add(egui::Slider::new(&mut self.view.note_scale, 0.3..=2.5).text("Tamaño de notas"));
                    ui.add(egui::Slider::new(&mut self.view.receptor_offset, 20.0..=600.0).text("Posición receptores"));
                    ui.add(egui::Slider::new(&mut self.view.background_alpha, 0.0..=1.0).text("Opacidad del fondo"));
                });
                ui.menu_button("Audio", |ui| {
                    if ui.add(egui::Button::new(if self.audio.is_playing() { "Pausa" } else { "Reproducir" }).shortcut_text("Espacio")).clicked() {
                        ui.close_menu();
                        self.toggle_play();
                    }
                    let (mut b, mut n) = self.audio.ticks_enabled();
                    let (b0, n0) = (b, n);
                    ui.checkbox(&mut b, "Tick de beat (F3)");
                    ui.checkbox(&mut n, "Tick de notas (F4)");
                    if (b, n) != (b0, n0) {
                        self.audio.set_ticks_enabled(b, n);
                    }
                    let mut vol = self.audio.volume() * 100.0;
                    if ui.add(egui::Slider::new(&mut vol, 0.0..=100.0).text("Volumen %")).changed() {
                        self.audio.set_volume(vol / 100.0);
                    }
                    let mut speed = self.audio.speed() * 100.0;
                    if ui.add(egui::Slider::new(&mut speed, 10.0..=300.0).text("Velocidad %")).changed() {
                        self.audio.set_speed(speed / 100.0);
                    }
                });
                ui.menu_button("Ayuda", |ui| {
                    if ui.add(egui::Button::new("Atajos de teclado").shortcut_text("F1")).clicked() {
                        ui.close_menu();
                        self.show_shortcuts = true;
                    }
                });
            });
        });
    }

    fn side_panel(&mut self, ctx: &egui::Context) {
        if !self.show_side {
            return;
        }
        egui::SidePanel::left("side").resizable(true).default_width(310.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let Some(song) = self.song.as_ref() else {
                    ui.add_space(8.0);
                    ui.heading(crate::APP_NAME);
                    ui.label("Abre un archivo .ssc o .sm con Ctrl+O, desde Archivo → Recientes, o arrastrándolo a la ventana.");
                    return;
                };
                ui.heading(song.title());
                let sub = song.get_or("SUBTITLE");
                if !sub.is_empty() {
                    ui.label(sub);
                }
                egui::Grid::new("song").num_columns(2).spacing([8.0, 2.0]).show(ui, |ui| {
                    ui.label("Artista");
                    ui.label(song.artist());
                    ui.end_row();
                    let credit = song.get_or("CREDIT");
                    if !credit.is_empty() {
                        ui.label("Crédito");
                        ui.label(credit);
                        ui.end_row();
                    }
                    ui.label("Audio");
                    ui.label(format!("{}  {}", song.get_or("MUSIC"), self.audio_status));
                    ui.end_row();
                    ui.label("Offset");
                    ui.label(format!("{:.3} s", song.timing.offset));
                    ui.end_row();
                    let (lo, hi) = song.timing.bpm_range();
                    ui.label("BPM");
                    ui.label(if (hi - lo).abs() < 1e-3 { format!("{:.3}", lo) } else { format!("{:.3} – {:.3}", lo, hi) });
                    ui.end_row();
                    ui.label("Formato");
                    ui.label(match song.format {
                        SimFormat::Ssc => format!("SSC {}", song.get_or("VERSION")),
                        SimFormat::Sm => "SM (se guardará como .ssc)".to_string(),
                    });
                    ui.end_row();
                });

                let mut open_song_props = false;
                let mut open_new = false;
                let mut open_chart_props = false;
                let mut ask_delete = false;
                if ui.small_button("Editar propiedades (Shift+P)").clicked() {
                    open_song_props = true;
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.strong(format!("Charts ({})", song.charts.len()));
                    if ui.small_button("+ Nuevo").on_hover_text("Nuevo chart (Shift+N)").clicked() {
                        open_new = true;
                    }
                });
                if song.charts.is_empty() {
                    ui.weak("Sin charts todavía. Crea uno con «+ Nuevo».");
                }
                let mut select = None;
                let mut quick_select: Option<u8> = None;
                for (i, c) in song.charts.iter().enumerate() {
                    let mut text = format!("{:<9} {}", c.short_name(), c.difficulty());
                    let desc = c.description();
                    if !desc.is_empty() {
                        text.push_str(&format!("  {}", desc));
                    }
                    if c.timing.is_some() {
                        text.push_str("  ⏱");
                    }
                    let resp = ui.selectable_label(i == self.chart_idx, RichText::new(text).monospace().color(style_color(c)));
                    let resp = resp.on_hover_text(format!(
                        "{}\n{}{}",
                        c.style.name,
                        c.display_name(),
                        if c.timing.is_some() { "\nTiene timing propio (split timing)" } else { "" }
                    ));
                    if resp.clicked() {
                        select = Some(i);
                    }
                }

                if let Some(c) = song.charts.get(self.chart_idx) {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.strong("Chart actual");
                        if ui.small_button("Propiedades").on_hover_text("Shift+C").clicked() {
                            open_chart_props = true;
                        }
                        if ui.small_button("Borrar").on_hover_text("Ctrl+Supr").clicked() {
                            ask_delete = true;
                        }
                    });
                    let counts = c.counts();
                    egui::Grid::new("chart").num_columns(2).spacing([8.0, 2.0]).show(ui, |ui| {
                        ui.label("Tipo");
                        ui.label(format!("{} ({} col.)", c.steps_type(), c.num_cols));
                        ui.end_row();
                        ui.label("Nivel");
                        ui.label(format!("{} {}", c.difficulty(), c.meter()));
                        ui.end_row();
                        let credit = c.credit();
                        if !credit.is_empty() {
                            ui.label("Autor");
                            ui.label(credit);
                            ui.end_row();
                        }
                        if c.players > 1 {
                            ui.label("Jugadores");
                            ui.label(format!("{}{}", c.players, if c.separated { " (bloques &)" } else { "" }));
                            ui.end_row();
                        }
                        ui.label("Timing");
                        ui.label(if c.timing.is_some() { "propio del chart" } else { "de la canción" });
                        ui.end_row();
                        ui.label("Pasos");
                        ui.label(format!("{}  (saltos {})", counts.steps, counts.jumps));
                        ui.end_row();
                        ui.label("Holds / Rolls");
                        ui.label(format!("{} / {}", counts.holds, counts.rolls));
                        ui.end_row();
                        ui.label("Minas / Fakes");
                        ui.label(format!("{} / {}", counts.mines, counts.fakes));
                        ui.end_row();
                        if counts.other > 0 || counts.lifts > 0 {
                            ui.label("Lifts / Especiales");
                            ui.label(format!("{} / {}", counts.lifts, counts.other));
                            ui.end_row();
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.small("Seleccionar:");
                        for (code, label) in [(0u8, "pasos"), (1, "saltos"), (2, "minas"), (3, "holds"), (4, "rolls"), (6, "fakes")] {
                            if ui.small_button(label).clicked() {
                                quick_select = Some(code);
                            }
                        }
                    });
                    let st = &c.stats;
                    if st.orphan_tails + st.unclosed_holds + st.collisions > 0 {
                        ui.colored_label(
                            Color32::from_rgb(240, 180, 80),
                            format!(
                                "Avisos al leer: {} colas sin hold, {} holds sin cola, {} notas superpuestas",
                                st.orphan_tails, st.unclosed_holds, st.collisions
                            ),
                        );
                    }
                    if c.players > 1 {
                        ui.separator();
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Notas nuevas para:");
                            for p in 1..=c.players {
                                let txt = RichText::new(format!("P{}", p)).color(noteskin::player_color(p)).strong();
                                if ui.selectable_label(self.current_player == p, txt).clicked() {
                                    self.current_player = p;
                                }
                            }
                        });
                        ui.small("Tecla / para cambiar de jugador.");
                    }
                }
                if let Some(i) = select {
                    self.select_chart(i);
                }
                if let Some(code) = quick_select {
                    self.select_by_code(code);
                }
                if open_song_props {
                    self.show_song_props = true;
                }
                if open_new {
                    self.open_new_chart();
                }
                if open_chart_props {
                    self.show_chart_props = true;
                }
                if ask_delete {
                    self.ask_delete_chart();
                }
            });
        });
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let sep = |ui: &mut egui::Ui| {
                    ui.separator();
                };
                if let Some(c) = self.chart() {
                    ui.label(RichText::new(c.display_name()).color(style_color(c)));
                    sep(ui);
                }
                let q = self.snap();
                ui.label(RichText::new(format!("Snap 1/{}", q)).color(noteskin::quant_color(q)));
                sep(ui);
                if let Some(td) = self.timing() {
                    let row = self.cursor_row();
                    ui.label(format!("BPM {:.3}", td.bpm_at_row(row)));
                    sep(ui);
                    let measure = row.div_euclid(ROWS_PER_MEASURE);
                    let beat_in = (self.cursor_beat - measure as f64 * 4.0).max(0.0);
                    ui.label(format!("Compás {} · beat {:.3}", measure, beat_in));
                    sep(ui);
                    ui.label(format!("Fila {}", row));
                    sep(ui);
                }
                ui.label(format!("Tiempo {}", fmt_time(self.cursor_time)));
                sep(ui);
                if self.view.time_based {
                    ui.label("Vista: tiempo");
                } else if self.view.apply_scrolls {
                    ui.label("Vista: juego");
                    let scroll = self.scroll_map.ratio_at(self.cursor_beat);
                    let speed = if self.view.apply_speeds { self.scroll_map.speed_at(self.cursor_beat, self.cursor_time) } else { 1.0 };
                    let color = |v: f64| if (v - 1.0).abs() < 1e-6 { Color32::from_gray(170) } else { Color32::from_rgb(110, 220, 240) };
                    if self.view.apply_speeds {
                        ui.label(RichText::new(format!("Speed {}x", fmt_ratio(speed))).color(color(speed)));
                    }
                    ui.label(RichText::new(format!("Scroll {}x", fmt_ratio(scroll))).color(color(scroll)));
                } else {
                    ui.label("Vista: filas");
                }
                sep(ui);
                ui.label(format!("Vol {:.0}%  Vel {:.0}%", self.audio.volume() * 100.0, self.audio.speed() * 100.0));
                let (b, n) = self.audio.ticks_enabled();
                if b || n {
                    sep(ui);
                    ui.label(format!("Ticks:{}{}", if b { " beat" } else { "" }, if n { " notas" } else { "" }));
                }
                if let Some(sel) = &self.selection {
                    sep(ui);
                    let n = self.chart().map(|c| edit::count_selected(&c.notes, sel)).unwrap_or(0);
                    let what = sel.filter.as_ref().map(|f| f.name()).unwrap_or_else(|| "notas".into());
                    let txt = format!(
                        "Selección: {} {} (beats {:.2}–{:.2}){}",
                        n,
                        what,
                        row_to_beat(sel.r0),
                        row_to_beat(sel.r1),
                        if self.sel_anchor.is_some() { " · Tab para cerrar" } else { "" }
                    );
                    ui.colored_label(Color32::from_rgb(130, 190, 255), txt);
                }
                if self.dirty {
                    sep(ui);
                    ui.colored_label(Color32::from_rgb(240, 180, 80), "● sin guardar");
                }
            });
        });
    }

    /// Minimapa a la derecha: todo el chart, recuadro de la vista y navegación con el mouse.
    fn minimap_panel(&mut self, ctx: &egui::Context) {
        if !self.view.show_minimap || self.song.is_none() {
            return;
        }
        egui::SidePanel::right("minimap")
            .resizable(true)
            .default_width(110.0)
            .width_range(60.0..=280.0)
            .frame(egui::Frame::default().fill(Color32::from_rgb(14, 14, 18)).inner_margin(egui::Margin::same(2)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.view.minimap_density, false, RichText::new("Notas").small());
                    ui.selectable_value(&mut self.view.minimap_density, true, RichText::new("Densidad").small());
                });
                let rect = ui.available_rect_before_wrap();
                let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
                let time_axis = self.view.time_based;

                // Dibujo (sólo lecturas) y datos para la interacción.
                let info = {
                    let Some(song) = self.song.as_ref() else { return };
                    let Some(chart) = song.charts.get(self.chart_idx) else { return };
                    let table = &self.table;
                    let music_end = self.music.as_ref().map(|m| m.duration());
                    let last_row = chart.last_row();
                    let range = if time_axis {
                        let end = table.time_at_row(last_row).max(music_end.unwrap_or(0.0));
                        (table.time_at_beat(0.0), end + 1.0)
                    } else {
                        let end = row_to_beat(last_row).max(music_end.map(|t| table.beat_at_time(t)).unwrap_or(0.0));
                        (0.0, end + 4.0)
                    };
                    let pump = chart.style.game == Game::Pump;
                    // Parte visible del notefield con el cursor actual.
                    let view = self.layout.as_ref().map(|l| {
                        let lay = Layout::new(l.rect, chart.num_cols.max(1), &self.view, pump, self.cursor_time, self.cursor_beat, Some(self.scroll_map.clone()));
                        if time_axis {
                            let (a, b) = (lay.time_at_y(l.rect.top()), lay.time_at_y(l.rect.bottom()));
                            (a.min(b), a.max(b))
                        } else {
                            lay.visible_beats(table)
                        }
                    });
                    let cursor = if time_axis { self.cursor_time } else { self.cursor_beat };
                    let input = MinimapInput {
                        chart,
                        table,
                        time_axis,
                        reverse: self.view.reverse,
                        range,
                        view,
                        cursor,
                        selection: self.selection.as_ref().map(|s| (s.r0, s.r1)),
                        density: self.view.minimap_density,
                        scale: if self.view.minimap_fit_active() {
                            None
                        } else {
                            // En la vista por filas: 1 beat = medio segundo a 120 BPM.
                            let s = self.view.minimap_scale_active() as f64;
                            Some(if time_axis { s } else { s * 0.5 })
                        },
                    };
                    let map = minimap::draw(&ui.painter_at(rect), rect, &input);
                    (map, view, cursor)
                };
                let (map, view, cursor) = info;

                // Click o arrastre: centrar la vista en el mouse.
                if response.is_pointer_button_down_on() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let new_cursor = map.cursor_for_pointer(pos.y, view, cursor);
                        let beat = if time_axis { self.table.beat_at_time(new_cursor) } else { new_cursor };
                        let step = self.snap_rows();
                        let row = ((beat * ROWS_PER_BEAT as f64) / step as f64).round() as i32 * step;
                        if row != self.cursor_row() {
                            self.jump_to_row(row);
                        }
                    }
                }
                // Rueda: de compás en compás. Ctrl+rueda: zoom del minimapa.
                if response.hovered() {
                    let mut steps = 0i32;
                    let mut zoom_steps = 0i32;
                    ui.input(|i| {
                        for e in &i.events {
                            if let Event::MouseWheel { unit, delta, modifiers } = e {
                                let d = match unit {
                                    egui::MouseWheelUnit::Point => delta.y / 50.0,
                                    egui::MouseWheelUnit::Line => delta.y,
                                    egui::MouseWheelUnit::Page => delta.y * 4.0,
                                };
                                self.wheel_accum += d;
                                let whole = self.wheel_accum.trunc() as i32;
                                self.wheel_accum -= whole as f32;
                                if modifiers.command {
                                    zoom_steps += whole;
                                } else {
                                    steps += whole;
                                }
                            }
                        }
                    });
                    if zoom_steps != 0 {
                        // Cada modo (notas / densidad) tiene su propio zoom.
                        *self.view.minimap_fit_mut() = false;
                        let s = self.view.minimap_scale_mut();
                        *s = (*s * 1.2f32.powi(zoom_steps)).clamp(1.0, 150.0);
                    }
                    if steps != 0 {
                        let dir = if self.view.reverse { 1 } else { -1 };
                        for _ in 0..steps.abs() {
                            self.move_rows(ROWS_PER_MEASURE * dir * steps.signum());
                        }
                    }
                }
            });
    }

    fn central(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().frame(egui::Frame::default().fill(Color32::from_rgb(18, 18, 22))).show(ctx, |ui| {
            let rect = ui.max_rect();
            let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
            let painter = ui.painter_at(rect);

            if self.view.show_background {
                if let Some(bg) = &self.background {
                    let a = (self.view.background_alpha.clamp(0.0, 1.0) * 255.0) as u8;
                    let size = bg.size_vec2();
                    // Recorte para llenar (cover).
                    let s = (rect.width() / size.x).max(rect.height() / size.y);
                    let dst = egui::Rect::from_center_size(rect.center(), size * s);
                    painter.image(bg.id(), dst, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::from_white_alpha(a));
                }
            }

            let Some(song) = self.song.as_ref() else {
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, "Arrastra un .ssc aquí o pulsa Ctrl+O", egui::FontId::proportional(20.0), Color32::from_gray(140));
                return;
            };
            let Some(chart) = song.charts.get(self.chart_idx) else {
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, "Esta canción no tiene charts: Chart → Nuevo chart (Shift+N)", egui::FontId::proportional(20.0), Color32::from_gray(140));
                return;
            };
            let input = FieldInput {
                chart,
                timing: song.chart_timing(self.chart_idx),
                table: &self.table,
                cursor_time: self.cursor_time,
                cursor_beat: self.cursor_beat,
                snap: self.snap(),
                view: &self.view,
                skin: self.skin.as_ref(),
                waveform: self.waveform.as_deref(),
                playing: self.audio.is_playing(),
                scroll: Some(self.scroll_map.clone()),
                selection: self.selection.as_ref(),
                ext: self.ext_skin.as_deref(),
                player_skins: &self.player_skins,
            };
            let layout = notefield::draw(&painter, rect, &input);
            self.handle_field_mouse(ui, &response, &layout);
            self.layout = Some(layout);

            // Mensajes temporales.
            let now = Instant::now();
            self.messages.retain(|(t, _)| now.duration_since(*t).as_secs_f32() < 4.0);
            let mut y = rect.top() + 8.0;
            for (_, m) in &self.messages {
                painter.text(egui::pos2(rect.right() - 10.0, y), egui::Align2::RIGHT_TOP, m, egui::FontId::proportional(14.0), Color32::from_rgb(230, 230, 160));
                y += 18.0;
            }
            if !self.messages.is_empty() {
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
            }
        });
    }

    fn shortcuts_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_shortcuts;
        egui::Window::new("Atajos de teclado").open(&mut open).resizable(false).show(ctx, |ui| {
            egui::Grid::new("keys").num_columns(2).striped(true).show(ui, |ui| {
                for (k, d) in [
                    ("Ctrl+O / Ctrl+S / Ctrl+Shift+S", "Abrir / Guardar / Guardar como"),
                    ("Espacio", "Reproducir / pausa"),
                    ("↑ ↓ / rueda del mouse", "Mover el cursor según el snap"),
                    ("← →", "Cambiar snap"),
                    ("Re Pág / Av Pág", "Compás anterior / siguiente"),
                    ("Shift+Re Pág / Shift+Av Pág", "Beat anterior / siguiente"),
                    ("Inicio / Fin", "Inicio / final del chart"),
                    ("F5 / F6", "Chart anterior / siguiente"),
                    ("+ / − , Ctrl+rueda", "Zoom"),
                    ("Ctrl + / Ctrl −, Ctrl+Alt+rueda", "Tamaño de las notas"),
                    ("1..9, 0", "Poner/quitar nota en la columna (mantener + mover = hold)"),
                    ("Shift+1..0", "Igual, pero mantener + mover crea un roll"),
                    ("Alt+1..0", "Poner/quitar mina"),
                    ("/", "Co-op: jugador de las notas nuevas o, con selección, pasarla al siguiente"),
                    ("Ctrl+Z / Ctrl+Y", "Deshacer / rehacer"),
                    ("Tab", "Empezar / cerrar una región en el cursor"),
                    ("Arrastrar con el mouse", "Selección rectangular (filas y columnas)"),
                    ("Shift+click", "Seleccionar desde el cursor hasta ahí"),
                    ("Ctrl+A / Esc", "Seleccionar todo / quitar selección"),
                    ("Shift+Inicio / Shift+Fin", "Seleccionar antes / después del cursor"),
                    ("Ctrl+Inicio / Ctrl+Fin", "Ir al inicio / final de la selección"),
                    ("Ctrl+C / Ctrl+X / Ctrl+V", "Copiar / cortar / pegar en el cursor"),
                    ("Supr", "Borrar la selección"),
                    ("Y / T / G", "Espejo horizontal / vertical / completo"),
                    ("º (tecla bajo Esc)", "Holds ↔ rolls en la selección"),
                    ("Shift+T (o Shift+S)", "Editor de tempo y sincronía"),
                    ("Shift+P / Shift+C", "Propiedades de la canción / del chart"),
                    ("Shift+N / Ctrl+Supr", "Nuevo chart / borrar chart"),
                    ("F3 / F4", "Tick de beat / tick de notas"),
                    ("Shift+↑ / Shift+↓", "Volumen"),
                    ("Shift+← / Shift+→", "Velocidad de reproducción"),
                    ("Shift+V", "Vista por tiempo / por filas"),
                    ("Shift+G", "Vista de juego: #SCROLLS y #SPEEDS como en XSanity"),
                    ("Shift+R", "Reverse scroll"),
                    ("Click en las columnas del notefield", "Mover el cursor ahí (click en el fondo: quitar selección)"),
                    ("Click / arrastrar en el minimapa", "Ir a esa parte del chart (rueda: de compás en compás)"),
                    ("Ctrl+rueda en el minimapa", "Zoom del minimapa"),
                    ("Shift+M", "Mostrar / ocultar el minimapa"),
                    ("F1", "Esta ventana"),
                ] {
                    ui.monospace(k);
                    ui.label(d);
                    ui.end_row();
                }
            });
        });
        self.show_shortcuts = open;
    }

    fn confirm_window(&mut self, ctx: &egui::Context) {
        let Some(p) = self.confirm.clone() else { return };
        egui::Window::new("Cambios sin guardar")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("Hay cambios sin guardar. ¿Qué quieres hacer?");
                ui.horizontal(|ui| {
                    if ui.button("Guardar").clicked() {
                        self.save(false);
                        self.confirm = None;
                        if !self.dirty {
                            self.pending = Some(p.clone());
                        }
                    }
                    if ui.button("Descartar").clicked() {
                        self.dirty = false;
                        self.confirm = None;
                        self.pending = Some(p.clone());
                    }
                    if ui.button("Cancelar").clicked() {
                        self.confirm = None;
                    }
                });
            });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_loaders(ctx);

        // Archivos arrastrados a la ventana.
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if let Some(p) = dropped.into_iter().find(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| matches!(e.to_ascii_lowercase().as_str(), "ssc" | "sm" | "ogg" | "mp3" | "wav" | "flac"))
                .unwrap_or(false)
        }) {
            self.request(Pending::Open(p));
        }

        // Cierre con cambios sin guardar.
        if ctx.input(|i| i.viewport().close_requested()) && self.dirty && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Pending::Quit);
        }

        if self.confirm.is_none() {
            self.handle_input(ctx);
        }
        self.run_pending(ctx);

        // Reproducción: el cursor sigue al audio.
        if self.audio.is_playing() {
            let t = self.audio.time();
            self.set_cursor_time(t);
            let stop_at = self.preview_stop.unwrap_or(self.play_end);
            if t > stop_at {
                self.audio.stop();
                self.preview_stop = None;
            }
            ctx.request_repaint();
        }

        // Título de la ventana.
        let title = match &self.song {
            Some(s) => format!("{} — {}{}", crate::APP_NAME, s.title(), if self.dirty { " *" } else { "" }),
            None => crate::APP_NAME.to_string(),
        };
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }

        self.menu_bar(ctx);
        self.status_bar(ctx);
        self.side_panel(ctx);
        self.minimap_panel(ctx);
        self.central(ctx);
        self.tempo_window(ctx);
        self.song_props_window(ctx);
        self.chart_props_window(ctx);
        self.new_chart_window(ctx);
        self.delete_chart_window(ctx);
        self.import_skins_window(ctx);
        self.shortcuts_window(ctx);
        self.confirm_window(ctx);
        self.meta_end_frame(ctx);
    }

    /// Tab se intercepta antes de que egui lo use para mover el foco entre widgets
    /// (eso dejaría los atajos sin efecto).
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        if text_input_active(ctx) {
            return;
        }
        let mut presses = 0;
        let mut down = self.tab_down;
        raw.events.retain(|e| match e {
            Event::Key { key: Key::Tab, pressed, .. } => {
                if *pressed {
                    if !down {
                        presses += 1;
                    }
                    down = true;
                } else {
                    down = false;
                }
                false
            }
            _ => true,
        });
        self.tab_down = down;
        self.tab_presses += presses;
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "view", &self.view);
        eframe::set_value(storage, "recent", &self.recent);
        eframe::set_value(storage, "snap", &self.snap_idx);
        eframe::set_value(storage, "volume", &self.audio.volume());
    }
}
