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
    /// Se eligió con el rectángulo del mouse: se marcan sólo las notas, sin el recuadro de la
    /// región (como en ArrowVortex).
    pub from_mouse: bool,
}

impl Selection {
    pub fn new(ra: i32, rb: i32, ca: usize, cb: usize) -> Self {
        Selection { r0: ra.min(rb), r1: ra.max(rb), c0: ca.min(cb), c1: ca.max(cb), filter: None, from_mouse: false }
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

/// La región ajustada a las notas que contiene (de la primera a la última fila), con las
/// mismas notas seleccionadas; `None` si no contiene ninguna. Se usa al soltar el rectángulo
/// del mouse: así, al copiar y pegar, la primera nota queda en el cursor (como en ArrowVortex)
/// y no a la distancia a la que empezó el rectángulo.
pub fn trim_to_notes(notes: &[Note], sel: &Selection) -> Option<Selection> {
    let mut rows = notes.iter().filter(|n| sel.contains(n)).map(|n| n.row);
    let first = rows.next()?;
    let (r0, r1) = rows.fold((first, first), |(a, b), r| (a.min(r), b.max(r)));
    Some(Selection { r0, r1, ..sel.clone() })
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
    let fake = n.fake;
    retype(n, NoteKind::Tap, fake);
}

/// Cambia el tipo de una nota (y si es fake). Si deja de ser hold/roll, pierde la cola. El
/// token de StepF2, si tiene, se reescribe con el tipo nuevo conservando el resto:
/// compacto `{110}` (tipo, jugador, atributo) o con barras `{1|n|0|0}` (tipo|attr|fake|x).
fn retype(n: &mut Note, kind: NoteKind, fake: bool) {
    n.kind = kind;
    n.fake = fake;
    if !n.is_long() {
        n.end_row = n.row;
        n.tail_token = None;
    }
    let ty = match kind {
        NoteKind::Hold => '2',
        NoteKind::Roll => '4',
        NoteKind::Mine => 'M',
        NoteKind::Lift => 'L',
        NoteKind::AutoKeysound => 'K',
        NoteKind::Special(c) => c,
        NoteKind::Tap => '1',
    };
    let set_fake = |t: &mut String| {
        let mut parts: Vec<String> = t.trim_start_matches('{').trim_end_matches('}').split('|').map(String::from).collect();
        if parts.len() >= 3 {
            parts[2] = if fake { "1" } else { "0" }.to_string();
            *t = format!("{{{}}}", parts.join("|"));
        }
    };
    if let Some(t) = n.token.as_mut() {
        if t.starts_with('{') && t.contains('|') {
            // Con barras: el tipo va en la primera parte y fake en la tercera.
            let rest = t.splitn(2, '|').nth(1).unwrap_or("").to_string();
            *t = format!("{{{}|{}", ty, rest);
            set_fake(t);
            if let Some(tt) = n.tail_token.as_mut() {
                set_fake(tt);
            }
        } else if t.starts_with('{') && t.chars().count() >= 3 {
            // Compacto: sin campo fake; un tap fake usa la letra `F` como en StepMania.
            let mut chars: Vec<char> = t.chars().collect();
            chars[1] = if kind == NoteKind::Tap && fake { 'F' } else { ty };
            *t = chars.into_iter().collect();
        } else {
            // Carácter simple con sufijo (keysound/attack): se regenera desde el tipo.
            n.token = None;
        }
    }
}

/// Vuelve a poner las notas `moved` en `notes`, reemplazando las que ocupen la misma celda
/// (fila, columna y bloque), y recorta los holds que queden solapados.
fn merge_moved(notes: &mut Vec<Note>, moved: Vec<Note>) {
    let cells: HashSet<(i32, u8, u8)> = moved.iter().map(|m| (m.row, m.col, m.block)).collect();
    notes.retain(|n| !cells.contains(&(n.row, n.col, n.block)));
    notes.extend(moved);
    normalize(notes);
}

/// Separa las notas seleccionadas (devueltas) de las demás (que quedan en `notes`).
fn take_selected(notes: &mut Vec<Note>, sel: &Selection) -> Vec<Note> {
    let (picked, rest): (Vec<Note>, Vec<Note>) = notes.drain(..).partition(|n| sel.contains(n));
    *notes = rest;
    picked
}

// ---------------------------------------------------------------------------
// Portapapeles
// ---------------------------------------------------------------------------

/// Notas copiadas, con filas relativas al inicio de la región, y cambios de tempo copiados
/// junto con ellas (cajas seleccionadas en el notefield), relativos al mismo origen.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    pub span: i32,
    pub c0: usize,
    pub c1: usize,
    pub notes: Vec<Note>,
    /// `true` si se copió con un filtro: al pegar sólo se reemplazan las celdas que
    /// ocupan las notas pegadas, sin vaciar el resto de la región.
    pub merge: bool,
    /// `false` si sólo se copiaron cambios de tempo: al pegar no se tocan las notas.
    pub with_notes: bool,
    pub tempo: Vec<(SegKind, i32, SegValue)>,
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
    Clip { span: sel.span(), c0: sel.c0, c1: sel.c1, notes, merge: sel.filter.is_some(), with_notes: true, tempo: Vec::new() }
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
    /// Sólo cambios de tempo (filas relativas al primero).
    pub fn tempo_only(tempo: Vec<(SegKind, i32, SegValue)>) -> Clip {
        let span = tempo.iter().map(|t| t.1).max().unwrap_or(0).max(0);
        Clip { span, c0: 0, c1: 0, notes: Vec::new(), merge: false, with_notes: false, tempo }
    }

    /// Texto para el portapapeles del sistema (permite pegar entre ventanas). Los cambios de
    /// tempo van después de las notas, en líneas `tempo <tipo> <fila> <valor...>` separadas
    /// por tabulaciones.
    pub fn to_text(&self) -> String {
        let mode = match (self.with_notes, self.merge) {
            (false, _) => "tempo",
            (true, true) => "merge",
            (true, false) => "region",
        };
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
        for (kind, row, value) in &self.tempo {
            let v = match value {
                SegValue::Num(x) => format!("{}", x),
                SegValue::Pair(a, b) => format!("{}\t{}", a, b),
                SegValue::Speed(r, d, u) => format!("{}\t{}\t{}", r, d, u),
                SegValue::Label(t) => t.replace(['\t', '\r', '\n'], " "),
            };
            s.push_str(&format!("tempo\t{}\t{}\t{}\n", kind.tag(), row, v));
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
        let mode = if head.len() >= 7 && head[5] == "mode" { head[6] } else { "region" };
        let merge = mode == "merge";
        let with_notes = mode != "tempo";
        let mut notes = Vec::new();
        let mut tempo = Vec::new();
        for l in lines {
            if l.trim().is_empty() {
                continue;
            }
            let f: Vec<&str> = l.split('\t').collect();
            if f[0] == "tempo" {
                if f.len() < 4 {
                    return None;
                }
                let kind = SegKind::from_tag(f[1])?;
                let row: i32 = f[2].parse().ok()?;
                let num = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok());
                let int = |i: usize| f.get(i).and_then(|v| v.parse::<i32>().ok());
                let value = match kind {
                    SegKind::TimeSig | SegKind::Combo => SegValue::Pair(int(3)?, int(4)?),
                    SegKind::Speed => SegValue::Speed(num(3)?, num(4)?, int(5)?),
                    SegKind::Label => SegValue::Label(f[3..].join(" ")),
                    _ => SegValue::Num(num(3)?),
                };
                tempo.push((kind, row, value));
                continue;
            }
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
        Some(Clip { span, c0, c1, notes, merge, with_notes, tempo })
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
    let mut moved = take_selected(notes, sel);
    for m in moved.iter_mut() {
        if let Some(&c) = perm.get(m.col as usize) {
            m.col = c as u8;
        }
    }
    let count = moved.len();
    merge_moved(notes, moved);
    count
}

/// Permutación "cambiar de lado": la mitad izquierda de las columnas pasa a la derecha y al
/// revés (en double, del pad de P1 al de P2). Con un número impar, la del medio no se mueve.
pub fn side_perm(num_cols: usize) -> Vec<usize> {
    let half = num_cols / 2;
    (0..num_cols)
        .map(|c| {
            if num_cols % 2 == 0 {
                (c + half) % num_cols
            } else if c < half {
                c + half + 1
            } else if c > half {
                c - half - 1
            } else {
                c
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Convertir tipos de nota (Edit → Convert de ArrowVortex)
// ---------------------------------------------------------------------------

/// Tipos que se convierten entre sí.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteType {
    /// Pasos: taps, holds y rolls que no son fakes.
    Step,
    Mine,
    Fake,
    Lift,
}

impl NoteType {
    fn of(n: &Note) -> Option<NoteType> {
        if n.fake {
            return Some(NoteType::Fake);
        }
        match n.kind {
            NoteKind::Tap | NoteKind::Hold | NoteKind::Roll => Some(NoteType::Step),
            NoteKind::Mine => Some(NoteType::Mine),
            NoteKind::Lift => Some(NoteType::Lift),
            NoteKind::AutoKeysound | NoteKind::Special(_) => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            NoteType::Step => "notas",
            NoteType::Mine => "minas",
            NoteType::Fake => "fakes",
            NoteType::Lift => "lifts",
        }
    }
}

/// Conversión sobre las notas seleccionadas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Convert {
    /// De un tipo a otro: los holds y rolls pasan a ser de una fila, salvo de fake a nota,
    /// que conserva el largo.
    Type(NoteType, NoteType),
    /// Holds y rolls a pasos de una fila.
    HoldsToSteps,
    HoldsToMines,
    /// Alterna cada hold ↔ roll.
    HoldsRolls,
}

impl Convert {
    /// Las conversiones de tipo que ofrece ArrowVortex, en el orden de su menú.
    pub const TYPES: [(NoteType, NoteType); 8] = [
        (NoteType::Step, NoteType::Mine),
        (NoteType::Step, NoteType::Fake),
        (NoteType::Step, NoteType::Lift),
        (NoteType::Mine, NoteType::Step),
        (NoteType::Mine, NoteType::Fake),
        (NoteType::Mine, NoteType::Lift),
        (NoteType::Fake, NoteType::Step),
        (NoteType::Lift, NoteType::Step),
    ];

    /// Texto para menús y mensajes ("Notas → minas").
    pub fn label(self) -> String {
        let cap = |s: &str| {
            let mut c = s.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        };
        match self {
            Convert::Type(a, b) => format!("{} → {}", cap(a.name()), b.name()),
            Convert::HoldsToSteps => "Holds → pasos".into(),
            Convert::HoldsToMines => "Holds → minas".into(),
            Convert::HoldsRolls => "Holds ↔ rolls".into(),
        }
    }
}

/// Aplica una conversión a las notas seleccionadas. Devuelve cuántas cambiaron.
pub fn convert(notes: &mut Vec<Note>, sel: &Selection, conv: Convert) -> usize {
    let mut count = 0;
    for n in notes.iter_mut().filter(|n| sel.contains(n)) {
        let changed = match conv {
            Convert::Type(from, to) if from != to && NoteType::of(n) == Some(from) => {
                match to {
                    NoteType::Step => {
                        // De fake a nota se conserva el hold; de mina o lift sale un tap.
                        let kind = if from == NoteType::Fake && n.is_long() { n.kind } else { NoteKind::Tap };
                        retype(n, kind, false);
                    }
                    NoteType::Mine => retype(n, NoteKind::Mine, false),
                    NoteType::Lift => retype(n, NoteKind::Lift, false),
                    NoteType::Fake => retype(n, NoteKind::Tap, true),
                }
                true
            }
            Convert::HoldsToSteps if n.is_long() => {
                let fake = n.fake;
                retype(n, NoteKind::Tap, fake);
                true
            }
            Convert::HoldsToMines if n.is_long() => {
                retype(n, NoteKind::Mine, false);
                true
            }
            Convert::HoldsRolls if n.is_long() => {
                let kind = if n.kind == NoteKind::Roll { NoteKind::Hold } else { NoteKind::Roll };
                let fake = n.fake;
                retype(n, kind, fake);
                true
            }
            _ => false,
        };
        count += changed as usize;
    }
    count
}

/// Para una selección filtrada por el tipo de origen de una conversión, el filtro del tipo de
/// destino (así la selección sigue mostrando las notas convertidas).
pub fn filter_after_convert(f: &NoteFilter, conv: Convert) -> Option<NoteFilter> {
    let of_type = |t: NoteType| match t {
        NoteType::Step => NoteFilter::Steps,
        NoteType::Mine => NoteFilter::Mines,
        NoteType::Fake => NoteFilter::Fakes,
        NoteType::Lift => NoteFilter::Lifts,
    };
    match (conv, f) {
        (Convert::Type(from, to), f) if *f == of_type(from) => Some(of_type(to)),
        (Convert::HoldsToSteps, NoteFilter::Holds | NoteFilter::Rolls) => Some(NoteFilter::Steps),
        (Convert::HoldsToMines, NoteFilter::Holds | NoteFilter::Rolls) => Some(NoteFilter::Mines),
        (Convert::HoldsRolls, NoteFilter::Holds) => Some(NoteFilter::Rolls),
        (Convert::HoldsRolls, NoteFilter::Rolls) => Some(NoteFilter::Holds),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Jugadores y lados
// ---------------------------------------------------------------------------

/// Pasa las notas seleccionadas al jugador siguiente (P1 → P2 → … → P1), como la tecla `/`
/// de ArrowVortex. Devuelve (notas cambiadas, jugador nuevo si todas iban al mismo).
pub fn cycle_players(notes: &mut Vec<Note>, sel: &Selection, players: u8, separated: bool, use_tokens: bool) -> (usize, Option<u8>) {
    let players = players.max(1);
    let mut moved = take_selected(notes, sel);
    let mut target: Option<Option<u8>> = None;
    for n in moved.iter_mut() {
        let next = if n.player == 0 || n.player >= players { 1 } else { n.player + 1 };
        set_note_player(n, next, separated, use_tokens);
        target = match target {
            None => Some(Some(next)),
            Some(Some(p)) if p == next => Some(Some(p)),
            _ => Some(None),
        };
    }
    let count = moved.len();
    merge_moved(notes, moved);
    (count, target.flatten())
}

// ---------------------------------------------------------------------------
// Escalar (Edit → Scale de ArrowVortex)
// ---------------------------------------------------------------------------

/// Escala la distancia de las notas seleccionadas a la primera de ellas por `num/den`
/// (2:1 expande al doble, 1:2 comprime a la mitad). Las que quedan fuera de la región se
/// eliminan, como en ArrowVortex. Devuelve (notas escaladas, si se recortó alguna).
pub fn scale(notes: &mut Vec<Note>, sel: &Selection, num: i32, den: i32) -> (usize, bool) {
    let mut moved = take_selected(notes, sel);
    let Some(top) = moved.iter().map(|n| n.row).min() else {
        return (0, false);
    };
    let den = den.max(1);
    for n in moved.iter_mut() {
        n.row = top + (n.row - top) * num / den;
        n.end_row = top + (n.end_row - top) * num / den;
        if n.is_long() && n.end_row <= n.row {
            to_tap(n);
        }
    }
    let before = moved.len();
    moved.retain(|n| n.row <= sel.r1);
    let truncated = moved.len() < before;
    let count = moved.len();
    merge_moved(notes, moved);
    (count, truncated)
}

// ---------------------------------------------------------------------------
// Insertar y borrar beats (Adjust Tempo → Insert/Delete beats de ArrowVortex)
// ---------------------------------------------------------------------------

/// Nueva fila de algo que estaba en `r` al insertar (`n > 0`) o borrar (`n < 0`) filas en `at`.
fn shift_row(r: i32, at: i32, n: i32) -> i32 {
    if n >= 0 {
        if r >= at {
            r + n
        } else {
            r
        }
    } else if r >= at - n {
        r + n
    } else if r >= at {
        at
    } else {
        r
    }
}

/// Inserta (`n > 0`) o borra (`n < 0`) filas en `at`. Las notas desde `at` se desplazan; al
/// borrar, las que empiezan en la zona borrada se eliminan y los holds que la atraviesan se
/// acortan. Devuelve cuántas notas se eliminaron.
pub fn insert_rows_notes(notes: &mut Vec<Note>, at: i32, n: i32) -> usize {
    if n == 0 {
        return 0;
    }
    let before = notes.len();
    if n < 0 {
        notes.retain(|x| x.row < at || x.row >= at - n);
    }
    let removed = before - notes.len();
    for x in notes.iter_mut() {
        x.row = shift_row(x.row, at, n);
        x.end_row = shift_row(x.end_row, at, n);
        if x.is_long() && x.end_row <= x.row {
            to_tap(x);
        }
    }
    normalize(notes);
    removed
}

/// Lo mismo para los segmentos de timing. Los de la fila 0 (BPM inicial, etc.) no se mueven
/// ni se borran, como en ArrowVortex.
pub fn insert_rows_timing(td: &mut TimingData, at: i32, n: i32) {
    if n == 0 {
        return;
    }
    macro_rules! shift {
        ($($list:ident),*) => {$(
            if n < 0 {
                td.$list.retain(|s| s.row == 0 || s.row < at || s.row >= at - n);
            }
            for s in td.$list.iter_mut() {
                if s.row > 0 {
                    s.row = shift_row(s.row, at, n);
                }
            }
        )*};
    }
    let first_bpm = td.bpms.first().cloned();
    shift!(bpms, stops, delays, warps, time_sigs, tick_counts, combos, speeds, scrolls, fakes, labels);
    if td.bpms.is_empty() {
        td.bpms.extend(first_bpm);
    }
}

// ---------------------------------------------------------------------------
// Segmentos de timing
// ---------------------------------------------------------------------------

/// Tipos de segmento que se editan: los del *Adjust Tempo* de ArrowVortex (BPM, stop, delay,
/// warp) y los de su *Adjust Tempo (SM5)*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SegKind {
    Bpm,
    Stop,
    Delay,
    Warp,
    TimeSig,
    Ticks,
    Combo,
    Speed,
    Scroll,
    Fake,
    Label,
}

/// Valor de un segmento.
#[derive(Debug, Clone, PartialEq)]
pub enum SegValue {
    /// BPM, stop y delay (segundos), warp y fake (beats), scroll (x) y ticks.
    Num(f64),
    /// Time signature (beats por compás / figura) o combo (multiplicador de acierto / fallo).
    Pair(i32, i32),
    /// Speed: factor, duración de la transición y unidad (0 = beats, 1 = segundos).
    Speed(f64, f64, i32),
    Label(String),
}

impl SegValue {
    pub fn num(&self) -> f64 {
        match self {
            SegValue::Num(v) => *v,
            _ => 0.0,
        }
    }
}

impl SegKind {
    pub const ALL: [SegKind; 11] = [
        SegKind::Bpm,
        SegKind::Stop,
        SegKind::Delay,
        SegKind::Warp,
        SegKind::TimeSig,
        SegKind::Ticks,
        SegKind::Combo,
        SegKind::Speed,
        SegKind::Scroll,
        SegKind::Fake,
        SegKind::Label,
    ];
    /// Los de *Adjust Tempo* (el resto son los de StepMania 5).
    pub const BASIC: [SegKind; 4] = [SegKind::Bpm, SegKind::Stop, SegKind::Delay, SegKind::Warp];
    pub const SM5: [SegKind; 7] =
        [SegKind::TimeSig, SegKind::Ticks, SegKind::Combo, SegKind::Speed, SegKind::Scroll, SegKind::Fake, SegKind::Label];

    pub fn name(self) -> &'static str {
        match self {
            SegKind::Bpm => "BPM",
            SegKind::Stop => "Stop",
            SegKind::Delay => "Delay",
            SegKind::Warp => "Warp",
            SegKind::TimeSig => "Compás",
            SegKind::Ticks => "Ticks",
            SegKind::Combo => "Combo",
            SegKind::Speed => "Speed",
            SegKind::Scroll => "Scroll",
            SegKind::Fake => "Fake",
            SegKind::Label => "Label",
        }
    }

    /// Explicación corta (para la ayuda emergente).
    pub fn help(self) -> &'static str {
        match self {
            SegKind::Bpm => "Tempo en beats por minuto",
            SegKind::Stop => "Pausa en este beat, en segundos (la nota se toca antes de la pausa)",
            SegKind::Delay => "Pausa en este beat, en segundos (la nota se toca después de la pausa)",
            SegKind::Warp => "Salto: los beats siguientes se saltan, en beats",
            SegKind::TimeSig => "Time signature: beats por compás / figura del beat",
            SegKind::Ticks => "Ticks de combo por beat mientras se mantiene un hold",
            SegKind::Combo => "Multiplicador de combo al acertar / al fallar",
            SegKind::Speed => "Speed: factor de velocidad, duración de la transición y unidad (beats o segundos)",
            SegKind::Scroll => "Scroll: factor de velocidad de desplazamiento desde este beat",
            SegKind::Fake => "Zona fake desde este beat: las notas no se evalúan, en beats",
            SegKind::Label => "Etiqueta de texto (sin comas, punto y coma ni =)",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            SegKind::Stop | SegKind::Delay => "s",
            SegKind::Warp | SegKind::Fake => "beats",
            SegKind::Scroll => "x",
            _ => "",
        }
    }

    /// Valor mínimo de los numéricos.
    pub fn min(self) -> f64 {
        match self {
            SegKind::Bpm => 0.001,
            SegKind::Scroll => -100000.0,
            _ => 0.0,
        }
    }

    /// El valor sigue vigente hasta el próximo segmento (BPM, compás, ticks, combo, speed,
    /// scroll); los demás sólo existen en su beat.
    pub fn inherits(self) -> bool {
        matches!(self, SegKind::Bpm | SegKind::TimeSig | SegKind::Ticks | SegKind::Combo | SegKind::Speed | SegKind::Scroll)
    }

    /// Su caja va a la izquierda del notefield (si no, a la derecha).
    pub fn left_side(self) -> bool {
        !matches!(self, SegKind::Speed | SegKind::Scroll | SegKind::Fake | SegKind::Label)
    }

    /// Nombre en el texto del portapapeles.
    fn tag(self) -> &'static str {
        match self {
            SegKind::Bpm => "bpm",
            SegKind::Stop => "stop",
            SegKind::Delay => "delay",
            SegKind::Warp => "warp",
            SegKind::TimeSig => "timesig",
            SegKind::Ticks => "ticks",
            SegKind::Combo => "combo",
            SegKind::Speed => "speed",
            SegKind::Scroll => "scroll",
            SegKind::Fake => "fake",
            SegKind::Label => "label",
        }
    }

    fn from_tag(s: &str) -> Option<SegKind> {
        SegKind::ALL.into_iter().find(|k| k.tag() == s)
    }

    /// Tipo por su nombre ("bpm", "stop", "compás"...), sin distinguir mayúsculas.
    pub fn parse(s: &str) -> Option<SegKind> {
        let s = s.to_lowercase();
        SegKind::ALL.into_iter().find(|k| k.tag() == s || k.name().to_lowercase() == s)
    }

    /// Valor cuando no hay ningún segmento (los de StepMania 5).
    fn default_value(self) -> SegValue {
        match self {
            SegKind::TimeSig => SegValue::Pair(4, 4),
            SegKind::Ticks => SegValue::Num(4.0),
            SegKind::Combo => SegValue::Pair(1, 1),
            SegKind::Speed => SegValue::Speed(1.0, 0.0, 0),
            SegKind::Scroll => SegValue::Num(1.0),
            SegKind::Label => SegValue::Label(String::new()),
            _ => SegValue::Num(0.0),
        }
    }
}

pub fn seg_list(td: &TimingData, kind: SegKind) -> Vec<(i32, SegValue)> {
    use SegValue::*;
    match kind {
        SegKind::Bpm => td.bpms.iter().map(|s| (s.row, Num(s.bpm))).collect(),
        SegKind::Stop => td.stops.iter().map(|s| (s.row, Num(s.seconds))).collect(),
        SegKind::Delay => td.delays.iter().map(|s| (s.row, Num(s.seconds))).collect(),
        SegKind::Warp => td.warps.iter().map(|s| (s.row, Num(s.beats))).collect(),
        SegKind::TimeSig => td.time_sigs.iter().map(|s| (s.row, Pair(s.num, s.den))).collect(),
        SegKind::Ticks => td.tick_counts.iter().map(|s| (s.row, Num(s.ticks as f64))).collect(),
        SegKind::Combo => td.combos.iter().map(|s| (s.row, Pair(s.hit, s.miss))).collect(),
        SegKind::Speed => td.speeds.iter().map(|s| (s.row, Speed(s.ratio, s.duration, s.unit))).collect(),
        SegKind::Scroll => td.scrolls.iter().map(|s| (s.row, Num(s.ratio))).collect(),
        SegKind::Fake => td.fakes.iter().map(|s| (s.row, Num(s.beats))).collect(),
        SegKind::Label => td.labels.iter().map(|s| (s.row, Label(s.text.clone()))).collect(),
    }
}

pub fn seg_at(td: &TimingData, kind: SegKind, row: i32) -> Option<SegValue> {
    seg_list(td, kind).into_iter().find(|(r, _)| *r == row).map(|(_, v)| v)
}

/// Valor vigente en `row`: el del segmento de esa fila; si no hay, el último anterior (para
/// los que se heredan) o el valor por defecto.
pub fn seg_effective(td: &TimingData, kind: SegKind, row: i32) -> SegValue {
    if let Some(v) = seg_at(td, kind, row) {
        return v;
    }
    if kind == SegKind::Bpm {
        return SegValue::Num(td.bpm_at_row(row));
    }
    if kind.inherits() {
        if let Some((_, v)) = seg_list(td, kind).into_iter().filter(|(r, _)| *r <= row).last() {
            return v;
        }
    }
    kind.default_value()
}

/// Texto de label válido: StepMania usa `,`, `;` y `=` como separadores.
pub fn clean_label(text: &str) -> String {
    text.replace([',', ';', '='], "_")
}

/// Inserta o reemplaza el segmento en `row`, ajustando el valor a su rango. En los que sólo
/// existen en su beat (stop, delay, warp, fake, label), un valor 0 o vacío quita el segmento.
pub fn seg_set(td: &mut TimingData, kind: SegKind, row: i32, value: SegValue) {
    use crate::simfile::timing::{ComboSeg, LabelSeg, SpeedSeg, TickSeg, TimeSigSeg};
    macro_rules! upsert {
        ($list:expr, $seg:expr) => {{
            let seg = $seg;
            match $list.iter_mut().find(|s| s.row == row) {
                Some(s) => *s = seg,
                None => {
                    $list.push(seg);
                    $list.sort_by_key(|s| s.row);
                }
            }
        }};
    }
    let n = value.num().max(kind.min());
    if !kind.inherits() {
        let empty = match &value {
            SegValue::Label(t) => clean_label(t).trim().is_empty(),
            _ => n <= 0.0,
        };
        if empty {
            seg_remove(td, kind, row);
            return;
        }
    }
    let pair = |lo: i32| match value {
        SegValue::Pair(a, b) => (a.clamp(lo, 1000), b.clamp(lo, 1000)),
        _ => (lo, lo),
    };
    match kind {
        SegKind::Bpm => upsert!(td.bpms, BpmSeg { row, bpm: n }),
        SegKind::Stop => upsert!(td.stops, StopSeg { row, seconds: n }),
        SegKind::Delay => upsert!(td.delays, StopSeg { row, seconds: n }),
        SegKind::Warp => upsert!(td.warps, WarpSeg { row, beats: n }),
        SegKind::Fake => upsert!(td.fakes, WarpSeg { row, beats: n }),
        SegKind::Scroll => upsert!(td.scrolls, ScrollSeg { row, ratio: n }),
        SegKind::Ticks => upsert!(td.tick_counts, TickSeg { row, ticks: (n.round() as i32).clamp(0, 1000) }),
        SegKind::TimeSig => {
            let (num, den) = pair(1);
            upsert!(td.time_sigs, TimeSigSeg { row, num, den })
        }
        SegKind::Combo => {
            let (hit, miss) = pair(1);
            upsert!(td.combos, ComboSeg { row, hit, miss })
        }
        SegKind::Speed => {
            let (ratio, duration, unit) = match value {
                SegValue::Speed(r, d, u) => (r.clamp(-1000.0, 1000.0), d.clamp(0.0, 1000.0), u.clamp(0, 1)),
                _ => (1.0, 0.0, 0),
            };
            upsert!(td.speeds, SpeedSeg { row, ratio, duration, unit })
        }
        SegKind::Label => {
            let text = match &value {
                SegValue::Label(t) => clean_label(t),
                _ => String::new(),
            };
            upsert!(td.labels, LabelSeg { row, text })
        }
    }
}

/// Un cambio de tempo concreto: tipo y fila (lo que se selecciona en el notefield).
pub type SegRef = (SegKind, i32);

/// Hay un segmento de ese tipo en esa fila.
pub fn seg_exists(td: &TimingData, kind: SegKind, row: i32) -> bool {
    match kind {
        SegKind::Bpm => td.bpms.iter().any(|s| s.row == row),
        SegKind::Stop => td.stops.iter().any(|s| s.row == row),
        SegKind::Delay => td.delays.iter().any(|s| s.row == row),
        SegKind::Warp => td.warps.iter().any(|s| s.row == row),
        SegKind::TimeSig => td.time_sigs.iter().any(|s| s.row == row),
        SegKind::Ticks => td.tick_counts.iter().any(|s| s.row == row),
        SegKind::Combo => td.combos.iter().any(|s| s.row == row),
        SegKind::Speed => td.speeds.iter().any(|s| s.row == row),
        SegKind::Scroll => td.scrolls.iter().any(|s| s.row == row),
        SegKind::Fake => td.fakes.iter().any(|s| s.row == row),
        SegKind::Label => td.labels.iter().any(|s| s.row == row),
    }
}

/// Es el primer BPM de la canción (obligatorio: no se borra ni se mueve).
fn is_first_bpm(td: &TimingData, kind: SegKind, row: i32) -> bool {
    kind == SegKind::Bpm && td.bpms.first().map(|b| b.row) == Some(row)
}

/// Puede borrarse el segmento: el primer BPM de la canción es obligatorio.
pub fn seg_can_remove(td: &TimingData, kind: SegKind, row: i32) -> bool {
    seg_exists(td, kind, row) && !is_first_bpm(td, kind, row)
}

/// Mueve un segmento a otra fila. No se mueve el primer BPM, ni a una fila negativa, ni
/// a una que ya tiene un segmento del mismo tipo.
pub fn seg_move(td: &mut TimingData, kind: SegKind, from: i32, to: i32) -> Result<(), String> {
    if to == from {
        return Ok(());
    }
    let Some(value) = seg_at(td, kind, from) else { return Err("Ese cambio de tempo ya no existe.".into()) };
    if is_first_bpm(td, kind, from) {
        return Err("El primer BPM no se puede mover.".into());
    }
    if to < 0 {
        return Err("No se puede mover antes del beat 0.".into());
    }
    if seg_exists(td, kind, to) {
        return Err(format!("Ya hay un {} en el beat {:.3}.", kind.name(), row_to_beat(to)));
    }
    if kind == SegKind::Bpm && td.bpms.first().map(|b| to < b.row).unwrap_or(false) {
        return Err("Un BPM no puede quedar antes del primero.".into());
    }
    seg_remove(td, kind, from);
    seg_set(td, kind, to, value);
    Ok(())
}

/// Segmentos de los tipos `kinds` con fila entre `r0` y `r1` (inclusive).
pub fn segs_in_range(td: &TimingData, kinds: &[SegKind], r0: i32, r1: i32) -> Vec<SegRef> {
    kinds
        .iter()
        .flat_map(|&k| seg_list(td, k).into_iter().filter(|(r, _)| *r >= r0 && *r <= r1).map(move |(r, _)| (k, r)))
        .collect()
}

/// Copia los segmentos indicados con filas relativas a `origin`.
pub fn copy_segs(td: &TimingData, refs: &[SegRef], origin: i32) -> Vec<(SegKind, i32, SegValue)> {
    let mut out: Vec<(SegKind, i32, SegValue)> =
        refs.iter().filter_map(|&(k, r)| seg_at(td, k, r).map(|v| (k, r - origin, v))).collect();
    out.sort_by_key(|(k, r, _)| (*r, SegKind::ALL.iter().position(|x| x == k)));
    out
}

/// Borra los segmentos indicados (menos el primer BPM). Devuelve cuántos se borraron.
pub fn remove_segs(td: &mut TimingData, refs: &[SegRef]) -> usize {
    refs.iter().filter(|&&(k, r)| seg_remove(td, k, r)).count()
}

/// Pega segmentos en `at` (reemplaza los del mismo tipo en la misma fila; se omiten los que
/// caerían antes del beat 0). Devuelve los pegados.
pub fn paste_segs(td: &mut TimingData, segs: &[(SegKind, i32, SegValue)], at: i32) -> Vec<SegRef> {
    let mut pasted = Vec::new();
    for (k, r, v) in segs {
        let row = at + r;
        if row >= 0 {
            seg_set(td, *k, row, v.clone());
            if seg_exists(td, *k, row) {
                pasted.push((*k, row));
            }
        }
    }
    pasted
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
        SegKind::TimeSig => td.time_sigs.retain(|s| s.row != row),
        SegKind::Ticks => td.tick_counts.retain(|s| s.row != row),
        SegKind::Combo => td.combos.retain(|s| s.row != row),
        SegKind::Speed => td.speeds.retain(|s| s.row != row),
        SegKind::Scroll => td.scrolls.retain(|s| s.row != row),
        SegKind::Fake => td.fakes.retain(|s| s.row != row),
        SegKind::Label => td.labels.retain(|s| s.row != row),
    }
    true
}

/// Valor en texto ("4/4", "1.500x en 2.000 beats", "0.250 s").
pub fn seg_value_text(kind: SegKind, value: &SegValue) -> String {
    match (kind, value) {
        (SegKind::Ticks, SegValue::Num(v)) => format!("{}", v.round() as i32),
        (_, SegValue::Num(v)) => {
            let unit = kind.unit();
            if unit.is_empty() {
                format!("{:.3}", v)
            } else if unit == "x" {
                format!("{:.3}x", v)
            } else {
                format!("{:.3} {}", v, unit)
            }
        }
        (SegKind::Combo, SegValue::Pair(a, b)) => format!("x{} / x{}", a, b),
        (_, SegValue::Pair(a, b)) => format!("{}/{}", a, b),
        (_, SegValue::Speed(r, d, u)) => {
            if *d > 0.0 {
                format!("{:.3}x en {:.3} {}", r, d, if *u == 1 { "s" } else { "beats" })
            } else {
                format!("{:.3}x", r)
            }
        }
        (_, SegValue::Label(t)) => format!("\"{}\"", t),
    }
}

/// Texto corto para un segmento (para listas).
pub fn seg_label(kind: SegKind, row: i32, value: &SegValue) -> String {
    format!("beat {:>9.3}   {} {}", row_to_beat(row), kind.name(), seg_value_text(kind, value))
}

// ---------------------------------------------------------------------------
// Herramientas de tempo (Adjust Tempo de ArrowVortex)
// ---------------------------------------------------------------------------

/// *Add bounding BPM change* (Ctrl+B): agrega en cada fila un cambio de BPM con el BPM que ya
/// rige ahí, así que no altera la sincronía; sirven de límites para ajustar el tempo sólo entre
/// ellos. Se omiten la fila 0 y las que ya tienen un BPM. Devuelve cuántos se agregaron.
pub fn add_bounding_bpms(td: &mut TimingData, rows: &[i32]) -> usize {
    let new: Vec<(i32, f64)> = rows
        .iter()
        .copied()
        .filter(|&r| r > 0 && seg_at(td, SegKind::Bpm, r).is_none())
        .map(|r| (r, td.bpm_at_row(r)))
        .collect();
    let mut added = 0;
    for (r, bpm) in new {
        if seg_at(td, SegKind::Bpm, r).is_none() {
            seg_set(td, SegKind::Bpm, r, SegValue::Num(bpm));
            added += 1;
        }
    }
    added
}

/// *Convert the selected region to a stop*: un stop en `r0` que dura lo mismo que la región.
/// (Lo que sigue se retrasa ese tiempo: luego se suelen borrar esos beats.) Devuelve la duración.
pub fn region_to_stop(td: &mut TimingData, r0: i32, r1: i32) -> Option<f64> {
    let table = td.build_table();
    let secs = table.time_at_row(r1) - table.time_at_row(r0);
    if r1 <= r0 || secs <= 0.0 {
        return None;
    }
    seg_set(td, SegKind::Stop, r0, SegValue::Num(secs));
    Some(secs)
}

/// *Convert the selected region to a stutter gimmick*: la mitad del tiempo de la región es un
/// stop en `r0` y en la otra mitad pasan todas sus filas con un BPM más rápido; en `r1` vuelve
/// el BPM que había. La duración total no cambia. Devuelve (stop, BPM del stutter).
pub fn region_to_stutter(td: &mut TimingData, r0: i32, r1: i32) -> Option<(f64, f64)> {
    use crate::simfile::timing::ROWS_PER_BEAT;
    let table = td.build_table();
    let secs = table.time_at_row(r1) - table.time_at_row(r0);
    if r1 - r0 < 2 || secs <= 0.0 {
        return None;
    }
    let half = secs * 0.5;
    let bpm = 60.0 * (r1 - r0) as f64 / ROWS_PER_BEAT as f64 / half;
    let end_bpm = td.bpm_at_row(r1);
    seg_set(td, SegKind::Stop, r0, SegValue::Num(half));
    seg_set(td, SegKind::Bpm, r0, SegValue::Num(bpm));
    seg_set(td, SegKind::Bpm, r1, SegValue::Num(end_bpm));
    Some((half, bpm))
}

/// Segmentos de timing copiados, con filas relativas al inicio de la región.
#[derive(Debug, Clone, PartialEq)]
pub struct TempoClip {
    pub span: i32,
    pub segs: Vec<(SegKind, i32, SegValue)>,
}

/// Copia todos los segmentos con fila entre `r0` y `r1` (inclusive).
pub fn copy_tempo(td: &TimingData, r0: i32, r1: i32) -> TempoClip {
    let segs = SegKind::ALL
        .iter()
        .flat_map(|&k| seg_list(td, k).into_iter().filter(|(r, _)| *r >= r0 && *r <= r1).map(move |(r, v)| (k, r - r0, v)))
        .collect();
    TempoClip { span: r1 - r0, segs }
}

/// Pega los segmentos en `at` (reemplaza los del mismo tipo en la misma fila). Devuelve cuántos.
pub fn paste_tempo(td: &mut TimingData, clip: &TempoClip, at: i32) -> usize {
    paste_segs(td, &clip.segs, at).len()
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

    /// El rectángulo del mouse empezó un beat antes de la primera nota: al soltarlo, la región
    /// se ajusta a las notas y al pegar la primera queda en el cursor.
    #[test]
    fn mouse_selection_pastes_first_note_at_cursor() {
        let notes = vec![tap(48, 0), tap(72, 1), tap(96, 2), tap(144, 4), tap(300, 3)];
        let drag = Selection { from_mouse: true, ..Selection::new(0, 200, 0, 4) };
        let sel = trim_to_notes(&notes, &drag).unwrap();
        assert_eq!((sel.r0, sel.r1), (48, 144));
        assert!(sel.from_mouse);
        assert_eq!(count_selected(&notes, &sel), count_selected(&notes, &drag));
        let clip = copy(&notes, &sel);
        let mut target = Vec::new();
        paste(&mut target, &clip, 480, 5);
        assert_eq!(target.iter().map(|n| n.row).collect::<Vec<_>>(), vec![480, 504, 528, 576]);
        // Sin notas dentro, no queda región.
        assert!(trim_to_notes(&notes, &Selection::new(150, 290, 0, 4)).is_none());
    }

    #[test]
    fn paste_truncates_overlapping_hold() {
        let mut notes = vec![hold(0, 200, 0)];
        let clip = Clip { span: 0, c0: 0, c1: 0, notes: vec![tap(0, 0)], merge: false, with_notes: true, tempo: Vec::new() };
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

    fn kinds(notes: &[Note]) -> Vec<(i32, i32, u8, NoteKind, bool)> {
        notes.iter().map(|n| (n.row, n.end_row, n.col, n.kind, n.fake)).collect()
    }

    #[test]
    fn convert_note_types() {
        let mut roll = hold(96, 144, 3);
        roll.kind = NoteKind::Roll;
        let all = Selection::rows(0, 500, 5);
        let mut notes = vec![tap(0, 0), hold(48, 96, 1), roll.clone(), Note::new(200, 2, NoteKind::Mine)];
        // Notas → minas: taps, holds y rolls pasan a minas de una fila; la mina no cambia.
        assert_eq!(convert(&mut notes, &all, Convert::Type(NoteType::Step, NoteType::Mine)), 3);
        assert!(notes.iter().all(|n| n.kind == NoteKind::Mine && n.end_row == n.row));
        // Minas → fakes: taps fake (se guardan como `F`).
        assert_eq!(convert(&mut notes, &all, Convert::Type(NoteType::Mine, NoteType::Fake)), 4);
        assert!(notes.iter().all(|n| n.kind == NoteKind::Tap && n.fake));
        assert_eq!(convert(&mut notes, &all, Convert::Type(NoteType::Fake, NoteType::Step)), 4);
        assert!(notes.iter().all(|n| n.kind == NoteKind::Tap && !n.fake));
        // Sólo dentro de la selección.
        let mut notes = vec![tap(0, 0), tap(300, 0)];
        assert_eq!(convert(&mut notes, &Selection::rows(0, 100, 5), Convert::Type(NoteType::Step, NoteType::Lift)), 1);
        assert_eq!(kinds(&notes), vec![(0, 0, 0, NoteKind::Lift, false), (300, 300, 0, NoteKind::Tap, false)]);
    }

    #[test]
    fn convert_rewrites_stepf2_tokens() {
        let all = Selection::rows(0, 500, 5);
        // Compacto: cambia la letra del tipo y conserva jugador y atributo.
        let mut t = tap(0, 0);
        t.token = Some("{120}".into());
        let mut notes = vec![t];
        convert(&mut notes, &all, Convert::Type(NoteType::Step, NoteType::Mine));
        assert_eq!(notes[0].token.as_deref(), Some("{M20}"));
        convert(&mut notes, &all, Convert::Type(NoteType::Mine, NoteType::Fake));
        assert_eq!(notes[0].token.as_deref(), Some("{F20}"));
        // Con barras: un hold fake pasa a hold normal y la cola también deja de ser fake.
        let mut h = hold(48, 96, 2);
        h.fake = true;
        h.token = Some("{2|n|1|0}".into());
        h.tail_token = Some("{3|n|1|0}".into());
        let mut notes = vec![h];
        convert(&mut notes, &all, Convert::Type(NoteType::Fake, NoteType::Step));
        assert_eq!(kinds(&notes), vec![(48, 96, 2, NoteKind::Hold, false)]);
        assert_eq!(notes[0].token.as_deref(), Some("{2|n|0|0}"));
        assert_eq!(notes[0].tail_token.as_deref(), Some("{3|n|0|0}"));
        // Hold ↔ roll en un token compacto.
        let mut h = hold(0, 48, 1);
        h.token = Some("{210}".into());
        h.tail_token = Some("{310}".into());
        let mut notes = vec![h];
        convert(&mut notes, &all, Convert::HoldsRolls);
        assert_eq!(notes[0].kind, NoteKind::Roll);
        assert_eq!(notes[0].token.as_deref(), Some("{410}"));
        assert_eq!(notes[0].tail_token.as_deref(), Some("{310}"));
    }

    /// Lo convertido se guarda y se vuelve a leer igual (tokens compactos, con barras y simples).
    #[test]
    fn converted_notes_survive_save_and_load() {
        use crate::simfile::notes::{parse_notes, write_notes};
        // Tap de P1 con token, hold fake con barras (filas 0..96), tap simple y hold `{2m8}`.
        let text = "{110}0{2|n|1|0}10\n0000{2m8}\n00{3|n|1|0}00\n00003\n";
        let (mut notes, cols, blocks, _) = parse_notes(text, 5, false);
        assert_eq!(notes.iter().filter(|n| n.is_long()).count(), 2);
        let all = Selection::rows(0, 1000, 5);
        convert(&mut notes, &all, Convert::Type(NoteType::Step, NoteType::Fake));
        convert(&mut notes, &all, Convert::Type(NoteType::Fake, NoteType::Mine));
        let saved = write_notes(&notes, cols, blocks);
        let (back, _, _, _) = parse_notes(&saved, cols, false);
        assert_eq!(kinds(&back), kinds(&notes), "guardado:\n{}", saved);
        assert!(back.iter().all(|n| n.kind == NoteKind::Mine && !n.fake));
        assert_eq!(back.iter().map(|n| n.player).collect::<Vec<_>>(), notes.iter().map(|n| n.player).collect::<Vec<_>>());
    }

    #[test]
    fn convert_holds() {
        let all = Selection::rows(0, 500, 5);
        let mut roll = hold(96, 144, 3);
        roll.kind = NoteKind::Roll;
        let mut notes = vec![tap(0, 0), hold(48, 96, 1), roll];
        assert_eq!(convert(&mut notes, &all, Convert::HoldsRolls), 2);
        assert_eq!(notes[1].kind, NoteKind::Roll);
        assert_eq!(notes[2].kind, NoteKind::Hold);
        assert_eq!(convert(&mut notes, &all.clone().with_filter(NoteFilter::Rolls), Convert::HoldsToMines), 1);
        assert_eq!(kinds(&notes)[1], (48, 48, 1, NoteKind::Mine, false));
        assert_eq!(convert(&mut notes, &all, Convert::HoldsToSteps), 1);
        assert_eq!(kinds(&notes)[2], (96, 96, 3, NoteKind::Tap, false));
        assert_eq!(convert(&mut notes, &all, Convert::HoldsToSteps), 0);

        // La selección por tipo sigue a las notas convertidas.
        let mines_to_steps = Convert::Type(NoteType::Mine, NoteType::Step);
        assert_eq!(filter_after_convert(&NoteFilter::Mines, mines_to_steps), Some(NoteFilter::Steps));
        assert_eq!(filter_after_convert(&NoteFilter::Holds, Convert::HoldsRolls), Some(NoteFilter::Rolls));
        assert_eq!(filter_after_convert(&NoteFilter::Quant(4), mines_to_steps), None);
    }

    #[test]
    fn players_cycle_and_side() {
        let mut a = tap(0, 0);
        a.player = 1;
        a.token = Some("{110}".into());
        let mut b = tap(48, 1);
        b.player = 2;
        b.block = 1;
        // Otra nota de P2 en la celda a la que llega `a` (bloques separados): se reemplaza.
        let mut c = tap(0, 0);
        c.player = 2;
        c.block = 1;
        let mut notes = vec![a, b, c];
        let sel = Selection::rows(0, 48, 5).with_filter(NoteFilter::Steps);
        let sel = Selection { r1: 0, filter: Some(NoteFilter::Player(1)), ..sel };
        assert_eq!(cycle_players(&mut notes, &sel, 2, true, true), (1, Some(2)));
        assert_eq!(notes.len(), 2);
        assert_eq!((notes[0].player, notes[0].block, notes[0].token.as_deref()), (2, 1, Some("{120}")));
        // P2 → P1 al dar la vuelta.
        let (n, to) = cycle_players(&mut notes, &Selection::rows(48, 48, 5), 2, true, true);
        assert_eq!((n, to, notes[1].player, notes[1].block), (1, Some(1), 1, 0));

        assert_eq!(side_perm(10), vec![5, 6, 7, 8, 9, 0, 1, 2, 3, 4]);
        assert_eq!(side_perm(6), vec![3, 4, 5, 0, 1, 2]);
        assert_eq!(side_perm(5), vec![3, 4, 2, 0, 1]);
    }

    #[test]
    fn scale_notes() {
        let region = Selection::rows(0, 95, 5);
        // 2:1 expande al doble desde la primera nota; lo que sale de la región se pierde.
        let mut notes = vec![tap(12, 0), hold(24, 36, 1), tap(60, 2), tap(200, 3)];
        let (n, truncated) = scale(&mut notes, &region, 2, 1);
        assert_eq!((n, truncated), (2, true));
        assert_eq!(kinds(&notes), vec![(12, 12, 0, NoteKind::Tap, false), (36, 60, 1, NoteKind::Hold, false), (200, 200, 3, NoteKind::Tap, false)]);
        // 1:2 comprime a la mitad.
        let mut notes = vec![tap(0, 0), tap(24, 1), tap(48, 2)];
        assert_eq!(scale(&mut notes, &region, 1, 2), (3, false));
        assert_eq!(notes.iter().map(|n| n.row).collect::<Vec<_>>(), vec![0, 12, 24]);
        // 3:4: de 16ths (12 filas) a 12 filas × 3/4 = 9.
        let mut notes = vec![tap(0, 0), tap(12, 1), tap(24, 2)];
        scale(&mut notes, &region, 3, 4);
        assert_eq!(notes.iter().map(|n| n.row).collect::<Vec<_>>(), vec![0, 9, 18]);
    }

    #[test]
    fn insert_and_delete_rows() {
        // Insertar un beat (48 filas) en la fila 96.
        let mut notes = vec![tap(0, 0), tap(96, 1), hold(48, 120, 2), tap(144, 3)];
        assert_eq!(insert_rows_notes(&mut notes, 96, 48), 0);
        assert_eq!(kinds(&notes), vec![
            (0, 0, 0, NoteKind::Tap, false),
            (48, 168, 2, NoteKind::Hold, false),
            (144, 144, 1, NoteKind::Tap, false),
            (192, 192, 3, NoteKind::Tap, false),
        ]);
        // Borrar ese beat lo deja como estaba.
        assert_eq!(insert_rows_notes(&mut notes, 96, -48), 0);
        assert_eq!(kinds(&notes), vec![
            (0, 0, 0, NoteKind::Tap, false),
            (48, 120, 2, NoteKind::Hold, false),
            (96, 96, 1, NoteKind::Tap, false),
            (144, 144, 3, NoteKind::Tap, false),
        ]);
        // Borrar un beat en 96: la nota de 96 se va, el hold termina donde empezaba la zona.
        assert_eq!(insert_rows_notes(&mut notes, 96, -48), 1);
        assert_eq!(kinds(&notes), vec![
            (0, 0, 0, NoteKind::Tap, false),
            (48, 96, 2, NoteKind::Hold, false),
            (96, 96, 3, NoteKind::Tap, false),
        ]);

        let mut td = TimingData {
            bpms: vec![BpmSeg { row: 0, bpm: 120.0 }, BpmSeg { row: 192, bpm: 150.0 }],
            stops: vec![StopSeg { row: 0, seconds: 0.1 }, StopSeg { row: 100, seconds: 0.5 }],
            ..Default::default()
        };
        insert_rows_timing(&mut td, 0, 48);
        assert_eq!(td.bpms.iter().map(|s| s.row).collect::<Vec<_>>(), vec![0, 240]);
        assert_eq!(td.stops.iter().map(|s| s.row).collect::<Vec<_>>(), vec![0, 148]);
        insert_rows_timing(&mut td, 96, -96);
        // El stop de 148 cae en la zona borrada (96..192); el BPM de 240 pasa a 144.
        assert_eq!(td.bpms.iter().map(|s| s.row).collect::<Vec<_>>(), vec![0, 144]);
        assert_eq!(td.stops.iter().map(|s| s.row).collect::<Vec<_>>(), vec![0]);
    }

    #[test]
    fn segments() {
        use SegValue::Num;
        let mut td = TimingData { bpms: vec![BpmSeg { row: 0, bpm: 120.0 }], ..Default::default() };
        assert_eq!(seg_effective(&td, SegKind::Bpm, 96), Num(120.0));
        seg_set(&mut td, SegKind::Bpm, 96, Num(150.0));
        seg_set(&mut td, SegKind::Stop, 48, Num(0.5));
        seg_set(&mut td, SegKind::Scroll, 0, Num(2.0));
        assert_eq!(seg_effective(&td, SegKind::Bpm, 200), Num(150.0));
        assert_eq!(seg_effective(&td, SegKind::Scroll, 500), Num(2.0));
        assert_eq!(seg_at(&td, SegKind::Stop, 48), Some(Num(0.5)));
        assert!(!seg_can_remove(&td, SegKind::Stop, 47));
        // Un stop de 0 segundos quita el segmento.
        seg_set(&mut td, SegKind::Stop, 48, Num(0.0));
        assert!(td.stops.is_empty());
        assert!(seg_remove(&mut td, SegKind::Bpm, 96));
        // El último BPM no se puede borrar.
        assert!(!seg_remove(&mut td, SegKind::Bpm, 0));
        seg_set(&mut td, SegKind::Bpm, 0, Num(-5.0));
        assert!(td.bpms[0].bpm > 0.0);
    }

    #[test]
    fn sm5_segments() {
        use SegValue::*;
        let mut td = TimingData { bpms: vec![BpmSeg { row: 0, bpm: 120.0 }], ..Default::default() };
        // Valores por defecto de StepMania 5 cuando no hay segmentos.
        assert_eq!(seg_effective(&td, SegKind::TimeSig, 100), Pair(4, 4));
        assert_eq!(seg_effective(&td, SegKind::Ticks, 100), Num(4.0));
        assert_eq!(seg_effective(&td, SegKind::Combo, 100), Pair(1, 1));
        assert_eq!(seg_effective(&td, SegKind::Speed, 100), Speed(1.0, 0.0, 0));
        // Se heredan hasta el próximo segmento.
        seg_set(&mut td, SegKind::TimeSig, 192, Pair(3, 4));
        seg_set(&mut td, SegKind::Ticks, 192, Num(2.4));
        seg_set(&mut td, SegKind::Combo, 192, Pair(0, 3));
        seg_set(&mut td, SegKind::Speed, 192, Speed(2.0, 1.5, 7));
        assert_eq!(seg_effective(&td, SegKind::TimeSig, 500), Pair(3, 4));
        assert_eq!(seg_effective(&td, SegKind::Ticks, 500), Num(2.0));
        // Combo mínimo 1 y unidad de speed 0 o 1, como en ArrowVortex.
        assert_eq!(seg_effective(&td, SegKind::Combo, 500), Pair(1, 3));
        assert_eq!(seg_effective(&td, SegKind::Speed, 500), Speed(2.0, 1.5, 1));
        // Fake y label sólo existen en su beat; vacíos se quitan.
        seg_set(&mut td, SegKind::Fake, 96, Num(2.0));
        assert_eq!(seg_effective(&td, SegKind::Fake, 97), Num(0.0));
        seg_set(&mut td, SegKind::Label, 48, Label("Coro, parte=2;".into()));
        assert_eq!(td.labels[0].text, "Coro_ parte_2_");
        seg_set(&mut td, SegKind::Label, 48, Label("  ".into()));
        assert!(td.labels.is_empty());
        assert_eq!(seg_label(SegKind::Speed, 192, &Speed(2.0, 1.5, 1)), "beat     4.000   Speed 2.000x en 1.500 s");
        assert_eq!(seg_value_text(SegKind::TimeSig, &Pair(3, 4)), "3/4");
        assert!(seg_remove(&mut td, SegKind::Speed, 192));
        assert!(td.speeds.is_empty());
    }

    /// Lo editado se escribe en los tags de StepMania 5 y se vuelve a leer igual.
    #[test]
    fn sm5_segments_save_roundtrip() {
        use crate::simfile::timing as t;
        use SegValue::*;
        let mut td = TimingData { bpms: vec![BpmSeg { row: 0, bpm: 120.0 }], ..Default::default() };
        seg_set(&mut td, SegKind::TimeSig, 0, Pair(7, 8));
        seg_set(&mut td, SegKind::Ticks, 96, Num(2.0));
        seg_set(&mut td, SegKind::Combo, 96, Pair(2, 3));
        seg_set(&mut td, SegKind::Speed, 192, Speed(0.5, 2.25, 1));
        seg_set(&mut td, SegKind::Fake, 240, Num(1.5));
        seg_set(&mut td, SegKind::Label, 288, Label("Drop final".into()));
        assert_eq!(t::parse_time_sigs(&t::write_time_sigs(&td.time_sigs)), td.time_sigs);
        assert_eq!(t::parse_ticks(&t::write_ticks(&td.tick_counts)), td.tick_counts);
        assert_eq!(t::parse_combos(&t::write_combos(&td.combos)), td.combos);
        assert_eq!(t::parse_speeds(&t::write_speeds(&td.speeds)), td.speeds);
        assert_eq!(t::parse_warps(&t::write_warps(&td.fakes)), td.fakes);
        assert_eq!(t::parse_labels(&t::write_labels(&td.labels)), td.labels);
    }

    #[test]
    fn tempo_tools() {
        use SegValue::Num;
        // 120 BPM: un beat = 0.5 s.
        let base = TimingData { bpms: vec![BpmSeg { row: 0, bpm: 120.0 }], ..Default::default() };
        let total = |td: &TimingData, row: i32| td.build_table().time_at_row(row);

        // Ctrl+B: BPM iguales al vigente en los extremos; no cambian los tiempos.
        let mut td = base.clone();
        seg_set(&mut td, SegKind::Bpm, 192, Num(180.0));
        let before = total(&td, 400);
        assert_eq!(add_bounding_bpms(&mut td, &[0, 96, 192, 288]), 2);
        assert_eq!(td.bpms.iter().map(|s| (s.row, s.bpm)).collect::<Vec<_>>(), vec![(0, 120.0), (96, 120.0), (192, 180.0), (288, 180.0)]);
        assert!((total(&td, 400) - before).abs() < 1e-9);

        // Región → stop: 2 beats = 1 s de stop en el inicio.
        let mut td = base.clone();
        assert!((region_to_stop(&mut td, 96, 192).unwrap() - 1.0).abs() < 1e-9);
        assert_eq!(td.stops.len(), 1);
        assert_eq!(td.stops[0].row, 96);

        // Región → stutter: medio tiempo de stop, la otra mitad al doble de BPM, y vuelve a 120.
        let mut td = base.clone();
        let before = total(&td, 400);
        let (half, bpm) = region_to_stutter(&mut td, 96, 192).unwrap();
        assert!((half - 0.5).abs() < 1e-9 && (bpm - 240.0).abs() < 1e-9);
        assert_eq!(seg_at(&td, SegKind::Bpm, 192), Some(Num(120.0)));
        assert!((total(&td, 400) - before).abs() < 1e-9, "el stutter no debe desfasar lo que sigue");
        assert!(region_to_stutter(&mut base.clone(), 10, 11).is_none());

        // Copiar y pegar tempo.
        let mut td = base.clone();
        seg_set(&mut td, SegKind::Scroll, 100, Num(2.0));
        seg_set(&mut td, SegKind::Label, 120, SegValue::Label("drop".into()));
        seg_set(&mut td, SegKind::Stop, 300, Num(0.25));
        let clip = copy_tempo(&td, 96, 192);
        assert_eq!(clip.span, 96);
        assert_eq!(clip.segs.len(), 2);
        assert_eq!(paste_tempo(&mut td, &clip, 480), 2);
        assert_eq!(seg_at(&td, SegKind::Scroll, 484), Some(Num(2.0)));
        assert_eq!(seg_at(&td, SegKind::Label, 504), Some(SegValue::Label("drop".into())));
    }

    /// Cajas de tempo seleccionadas: mover, borrar, copiar y pegar.
    #[test]
    fn selected_segments() {
        use SegValue::*;
        let mut td = TimingData { bpms: vec![BpmSeg { row: 0, bpm: 120.0 }], ..Default::default() };
        seg_set(&mut td, SegKind::Bpm, 96, Num(150.0));
        seg_set(&mut td, SegKind::Stop, 96, Num(0.5));
        seg_set(&mut td, SegKind::Scroll, 144, Num(2.0));
        seg_set(&mut td, SegKind::Bpm, 192, Num(180.0));

        // Mover: a una fila libre sí; a una ocupada por el mismo tipo, el primer BPM o antes del 0, no.
        assert!(seg_move(&mut td, SegKind::Stop, 96, 120).is_ok());
        assert_eq!(seg_at(&td, SegKind::Stop, 120), Some(Num(0.5)));
        assert!(!seg_exists(&td, SegKind::Stop, 96));
        assert!(seg_move(&mut td, SegKind::Bpm, 96, 192).is_err());
        assert!(seg_move(&mut td, SegKind::Bpm, 0, 48).is_err());
        assert!(seg_move(&mut td, SegKind::Scroll, 144, -1).is_err());
        assert!(!seg_can_remove(&td, SegKind::Bpm, 0));

        // Por tipo dentro de una región.
        assert_eq!(segs_in_range(&td, &[SegKind::Bpm], 50, 500), vec![(SegKind::Bpm, 96), (SegKind::Bpm, 192)]);
        assert_eq!(segs_in_range(&td, &SegKind::ALL, 100, 150), vec![(SegKind::Stop, 120), (SegKind::Scroll, 144)]);

        // Copiar con filas relativas al origen, ordenados por fila.
        let refs = [(SegKind::Scroll, 144), (SegKind::Bpm, 96), (SegKind::Stop, 120)];
        let segs = copy_segs(&td, &refs, 96);
        assert_eq!(segs, vec![(SegKind::Bpm, 0, Num(150.0)), (SegKind::Stop, 24, Num(0.5)), (SegKind::Scroll, 48, Num(2.0))]);

        // Pegar devuelve lo pegado (para dejarlo seleccionado) y omite lo que cae antes del 0.
        let pasted = paste_segs(&mut td, &segs, 480);
        assert_eq!(pasted, vec![(SegKind::Bpm, 480), (SegKind::Stop, 504), (SegKind::Scroll, 528)]);
        assert_eq!(paste_segs(&mut td, &[(SegKind::Stop, -10, Num(1.0))], 5), vec![]);

        // Borrar varios: el primer BPM se salta.
        let n = remove_segs(&mut td, &[(SegKind::Bpm, 0), (SegKind::Bpm, 480), (SegKind::Stop, 504)]);
        assert_eq!(n, 2);
        assert_eq!(td.bpms[0].row, 0);
    }

    /// Las cajas de tempo viajan en el texto del portapapeles, con o sin notas.
    #[test]
    fn clip_text_with_tempo() {
        use SegValue::*;
        let tempo = vec![
            (SegKind::Bpm, 0, Num(150.5)),
            (SegKind::TimeSig, 0, Pair(3, 4)),
            (SegKind::Speed, 24, Speed(0.5, 2.25, 1)),
            (SegKind::Label, 48, Label("Drop\tfinal".into())),
            (SegKind::Stop, -12, Num(0.25)),
        ];
        let only = Clip::tempo_only(tempo.clone());
        assert!(!only.with_notes);
        assert_eq!(only.span, 48);
        let back = Clip::from_text(&only.to_text()).unwrap();
        assert!(!back.with_notes);
        assert_eq!(back.tempo[3].2, Label("Drop final".into()));
        assert_eq!(back.tempo[..3], tempo[..3]);
        assert_eq!(back.tempo[4], tempo[4]);

        let mut with_notes = copy(&[tap(48, 2)], &Selection::rows(0, 96, 5));
        with_notes.tempo = tempo[..2].to_vec();
        let back = Clip::from_text(&with_notes.to_text()).unwrap();
        assert!(back.with_notes);
        assert_eq!(back.notes.len(), 1);
        assert_eq!(back.tempo, tempo[..2].to_vec());

        // El formato anterior (sin tempo) se sigue leyendo.
        let old = "Step Charter notes v1\nspan\t0\tcols\t0\t4\tmode\tregion\n0\t0\t1\ttap\t0\t0\t0\t-\t-\t-\n";
        let c = Clip::from_text(old).unwrap();
        assert!(c.with_notes && c.tempo.is_empty() && c.notes.len() == 1);
    }
}
