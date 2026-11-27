//! Modelo de simfile (canción + charts) y carga/guardado de .ssc / .sm.

pub mod msd;
pub mod notes;
pub mod sm;
pub mod ssc;
pub mod style;
pub mod timing;

use std::path::{Path, PathBuf};

use notes::{Note, ParseStats};
use style::Style;
use timing::TimingData;

/// Un tag `#NOMBRE:params;` conservado tal cual (para no perder datos desconocidos).
#[derive(Debug, Clone)]
pub struct Tag {
    pub name: String,
    pub params: Vec<String>,
    /// Texto fuente original del valor (con comentarios); se reescribe tal cual
    /// mientras siga representando los mismos `params`.
    pub raw: Option<String>,
}

/// Dos tags son iguales si tienen el mismo nombre y valor (el texto fuente no cuenta).
impl PartialEq for Tag {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.params == other.params
    }
}

impl Tag {
    pub fn new(name: &str, value: &str) -> Self {
        Tag { name: name.to_string(), params: vec![value.to_string()], raw: None }
    }

    pub fn from_msd(v: &msd::MsdValue) -> Self {
        Tag { name: v.tag.trim().to_string(), params: v.params.clone(), raw: Some(v.raw.clone()) }
    }

    /// Texto a escribir: el original si sigue siendo válido, o uno escapado.
    pub fn source_text(&self) -> String {
        if let Some(raw) = &self.raw {
            let probe = msd::parse(&format!("#X:{};", raw));
            if probe.len() == 1 && probe[0].params == self.params {
                return raw.clone();
            }
        }
        msd::escape_params(&self.params)
    }
    pub fn key(&self) -> String {
        self.name.trim().to_ascii_uppercase()
    }
    pub fn value(&self) -> String {
        self.params.join(":")
    }
}

/// Tags de timing que pueden aparecer en la canción o en cada chart (split timing).
pub const TIMING_KEYS: &[&str] = &[
    "OFFSET", "BPMS", "STOPS", "DELAYS", "WARPS", "TIMESIGNATURES", "TICKCOUNTS", "COMBOS", "SPEEDS", "SCROLLS",
    "FAKES", "LABELS",
];

pub fn is_timing_key(key: &str) -> bool {
    TIMING_KEYS.contains(&key) || key == "FREEZES"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimFormat {
    Ssc,
    Sm,
}

fn get_tag(tags: &[Tag], key: &str) -> Option<String> {
    tags.iter().rev().find(|t| t.key() == key).map(|t| t.value())
}

/// Fija un tag con varios parámetros (`#DISPLAYBPM:120:180;`).
fn set_tag_params(tags: &mut Vec<Tag>, key: &str, params: Vec<String>) {
    if let Some(t) = tags.iter_mut().rev().find(|t| t.key() == key) {
        t.params = params;
        t.raw = None;
    } else {
        tags.push(Tag { name: key.to_string(), params, raw: None });
    }
}

fn set_tag(tags: &mut Vec<Tag>, key: &str, value: &str) {
    if let Some(t) = tags.iter_mut().rev().find(|t| t.key() == key) {
        t.params = vec![value.to_string()];
        t.raw = None;
    } else {
        tags.push(Tag::new(key, value));
    }
}

#[derive(Debug, Clone)]
pub struct Chart {
    /// Tags del chart en su orden original (sin NOTEDATA ni NOTES).
    pub tags: Vec<Tag>,
    pub style: Style,
    pub players: u8,
    /// `true` si los bloques `&` separan jugadores.
    pub separated: bool,
    /// Timing propio del chart (split timing) o `None` si usa el de la canción.
    pub timing: Option<TimingData>,
    pub notes: Vec<Note>,
    pub num_cols: usize,
    pub num_blocks: usize,
    /// Texto original de `#NOTES` y las notas tal como se cargaron: si no cambian,
    /// se escribe el texto original sin tocar.
    pub notes_raw: Option<String>,
    pub notes_orig: Vec<Note>,
    pub stats: ParseStats,
}

impl Chart {
    pub fn new(style: Style) -> Self {
        let mut c = Chart {
            tags: Vec::new(),
            style,
            players: style.default_players,
            separated: false,
            timing: None,
            notes: Vec::new(),
            num_cols: style.num_cols(),
            num_blocks: 1,
            notes_raw: None,
            notes_orig: Vec::new(),
            stats: ParseStats::default(),
        };
        // Mismo orden de tags que escribe StepMania 5 / StepF2.
        c.set("CHARTNAME", "");
        c.set("STEPSTYPE", style.id);
        c.set("DESCRIPTION", "");
        c.set("DIFFICULTY", "Edit");
        c.set("METER", "1");
        c.set("RADARVALUES", "0,0,0,0,0");
        c.set("CREDIT", "");
        if style.id.ends_with("-p") {
            c.set("SEPARATENOTES", "NO");
            c.set("PLAYERS", &style.default_players.to_string());
        }
        c.separated = style.id.ends_with("-routine");
        if c.separated {
            c.num_blocks = c.players.max(1) as usize;
        }
        c
    }

    /// Cambia el tipo de chart. Sólo entre estilos con el mismo número de columnas,
    /// para no perder notas. Devuelve `false` si no se puede.
    pub fn set_style(&mut self, style: Style) -> bool {
        if style.num_cols() != self.num_cols {
            return false;
        }
        self.style = style;
        self.set("STEPSTYPE", style.id);
        self.separated = style.id.ends_with("-routine") || self.get_or("SEPARATENOTES").eq_ignore_ascii_case("YES");
        if style.id.ends_with("-p") && self.get("PLAYERS").is_none() {
            self.set("PLAYERS", &self.players.max(1).to_string());
        }
        self.players = self.players.max(style.default_players).max(1);
        if self.separated {
            self.num_blocks = self.num_blocks.max(self.players as usize);
        }
        true
    }

    /// Cambia el número de jugadores (charts co-op).
    pub fn set_players(&mut self, players: u8) {
        let players = players.clamp(1, 9);
        self.players = players;
        self.set("PLAYERS", &players.to_string());
        if self.separated {
            self.num_blocks = self.num_blocks.max(players as usize);
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        get_tag(&self.tags, key)
    }
    pub fn get_or(&self, key: &str) -> String {
        self.get(key).unwrap_or_default().trim().to_string()
    }
    pub fn set(&mut self, key: &str, value: &str) {
        set_tag(&mut self.tags, key, value)
    }

    pub fn steps_type(&self) -> String {
        self.get_or("STEPSTYPE")
    }
    pub fn meter(&self) -> i32 {
        self.get_or("METER").parse::<f64>().map(|v| v as i32).unwrap_or(0)
    }
    pub fn difficulty(&self) -> String {
        self.get_or("DIFFICULTY")
    }
    pub fn description(&self) -> String {
        self.get_or("DESCRIPTION")
    }
    pub fn credit(&self) -> String {
        self.get_or("CREDIT")
    }

    /// Nombre corto estilo Pump: `S23`, `D24`, `DP3 (2P)`...
    pub fn short_name(&self) -> String {
        let mut s = format!("{}{}", self.style.short, self.meter());
        if self.players > 1 {
            s.push_str(&format!(" ({}P)", self.players));
        }
        s
    }

    pub fn display_name(&self) -> String {
        let mut parts = vec![self.short_name(), self.difficulty()];
        let desc = self.description();
        let credit = self.credit();
        if !desc.is_empty() {
            parts.push(desc);
        }
        if !credit.is_empty() && credit != self.description() {
            parts.push(format!("por {}", credit));
        }
        parts.retain(|p| !p.is_empty());
        parts.join(" · ")
    }

    pub fn notes_changed(&self) -> bool {
        !notes::same_notes(&self.notes, &self.notes_orig)
    }

    pub fn last_row(&self) -> i32 {
        self.notes.iter().map(|n| n.end_row.max(n.row)).max().unwrap_or(0)
    }

    /// Cuenta de notas por tipo (taps, holds, rolls, minas, saltos...).
    pub fn counts(&self) -> NoteCounts {
        let mut c = NoteCounts::default();
        let mut rows: std::collections::BTreeMap<(u8, i32), u32> = std::collections::BTreeMap::new();
        for n in &self.notes {
            match n.kind {
                notes::NoteKind::Mine => c.mines += 1,
                notes::NoteKind::Hold => c.holds += 1,
                notes::NoteKind::Roll => c.rolls += 1,
                notes::NoteKind::Lift => c.lifts += 1,
                notes::NoteKind::Tap => c.taps += 1,
                _ => c.other += 1,
            }
            if n.fake {
                c.fakes += 1;
            }
            if !matches!(n.kind, notes::NoteKind::Mine) && !n.fake {
                *rows.entry((n.player, n.row)).or_insert(0) += 1;
            }
        }
        c.steps = rows.len() as u32;
        c.jumps = rows.values().filter(|&&v| v >= 2).count() as u32;
        c
    }
}

#[derive(Debug, Clone, Default)]
pub struct NoteCounts {
    pub steps: u32,
    pub taps: u32,
    pub jumps: u32,
    pub holds: u32,
    pub rolls: u32,
    pub mines: u32,
    pub lifts: u32,
    pub fakes: u32,
    pub other: u32,
}

#[derive(Debug, Clone)]
pub struct Song {
    pub path: Option<PathBuf>,
    pub format: SimFormat,
    /// Tags de cabecera en su orden original.
    pub header: Vec<Tag>,
    pub timing: TimingData,
    pub charts: Vec<Chart>,
    pub warnings: Vec<String>,
}

impl Default for Song {
    fn default() -> Self {
        let mut s = Song {
            path: None,
            format: SimFormat::Ssc,
            header: Vec::new(),
            timing: TimingData::default(),
            charts: Vec::new(),
            warnings: Vec::new(),
        };
        for k in ["VERSION", "TITLE", "SUBTITLE", "ARTIST", "TITLETRANSLIT", "SUBTITLETRANSLIT", "ARTISTTRANSLIT", "GENRE", "CREDIT", "MUSIC", "BANNER", "BACKGROUND", "CDTITLE", "SAMPLESTART", "SAMPLELENGTH", "SELECTABLE", "OFFSET", "BPMS", "STOPS"] {
            s.header.push(Tag::new(k, ""));
        }
        s.set("VERSION", "0.83");
        s.set("SELECTABLE", "YES");
        s.set("OFFSET", "0.000000");
        s.set("BPMS", "0.000000=120.000000");
        s.timing.bpms.push(timing::BpmSeg { row: 0, bpm: 120.0 });
        s
    }
}

impl Song {
    pub fn get(&self, key: &str) -> Option<String> {
        get_tag(&self.header, key)
    }
    pub fn get_or(&self, key: &str) -> String {
        self.get(key).unwrap_or_default().trim().to_string()
    }
    pub fn set(&mut self, key: &str, value: &str) {
        set_tag(&mut self.header, key, value)
    }
    pub fn set_params(&mut self, key: &str, params: Vec<String>) {
        set_tag_params(&mut self.header, key, params)
    }

    /// Simfile nuevo (sin charts) para un audio: `<carpeta del audio>/<nombre>.ssc`.
    pub fn new_for_audio(audio: &Path) -> Song {
        let mut s = Song::default();
        let stem = audio.file_stem().and_then(|n| n.to_str()).unwrap_or("nuevo").to_string();
        let file = audio.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        s.set("TITLE", &stem);
        s.set("MUSIC", &file);
        s.path = audio.parent().map(|d| d.join(format!("{}.ssc", stem)));
        s
    }

    pub fn title(&self) -> String {
        self.get_or("TITLE")
    }
    pub fn artist(&self) -> String {
        self.get_or("ARTIST")
    }

    pub fn dir(&self) -> Option<PathBuf> {
        self.path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf()))
    }

    /// Ruta absoluta del audio (`#MUSIC`), buscando sin distinguir mayúsculas.
    pub fn music_path(&self) -> Option<PathBuf> {
        let music = self.get_or("MUSIC");
        let dir = self.dir()?;
        if !music.is_empty() {
            if let Some(p) = resolve_case_insensitive(&dir, &music) {
                return Some(p);
            }
        }
        // Sin #MUSIC válido: primer archivo de audio de la carpeta.
        let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
            .ok()?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| matches!(e.to_ascii_lowercase().as_str(), "ogg" | "mp3" | "wav" | "flac"))
                    .unwrap_or(false)
            })
            .collect();
        found.sort();
        found.into_iter().next()
    }

    pub fn file_resource(&self, key: &str) -> Option<PathBuf> {
        let v = self.get_or(key);
        if v.is_empty() {
            return None;
        }
        resolve_case_insensitive(&self.dir()?, &v)
    }

    /// Timing efectivo de un chart (el suyo o el de la canción).
    pub fn chart_timing(&self, chart: usize) -> &TimingData {
        self.charts.get(chart).and_then(|c| c.timing.as_ref()).unwrap_or(&self.timing)
    }
}

pub fn resolve_case_insensitive(dir: &Path, rel: &str) -> Option<PathBuf> {
    let direct = dir.join(rel);
    if direct.exists() {
        return Some(direct);
    }
    let want = rel.replace('\\', "/").to_ascii_lowercase();
    let file_name = want.rsplit('/').next().unwrap_or(&want).to_string();
    std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| {
        p.file_name().and_then(|n| n.to_str()).map(|n| n.to_ascii_lowercase() == file_name).unwrap_or(false)
    })
}

/// Lee un archivo de texto tolerando BOM y codificaciones no UTF-8 (Latin-1).
pub fn read_text(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(&bytes);
    Ok(match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    })
}

pub fn load(path: &Path) -> Result<Song, String> {
    let text = read_text(path).map_err(|e| format!("No se pudo leer {}: {}", path.display(), e))?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let mut song = match ext.as_str() {
        "sm" => sm::load_str(&text),
        _ => ssc::load_str(&text),
    };
    song.path = Some(path.to_path_buf());
    Ok(song)
}

/// Guarda como .ssc. La primera vez que se sobrescribe un archivo deja una copia
/// `.bak` del original (no se vuelve a pisar en guardados posteriores).
pub fn save_ssc(song: &Song, path: &Path) -> Result<(), String> {
    let text = ssc::write_string(song);
    if path.exists() {
        let mut bak = path.as_os_str().to_owned();
        bak.push(".bak");
        let bak = PathBuf::from(bak);
        if !bak.exists() {
            std::fs::copy(path, &bak).map_err(|e| format!("No se pudo crear la copia {}: {}", bak.display(), e))?;
        }
    }
    std::fs::write(path, text).map_err(|e| format!("No se pudo guardar {}: {}", path.display(), e))
}
