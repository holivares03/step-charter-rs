//! Operaciones de edición independientes de la interfaz: selección por región,
//! portapapeles, borrar, espejo y edición de segmentos de timing.

use std::collections::HashSet;
use std::sync::Arc;

use crate::simfile::notes::{quantization_of_row, sort_notes, Note, NoteKind};
use crate::simfile::timing::{row_to_beat, BpmSeg, ScrollSeg, StopSeg, TimingData, WarpSeg};

// ---------------------------------------------------------------------------
// Selección
// ---------------------------------------------------------------------------

/// Filtro por tipo de nota dentro de una región (como *Edit → Select* de ArrowVortex).
#[derive(Debug, Clone, PartialEq)]
pub enum NoteFilter {
    /// Pasos: taps, holds, rolls y lifts que no son fakes.
    Steps,
    /// Pasos en filas donde el mismo jugador pisa 2 o más paneles. Guarda (fila, jugador).
    Jumps(Arc<HashSet<(i32, u8)>>),
    Mines,
    Holds,
    Rolls,
    Lifts,
    Fakes,
    /// Tipos especiales de StepF2/StepP1 y keysounds automáticos.
    Specials,
    /// Notas dentro de warps o fakes (no evaluables). Guarda las filas.
    Warped(Arc<HashSet<i32>>),
    /// Notas de una cuantización exacta (4 = 4th, 16 = 16th...).
    Quant(i32),
    /// Notas de un jugador (co-op).
    Player(u8),
}

fn is_step(n: &Note) -> bool {
    !n.fake && matches!(n.kind, NoteKind::Tap | NoteKind::Hold | NoteKind::Roll | NoteKind::Lift)
}

/// Sufijo inglés de cuantización, como en ArrowVortex (4th, 8th, 12th, 32nd...).
pub fn quant_name(q: i32) -> String {
    let suffix = match q % 10 {
        2 if q % 100 != 12 => "nd",
        _ => "th",
    };
    format!("{}{}", q, suffix)
}

impl NoteFilter {
    pub fn jumps(notes: &[Note]) -> Self {
        let mut count: std::collections::HashMap<(i32, u8), u32> = std::collections::HashMap::new();
        for n in notes.iter().filter(|n| is_step(n)) {
            *count.entry((n.row, n.player)).or_insert(0) += 1;
        }
        NoteFilter::Jumps(Arc::new(count.into_iter().filter(|(_, c)| *c >= 2).map(|(k, _)| k).collect()))
    }

    pub fn warped(notes: &[Note], td: &TimingData) -> Self {
        NoteFilter::Warped(Arc::new(notes.iter().map(|n| n.row).filter(|&r| td.is_unjudgable(r)).collect()))
    }

    pub fn matches(&self, n: &Note) -> bool {
        match self {
            NoteFilter::Steps => is_step(n),
            NoteFilter::Jumps(set) => is_step(n) && set.contains(&(n.row, n.player)),
            NoteFilter::Mines => n.kind == NoteKind::Mine,
            NoteFilter::Holds => n.kind == NoteKind::Hold,
            NoteFilter::Rolls => n.kind == NoteKind::Roll,
            NoteFilter::Lifts => n.kind == NoteKind::Lift,
            NoteFilter::Fakes => n.fake,
            NoteFilter::Specials => matches!(n.kind, NoteKind::Special(_) | NoteKind::AutoKeysound),
            NoteFilter::Warped(rows) => rows.contains(&n.row),
            NoteFilter::Quant(q) => quantization_of_row(n.row) == *q,
            NoteFilter::Player(p) => n.player == *p,
        }
    }

    /// Nombre en plural para mensajes ("12 minas").
    pub fn name(&self) -> String {
        match self {
            NoteFilter::Steps => "pasos".into(),
            NoteFilter::Jumps(_) => "notas en saltos".into(),
            NoteFilter::Mines => "minas".into(),
            NoteFilter::Holds => "holds".into(),
            NoteFilter::Rolls => "rolls".into(),
            NoteFilter::Lifts => "lifts".into(),
            NoteFilter::Fakes => "fakes".into(),
            NoteFilter::Specials => "notas especiales".into(),
            NoteFilter::Warped(_) => "notas en warps/fakes".into(),
            NoteFilter::Quant(q) => format!("notas {}", quant_name(*q)),
            NoteFilter::Player(p) => format!("notas de P{}", p),
        }
    }
}

/// Región seleccionada: filas y columnas, ambas inclusivas, con un filtro opcional.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub r0: i32,
    pub r1: i32,
    pub c0: usize,
    pub c1: usize,
    pub filter: Option<NoteFilter>,
}

impl Selection {
    pub fn new(ra: i32, rb: i32, ca: usize, cb: usize) -> Self {
        Selection { r0: ra.min(rb), r1: ra.max(rb), c0: ca.min(cb), c1: ca.max(cb), filter: None }
    }

    pub fn with_filter(mut self, f: NoteFilter) -> Self {
        self.filter = Some(f);
        self
    }

    /// Región de filas que abarca todas las columnas.
    pub fn rows(ra: i32, rb: i32, num_cols: usize) -> Self {
        Selection::new(ra, rb, 0, num_cols.saturating_sub(1))
    }

    pub fn contains(&self, n: &Note) -> bool {
        let c = n.col as usize;
        n.row >= self.r0
            && n.row <= self.r1
            && c >= self.c0
            && c <= self.c1
            && self.filter.as_ref().map(|f| f.matches(n)).unwrap_or(true)
    }

    pub fn span(&self) -> i32 {
        self.r1 - self.r0
    }
}

pub fn count_selected(notes: &[Note], sel: &Selection) -> usize {
    notes.iter().filter(|n| sel.contains(n)).count()
}

/// Recorta holds que se solapan con la siguiente nota de su columna (StepMania no
/// admite solapes) y ordena.
pub fn normalize(notes: &mut Vec<Note>) {
    sort_notes(notes);
    let mut last_long: std::collections::HashMap<(u8, u8), usize> = std::collections::HashMap::new();
    for i in 0..notes.len() {
        let key = (notes[i].col, notes[i].block);
        if let Some(&h) = last_long.get(&key) {
            if notes[h].end_row >= notes[i].row {
                notes[h].end_row = notes[i].row - 1;
                if notes[h].end_row <= notes[h].row {
                    to_tap(&mut notes[h]);
                }
            }
            last_long.remove(&key);
        }
        if notes[i].is_long() {
            last_long.insert(key, i);
        }
    }
}

fn to_tap(n: &mut Note) {
    n.kind = NoteKind::Tap;
    n.end_row = n.row;
    n.tail_token = None;
    if let Some(t) = n.token.as_mut() {
        // Token compacto de StepF2: cambiar el tipo a tap conservando el resto.
        if t.starts_with('{') && !t.contains('|') && t.len() >= 3 {
            let mut chars: Vec<char> = t.chars().collect();
            chars[1] = '1';
            *t = chars.into_iter().collect();
        }
    }
}

// ---------------------------------------------------------------------------
// Portapapeles
// ---------------------------------------------------------------------------

/// Notas copiadas, con filas relativas al inicio de la región.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    pub span: i32,
    pub c0: usize,
    pub c1: usize,
    pub notes: Vec<Note>,
    /// `true` si se copió con un filtro: al pegar sólo se reemplazan las celdas que
    /// ocupan las notas pegadas, sin vaciar el resto de la región.
    pub merge: bool,
}

const CLIP_HEADER: &str = "Step Charter notes v1";
/// Encabezado de las versiones con el nombre anterior (mismo formato).
const OLD_CLIP_HEADER: &str = "ArrowVortex-RS notes v1";

pub fn copy(notes: &[Note], sel: &Selection) -> Clip {
    let notes = notes
        .iter()
        .filter(|n| sel.contains(n))
        .map(|n| {
            let mut c = n.clone();
            c.row -= sel.r0;
            c.end_row -= sel.r0;
            c
        })
        .collect();
    Clip { span: sel.span(), c0: sel.c0, c1: sel.c1, notes, merge: sel.filter.is_some() }
}

fn kind_str(k: NoteKind) -> String {
    match k {
        NoteKind::Tap => "tap".into(),
        NoteKind::Hold => "hold".into(),
        NoteKind::Roll => "roll".into(),
        NoteKind::Mine => "mine".into(),
        NoteKind::Lift => "lift".into(),
        NoteKind::AutoKeysound => "auto".into(),
        NoteKind::Special(c) => format!("special:{}", c),
    }
}

fn kind_from(s: &str) -> Option<NoteKind> {
    Some(match s {
        "tap" => NoteKind::Tap,
        "hold" => NoteKind::Hold,
        "roll" => NoteKind::Roll,
        "mine" => NoteKind::Mine,
        "lift" => NoteKind::Lift,
        "auto" => NoteKind::AutoKeysound,
        _ => NoteKind::Special(s.strip_prefix("special:")?.chars().next()?),
    })
}

fn opt(s: &Option<String>) -> String {
    s.clone().unwrap_or_else(|| "-".into())
}

fn unopt(s: &str) -> Option<String> {
    if s == "-" {
        None
    } else {
        Some(s.to_string())
    }
}

impl Clip {
    /// Texto para el portapapeles del sistema (permite pegar entre ventanas).
    pub fn to_text(&self) -> String {
        let mode = if self.merge { "merge" } else { "region" };
        let mut s = format!("{}\nspan\t{}\tcols\t{}\t{}\tmode\t{}\n", CLIP_HEADER, self.span, self.c0, self.c1, mode);
        for n in &self.notes {
            s.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                n.row,
                n.end_row,
                n.col,
                kind_str(n.kind),
                n.fake as u8,
                n.player,
                n.block,
                opt(&n.token),
                opt(&n.tail_token),
                opt(&n.suffix)
            ));
        }
        s
    }

    pub fn from_text(text: &str) -> Option<Clip> {
        let mut lines = text.lines();
        if !matches!(lines.next()?.trim(), CLIP_HEADER | OLD_CLIP_HEADER) {
            return None;
        }
        let head: Vec<&str> = lines.next()?.split('\t').collect();
        if head.len() < 5 || head[0] != "span" || head[2] != "cols" {
            return None;
        }
        let span = head[1].parse().ok()?;
        let c0 = head[3].parse().ok()?;
        let c1 = head[4].parse().ok()?;
        let merge = head.len() >= 7 && head[5] == "mode" && head[6] == "merge";
        let mut notes = Vec::new();
        for l in lines {
            if l.trim().is_empty() {
                continue;
            }
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() < 10 {
                return None;
            }
            notes.push(Note {
                row: f[0].parse().ok()?,
                end_row: f[1].parse().ok()?,
                col: f[2].parse().ok()?,
                kind: kind_from(f[3])?,
                fake: f[4] == "1",
                player: f[5].parse().ok()?,
                block: f[6].parse().ok()?,
                token: unopt(f[7]),
                tail_token: unopt(f[8]),
                suffix: unopt(f[9]),
            });
        }
        Some(Clip { span, c0, c1, notes, merge })
    }
}

/// Borra las notas seleccionadas. Devuelve cuántas se borraron.
pub fn delete(notes: &mut Vec<Note>, sel: &Selection) -> usize {
    let before = notes.len();
    notes.retain(|n| !sel.contains(n));
    before - notes.len()
}

/// Pega en `at_row`, reemplazando lo que haya en la región destino.
/// Devuelve la región pegada y cuántas notas se insertaron.
pub fn paste(notes: &mut Vec<Note>, clip: &Clip, at_row: i32, num_cols: usize) -> (Selection, usize) {
    let c1 = clip.c1.min(num_cols.saturating_sub(1));
    let target = Selection::new(at_row, at_row + clip.span, clip.c0.min(c1), c1);
    let incoming: Vec<Note> = clip
        .notes
        .iter()
        .filter(|n| (n.col as usize) < num_cols)
        .map(|n| {
            let mut c = n.clone();
            c.row += at_row;
            c.end_row += at_row;
            c
        })
        .collect();
    if clip.merge {
        notes.retain(|n| !incoming.iter().any(|m| m.row == n.row && m.col == n.col && m.block == n.block));
    } else {
        notes.retain(|n| !target.contains(n));
    }
    let added = incoming.len();
    notes.extend(incoming);
    normalize(notes);
    (target, added)
}

// ---------------------------------------------------------------------------
// Jugadores (co-op)
// ---------------------------------------------------------------------------

fn type_char(n: &Note) -> char {
    match n.kind {
        NoteKind::Hold => '2',
        NoteKind::Roll => '4',
        NoteKind::Mine => 'M',
        NoteKind::Lift => 'L',
        NoteKind::Special(c) => c,
        _ => '1',
    }
}

/// Cambia el jugador de una nota y ajusta su bloque `&` y su token de StepF2.
/// `player = 0` la deja sin jugador (charts de un jugador).
pub fn set_note_player(n: &mut Note, player: u8, separated: bool, use_tokens: bool) {
    n.player = player;
    n.block = if separated && player > 0 { player - 1 } else { 0 };
    let digit = char::from_digit(player as u32, 10).unwrap_or('0');
    let rewrite = |t: &mut String| {
        if t.starts_with('{') && !t.contains('|') && t.chars().count() >= 3 {
            let mut chars: Vec<char> = t.chars().collect();
            chars[2] = digit;
            *t = chars.into_iter().collect();
        }
    };
    match n.token.as_mut() {
        Some(t) => {
            rewrite(t);
            if let Some(tt) = n.tail_token.as_mut() {
                rewrite(tt);
            }
        }
        None if use_tokens && player > 0 => {
            n.token = Some(format!("{{{}{}0}}", type_char(n), player));
            if n.is_long() {
                n.tail_token = Some(format!("{{3{}0}}", player));
            }
        }
        None => {}
    }
}

// ---------------------------------------------------------------------------
// Espejo
// ---------------------------------------------------------------------------

/// Aplica la permutación de columnas `perm` a las notas seleccionadas. Las notas no
/// seleccionadas que queden debajo se reemplazan. Devuelve cuántas se movieron.
pub fn mirror(notes: &mut Vec<Note>, sel: &Selection, perm: &[usize]) -> usize {
    let mut moved = Vec::new();
    let mut rest = Vec::new();
    for n in notes.drain(..) {
        if sel.contains(&n) {
            let mut m = n;
            if let Some(&c) = perm.get(m.col as usize) {
                m.col = c as u8;
            }
            moved.push(m);
        } else {
            rest.push(n);
        }
    }
    let count = moved.len();
    rest.retain(|n| !moved.iter().any(|m| m.row == n.row && m.col == n.col && m.block == n.block));
    rest.extend(moved);
    *notes = rest;
    normalize(notes);
    count
}

// ---------------------------------------------------------------------------
// Segmentos de timing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegKind {
    Bpm,
    Stop,
    Delay,
    Warp,
    Scroll,
}

impl SegKind {
    pub const ALL: [SegKind; 5] = [SegKind::Bpm, SegKind::Stop, SegKind::Delay, SegKind::Warp, SegKind::Scroll];

    pub fn name(self) -> &'static str {
        match self {
            SegKind::Bpm => "BPM",
            SegKind::Stop => "Stop",
            SegKind::Delay => "Delay",
            SegKind::Warp => "Warp",
            SegKind::Scroll => "Scroll",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            SegKind::Bpm => "",
            SegKind::Stop | SegKind::Delay => "s",
            SegKind::Warp => "beats",
            SegKind::Scroll => "x",
        }
    }

    /// Valor mínimo permitido.
    pub fn min(self) -> f64 {
        match self {
            SegKind::Bpm => 0.001,
            SegKind::Stop | SegKind::Delay | SegKind::Warp => 0.0,
            SegKind::Scroll => -100.0,
        }
    }

    /// Si el valor se hereda del segmento anterior (BPM, scroll) o vale 0 fuera del segmento.
    pub fn inherits(self) -> bool {
        matches!(self, SegKind::Bpm | SegKind::Scroll)
    }
}

pub fn seg_list(td: &TimingData, kind: SegKind) -> Vec<(i32, f64)> {
    match kind {
        SegKind::Bpm => td.bpms.iter().map(|s| (s.row, s.bpm)).collect(),
        SegKind::Stop => td.stops.iter().map(|s| (s.row, s.seconds)).collect(),
        SegKind::Delay => td.delays.iter().map(|s| (s.row, s.seconds)).collect(),
        SegKind::Warp => td.warps.iter().map(|s| (s.row, s.beats)).collect(),
        SegKind::Scroll => td.scrolls.iter().map(|s| (s.row, s.ratio)).collect(),
    }
}

pub fn seg_at(td: &TimingData, kind: SegKind, row: i32) -> Option<f64> {
    seg_list(td, kind).into_iter().find(|(r, _)| *r == row).map(|(_, v)| v)
}

/// Valor vigente en `row` (heredado para BPM/scroll, 0 para stops/delays/warps).
pub fn seg_effective(td: &TimingData, kind: SegKind, row: i32) -> f64 {
    if let Some(v) = seg_at(td, kind, row) {
        return v;
    }
    match kind {
        SegKind::Bpm => td.bpm_at_row(row),
        SegKind::Scroll => td.scrolls.iter().filter(|s| s.row <= row).last().map(|s| s.ratio).unwrap_or(1.0),
        _ => 0.0,
    }
}

/// Inserta o reemplaza el segmento en `row`.
pub fn seg_set(td: &mut TimingData, kind: SegKind, row: i32, value: f64) {
    let value = value.max(kind.min());
    macro_rules! upsert {
        ($list:expr, $make:expr, $field:ident) => {{
            if let Some(s) = $list.iter_mut().find(|s| s.row == row) {
                s.$field = value;
            } else {
                $list.push($make);
                $list.sort_by_key(|s| s.row);
            }
        }};
    }
    match kind {
        SegKind::Bpm => upsert!(td.bpms, BpmSeg { row, bpm: value }, bpm),
        SegKind::Stop => upsert!(td.stops, StopSeg { row, seconds: value }, seconds),
        SegKind::Delay => upsert!(td.delays, StopSeg { row, seconds: value }, seconds),
        SegKind::Warp => upsert!(td.warps, WarpSeg { row, beats: value }, beats),
        SegKind::Scroll => upsert!(td.scrolls, ScrollSeg { row, ratio: value }, ratio),
    }
}

/// Puede borrarse el segmento: el primer BPM de la canción es obligatorio.
pub fn seg_can_remove(td: &TimingData, kind: SegKind, row: i32) -> bool {
    if seg_at(td, kind, row).is_none() {
        return false;
    }
    !(kind == SegKind::Bpm && td.bpms.len() <= 1)
}

pub fn seg_remove(td: &mut TimingData, kind: SegKind, row: i32) -> bool {
    if !seg_can_remove(td, kind, row) {
        return false;
    }
    match kind {
        SegKind::Bpm => td.bpms.retain(|s| s.row != row),
        SegKind::Stop => td.stops.retain(|s| s.row != row),
        SegKind::Delay => td.delays.retain(|s| s.row != row),
        SegKind::Warp => td.warps.retain(|s| s.row != row),
        SegKind::Scroll => td.scrolls.retain(|s| s.row != row),
    }
    true
}

/// Texto corto para un segmento (para listas).
pub fn seg_label(kind: SegKind, row: i32, value: f64) -> String {
    format!("beat {:>9.3}   {} {:.3}{}", row_to_beat(row), kind.name(), value, kind.unit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tap(row: i32, col: u8) -> Note {
        Note::new(row, col, NoteKind::Tap)
    }

    fn hold(row: i32, end: i32, col: u8) -> Note {
        let mut n = Note::new(row, col, NoteKind::Hold);
        n.end_row = end;
        n
    }

    #[test]
    fn copy_paste_roundtrip_text() {
        let mut t = tap(48, 2);
        t.token = Some("{110}".into());
        t.player = 1;
        let notes = vec![tap(0, 0), t, hold(96, 192, 4), tap(300, 1)];
        let sel = Selection::rows(0, 192, 5);
        let clip = copy(&notes, &sel);
        assert_eq!(clip.notes.len(), 3);
        let back = Clip::from_text(&clip.to_text()).unwrap();
        assert_eq!(back, clip);
    }

    #[test]
    fn paste_overwrites_region() {
        let mut notes = vec![tap(0, 0), tap(48, 1), tap(192, 3), tap(200, 3), tap(400, 2)];
        let clip = copy(&notes, &Selection::rows(0, 48, 5));
        let (sel, added) = paste(&mut notes, &clip, 192, 5);
        assert_eq!(added, 2);
        assert_eq!(sel, Selection::rows(192, 240, 5));
        // Las notas en 192 y 200 (dentro de la región) se reemplazan.
        let rows: Vec<(i32, u8)> = notes.iter().map(|n| (n.row, n.col)).collect();
        assert_eq!(rows, vec![(0, 0), (48, 1), (192, 0), (240, 1), (400, 2)]);
    }

    #[test]
    fn paste_truncates_overlapping_hold() {
        let mut notes = vec![hold(0, 200, 0)];
        let clip = Clip { span: 0, c0: 0, c1: 0, notes: vec![tap(0, 0)], merge: false };
        paste(&mut notes, &clip, 96, 5);
        assert_eq!(notes[0].end_row, 95);
        assert_eq!(notes[1].row, 96);
    }

    #[test]
    fn filters() {
        let mut mine = Note::new(48, 1, NoteKind::Mine);
        mine.player = 0;
        let mut fake = tap(96, 2);
        fake.fake = true;
        let mut p2 = tap(144, 3);
        p2.player = 2;
        let notes = vec![tap(0, 0), tap(0, 4), mine, fake, p2, hold(12, 60, 2)];
        let all = Selection::rows(0, 200, 5);
        let count = |f: NoteFilter| count_selected(&notes, &all.clone().with_filter(f));
        assert_eq!(count(NoteFilter::Steps), 4); // 2 taps + p2 + hold
        assert_eq!(count(NoteFilter::jumps(&notes)), 2);
        assert_eq!(count(NoteFilter::Mines), 1);
        assert_eq!(count(NoteFilter::Holds), 1);
        assert_eq!(count(NoteFilter::Fakes), 1);
        assert_eq!(count(NoteFilter::Player(2)), 1);
        assert_eq!(count(NoteFilter::Quant(4)), 5); // filas 0, 0, 48, 96 y 144
        assert_eq!(count(NoteFilter::Quant(16)), 1); // fila 12
        assert_eq!(quant_name(32), "32nd");
        assert_eq!(quant_name(12), "12th");
    }

    #[test]
    fn filtered_copy_pastes_without_clearing() {
        let mine = Note::new(48, 1, NoteKind::Mine);
        let mut notes = vec![tap(0, 0), mine, tap(192, 3), tap(240, 4)];
        let sel = Selection::rows(0, 48, 5).with_filter(NoteFilter::Mines);
        let clip = copy(&notes, &sel);
        assert!(clip.merge);
        assert_eq!(Clip::from_text(&clip.to_text()).unwrap(), clip);
        paste(&mut notes, &clip, 192, 5);
        // Las notas de la región destino que no chocan se conservan.
        let rows: Vec<(i32, u8)> = notes.iter().map(|n| (n.row, n.col)).collect();
        assert_eq!(rows, vec![(0, 0), (48, 1), (192, 3), (240, 1), (240, 4)]);
    }

    #[test]
    fn delete_only_box() {
        let mut notes = vec![tap(0, 0), tap(0, 4), tap(48, 2)];
        let n = delete(&mut notes, &Selection::new(0, 48, 0, 2));
        assert_eq!(n, 2);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].col, 4);
    }

    #[test]
    fn mirror_pump_single() {
        // H: DL<->DR, UL<->UR
        let perm = [4, 3, 2, 1, 0];
        let mut notes = vec![tap(0, 0), tap(48, 1), tap(96, 2), tap(144, 4)];
        let n = mirror(&mut notes, &Selection::rows(0, 96, 5), &perm);
        assert_eq!(n, 3);
        let cols: Vec<(i32, u8)> = notes.iter().map(|n| (n.row, n.col)).collect();
        assert_eq!(cols, vec![(0, 4), (48, 3), (96, 2), (144, 4)]);
    }

    #[test]
    fn mirror_replaces_unselected_collision() {
        let perm = [4, 3, 2, 1, 0];
        let mut notes = vec![tap(0, 0), tap(0, 4)];
        mirror(&mut notes, &Selection::new(0, 0, 0, 0), &perm);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].col, 4);
    }

    #[test]
    fn players_and_tokens() {
        let mut n = tap(0, 1);
        set_note_player(&mut n, 2, false, true);
        assert_eq!(n.token.as_deref(), Some("{120}"));
        set_note_player(&mut n, 1, true, true);
        assert_eq!(n.token.as_deref(), Some("{110}"));
        assert_eq!(n.block, 0);
        set_note_player(&mut n, 0, false, false);
        assert_eq!(n.token.as_deref(), Some("{100}"));
    }

    #[test]
    fn segments() {
        let mut td = TimingData { bpms: vec![BpmSeg { row: 0, bpm: 120.0 }], ..Default::default() };
        assert_eq!(seg_effective(&td, SegKind::Bpm, 96), 120.0);
        seg_set(&mut td, SegKind::Bpm, 96, 150.0);
        seg_set(&mut td, SegKind::Stop, 48, 0.5);
        seg_set(&mut td, SegKind::Scroll, 0, 2.0);
        assert_eq!(seg_effective(&td, SegKind::Bpm, 200), 150.0);
        assert_eq!(seg_effective(&td, SegKind::Scroll, 500), 2.0);
        assert_eq!(seg_at(&td, SegKind::Stop, 48), Some(0.5));
        assert!(!seg_can_remove(&td, SegKind::Stop, 47));
        assert!(seg_remove(&mut td, SegKind::Bpm, 96));
        // El último BPM no se puede borrar.
        assert!(!seg_remove(&mut td, SegKind::Bpm, 0));
        seg_set(&mut td, SegKind::Bpm, 0, -5.0);
        assert!(td.bpms[0].bpm > 0.0);
    }
}
