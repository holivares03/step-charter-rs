//! Notas y conversión entre el texto de `#NOTES` y una lista de notas.
//!
//! Soporta:
//! - Caracteres clásicos de StepMania: `0 1 2 3 4 M L F K` (+ sufijo de keysound `[n]`).
//! - Tokens de StepF2/StepP1 entre llaves: compacto `{T P A ...}` (p. ej. `{110}`, `{2m8}`)
//!   y con barras `{T|attr|fake|x}` (p. ej. `{2|n|1|0}`). El token original se conserva
//!   para escribirlo sin pérdidas.
//! - Bloques por jugador separados con `&` (routine / `#SEPARATENOTES:YES`).

use super::timing::{ROWS_PER_BEAT, ROWS_PER_MEASURE};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoteKind {
    Tap,
    Hold,
    Roll,
    Mine,
    Lift,
    /// Nota que se reproduce sola (`K`, keysound automático).
    AutoKeysound,
    /// Tipo especial de StepF2/StepP1 que no interpretamos (se dibuja con su letra).
    Special(char),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub row: i32,
    /// Fila final (igual a `row` si no es hold/roll).
    pub end_row: i32,
    pub col: u8,
    pub kind: NoteKind,
    /// Nota falsa (no se evalúa): `F` o fake=1 en el token de StepF2.
    pub fake: bool,
    /// Jugador: 0 = sin jugador explícito, 1..=9 en charts co-op.
    pub player: u8,
    /// Bloque `&` al que pertenece (0 si el chart no está separado).
    pub block: u8,
    /// Token original de la cabeza (sólo si no es un carácter simple).
    pub token: Option<String>,
    /// Token original de la cola del hold.
    pub tail_token: Option<String>,
    /// Sufijo crudo (`[12]` keysound o `{...}` de attack) que va tras el carácter.
    pub suffix: Option<String>,
}

impl Note {
    pub fn new(row: i32, col: u8, kind: NoteKind) -> Self {
        Note {
            row,
            end_row: row,
            col,
            kind,
            fake: false,
            player: 0,
            block: 0,
            token: None,
            tail_token: None,
            suffix: None,
        }
    }

    pub fn is_long(&self) -> bool {
        matches!(self.kind, NoteKind::Hold | NoteKind::Roll)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ParseStats {
    pub measures: usize,
    pub odd_measure_sizes: usize,
    pub collisions: usize,
    pub orphan_tails: usize,
    pub unclosed_holds: usize,
    pub unknown_chars: usize,
    pub special_tokens: usize,
    pub width_mismatch: usize,
}

/// Información extraída de un token.
#[derive(Debug, Clone, PartialEq)]
struct TokenInfo {
    ty: char,
    player: u8,
    fake: bool,
}

fn parse_brace_token(tok: &str) -> TokenInfo {
    let inner = tok.trim_start_matches('{').trim_end_matches('}');
    if inner.contains('|') {
        let parts: Vec<&str> = inner.split('|').collect();
        let ty = parts.first().and_then(|p| p.chars().next()).unwrap_or('0');
        let fake = parts.get(2).map(|f| f.trim() == "1").unwrap_or(false);
        return TokenInfo { ty, player: 0, fake };
    }
    let mut it = inner.chars();
    let ty = it.next().unwrap_or('0');
    let player = it.next().and_then(|c| c.to_digit(10)).filter(|d| (1..=9).contains(d)).unwrap_or(0) as u8;
    TokenInfo { ty, player, fake: false }
}

/// Divide una línea de notas en tokens (uno por columna).
fn tokenize_line(line: &str) -> Vec<(String, Option<String>)> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let mut tok = String::new();
        if c == '{' {
            while i < chars.len() {
                tok.push(chars[i]);
                i += 1;
                if tok.ends_with('}') {
                    break;
                }
            }
        } else {
            tok.push(c);
            i += 1;
        }
        // Sufijos estilo StepMania: keysound `[n]` y attack `{...}` tras una `A`.
        let mut suffix = String::new();
        loop {
            if i < chars.len() && chars[i] == '[' {
                while i < chars.len() {
                    suffix.push(chars[i]);
                    i += 1;
                    if suffix.ends_with(']') {
                        break;
                    }
                }
            } else if i < chars.len() && chars[i] == '{' && tok == "A" {
                while i < chars.len() {
                    suffix.push(chars[i]);
                    i += 1;
                    if suffix.ends_with('}') {
                        break;
                    }
                }
            } else {
                break;
            }
        }
        out.push((tok, if suffix.is_empty() { None } else { Some(suffix) }));
    }
    out
}

/// Parsea el contenido de `#NOTES`.
///
/// - `num_cols`: columnas esperadas (0 = deducir de la primera línea).
/// - `separated`: si los bloques `&` corresponden a jugadores (routine / SEPARATENOTES).
pub fn parse_notes(text: &str, num_cols: usize, separated: bool) -> (Vec<Note>, usize, usize, ParseStats) {
    let mut stats = ParseStats::default();
    let mut notes: Vec<Note> = Vec::new();

    let blocks: Vec<&str> = text.split('&').collect();
    let mut cols = num_cols;

    for (block_idx, block) in blocks.iter().enumerate() {
        let block_u8 = block_idx.min(255) as u8;
        let default_player = if separated { (block_idx + 1).min(9) as u8 } else { 0 };

        // Holds pendientes: (columna) -> índices de notas cabeza.
        let mut pending: Vec<Vec<usize>> = Vec::new();
        // Mapa (fila, columna) -> índice, para detectar colisiones.
        let mut occupied: std::collections::HashMap<(i32, u8), usize> = std::collections::HashMap::new();

        let measures: Vec<&str> = block.split(',').collect();
        let mut measure_index = 0i32;
        for (mi, measure) in measures.iter().enumerate() {
            let lines: Vec<&str> = measure
                .lines()
                .map(|l| l.split("//").next().unwrap_or("").trim())
                .filter(|l| !l.is_empty())
                .collect();
            if lines.is_empty() {
                // Una medida vacía al final (por una coma final) no cuenta.
                if mi + 1 == measures.len() {
                    continue;
                }
            }
            stats.measures += 1;
            let n = lines.len().max(1);
            if ROWS_PER_MEASURE as usize % n != 0 {
                stats.odd_measure_sizes += 1;
            }
            for (li, line) in lines.iter().enumerate() {
                let toks = tokenize_line(line);
                if cols == 0 {
                    cols = toks.len();
                }
                if toks.len() != cols {
                    stats.width_mismatch += 1;
                }
                let frac = li as f64 / n as f64;
                let row = ((measure_index as f64 + frac) * ROWS_PER_MEASURE as f64).round() as i32;

                for (col, (tok, suffix)) in toks.into_iter().enumerate().take(cols.max(1)) {
                    if tok == "0" {
                        continue;
                    }
                    let col_u8 = col as u8;
                    let (ty, player, fake, raw) = if tok.starts_with('{') {
                        let info = parse_brace_token(&tok);
                        (info.ty, info.player, info.fake, Some(tok.clone()))
                    } else {
                        (tok.chars().next().unwrap_or('0'), 0, false, None)
                    };
                    let player = if player == 0 { default_player } else { player };

                    if pending.len() <= col {
                        pending.resize(col + 1, Vec::new());
                    }

                    if ty == '3' {
                        // Cola: busca la cabeza pendiente (mismo jugador si es posible).
                        let list = &mut pending[col];
                        let pos = list
                            .iter()
                            .rposition(|&h| notes[h].player == player)
                            .or_else(|| if list.is_empty() { None } else { Some(list.len() - 1) });
                        if let Some(p) = pos {
                            let h = list.remove(p);
                            notes[h].end_row = row;
                            notes[h].tail_token = raw;
                        } else {
                            stats.orphan_tails += 1;
                        }
                        continue;
                    }

                    let (kind, fake) = match ty {
                        '1' => (NoteKind::Tap, fake),
                        '2' => (NoteKind::Hold, fake),
                        '4' => (NoteKind::Roll, fake),
                        'M' => (NoteKind::Mine, fake),
                        'L' => (NoteKind::Lift, fake),
                        'F' => (NoteKind::Tap, true),
                        'K' => (NoteKind::AutoKeysound, fake),
                        c => {
                            if raw.is_some() {
                                stats.special_tokens += 1;
                            } else {
                                stats.unknown_chars += 1;
                            }
                            (NoteKind::Special(c), fake)
                        }
                    };

                    // Una nota nueva en la columna cierra holds pendientes del mismo jugador
                    // (quedan como taps, igual que en StepMania).
                    let list = &mut pending[col];
                    let mut k = 0;
                    while k < list.len() {
                        let h = list[k];
                        if notes[h].player == player {
                            stats.unclosed_holds += 1;
                            notes[h].kind = NoteKind::Tap;
                            list.remove(k);
                        } else {
                            k += 1;
                        }
                    }

                    let note = Note {
                        row,
                        end_row: row,
                        col: col_u8,
                        kind,
                        fake,
                        player,
                        block: block_u8,
                        token: raw,
                        tail_token: None,
                        suffix,
                    };

                    if let Some(&prev) = occupied.get(&(row, col_u8)) {
                        // Colisión: StepMania sobrescribe con la última.
                        stats.collisions += 1;
                        pending[col].retain(|&h| h != prev);
                        notes[prev] = note;
                        if notes[prev].is_long() {
                            pending[col].push(prev);
                        }
                        continue;
                    }
                    let idx = notes.len();
                    occupied.insert((row, col_u8), idx);
                    if note.is_long() {
                        pending[col].push(idx);
                    }
                    notes.push(note);
                }
            }
            measure_index += 1;
        }

        // Holds sin cola => taps.
        for list in pending {
            for h in list {
                stats.unclosed_holds += 1;
                notes[h].kind = NoteKind::Tap;
            }
        }
    }

    sort_notes(&mut notes);
    (notes, cols, blocks.len(), stats)
}

/// Orden canónico: por fila, columna y bloque (para búsquedas binarias por fila).
pub fn sort_notes(notes: &mut [Note]) {
    notes.sort_by(|a, b| (a.row, a.col, a.block).cmp(&(b.row, b.col, b.block)));
}

fn head_char(n: &Note) -> String {
    if let Some(t) = &n.token {
        return t.clone();
    }
    let c = match n.kind {
        NoteKind::Tap if n.fake => 'F',
        NoteKind::Tap => '1',
        NoteKind::Hold => '2',
        NoteKind::Roll => '4',
        NoteKind::Mine => 'M',
        NoteKind::Lift => 'L',
        NoteKind::AutoKeysound => 'K',
        NoteKind::Special(c) => c,
    };
    let mut s = c.to_string();
    if let Some(sfx) = &n.suffix {
        s.push_str(sfx);
    }
    s
}

const DIVISIONS: [i32; 10] = [4, 8, 12, 16, 24, 32, 48, 64, 96, 192];

/// Escribe las notas en el formato de `#NOTES` (estilo de StepMania 5).
pub fn write_notes(notes: &[Note], num_cols: usize, num_blocks: usize) -> String {
    let mut out = String::new();
    let max_block = notes.iter().map(|n| n.block as usize + 1).max().unwrap_or(1);
    let blocks = num_blocks.max(max_block).max(1);

    for b in 0..blocks {
        if b > 0 {
            out.push_str("&\n");
        }
        // Celdas: (fila, col) -> texto
        let mut cells: std::collections::BTreeMap<(i32, usize), String> = std::collections::BTreeMap::new();
        let mut last_row = 0;
        for n in notes.iter().filter(|n| n.block as usize == b) {
            if (n.col as usize) >= num_cols {
                continue;
            }
            cells.insert((n.row, n.col as usize), head_char(n));
            last_row = last_row.max(n.row);
            if n.is_long() {
                let tail = n.tail_token.clone().unwrap_or_else(|| "3".to_string());
                cells.entry((n.end_row, n.col as usize)).or_insert(tail);
                last_row = last_row.max(n.end_row);
            }
        }
        let num_measures = if cells.is_empty() { 1 } else { (last_row / ROWS_PER_MEASURE) as usize + 1 };

        out.push_str("// measure 0\n");
        for m in 0..num_measures {
            if m > 0 {
                out.push_str(&format!(",  // measure {}\n", m));
            }
            let start = m as i32 * ROWS_PER_MEASURE;
            let end = start + ROWS_PER_MEASURE;
            let rows_in_measure: Vec<i32> =
                cells.range((start, 0)..(end, 0)).map(|((r, _), _)| *r - start).collect();
            let div = DIVISIONS
                .iter()
                .copied()
                .find(|&d| {
                    let step = ROWS_PER_MEASURE / d;
                    rows_in_measure.iter().all(|r| r % step == 0)
                })
                .unwrap_or(ROWS_PER_MEASURE);
            let step = ROWS_PER_MEASURE / div;
            for l in 0..div {
                let row = start + l * step;
                for c in 0..num_cols {
                    match cells.get(&(row, c)) {
                        Some(t) => out.push_str(t),
                        None => out.push('0'),
                    }
                }
                out.push('\n');
            }
        }
    }
    out
}

/// Comparación semántica de dos listas de notas (ignora el orden).
pub fn same_notes(a: &[Note], b: &[Note]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let key = |n: &Note| (n.block, n.row, n.col);
    let mut x: Vec<&Note> = a.iter().collect();
    let mut y: Vec<&Note> = b.iter().collect();
    x.sort_by_key(|n| key(n));
    y.sort_by_key(|n| key(n));
    x.iter().zip(y.iter()).all(|(p, q)| p == q)
}

/// Divisiones de beat típicas para colorear notas por cuantización.
pub fn quantization_of_row(row: i32) -> i32 {
    let r = row.rem_euclid(ROWS_PER_BEAT);
    for (q, step) in [(4, 48), (8, 24), (12, 16), (16, 12), (24, 8), (32, 6), (48, 4), (64, 3), (192, 1)] {
        if r % step == 0 {
            return q;
        }
    }
    192
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_notes_and_holds() {
        let text = "\n10000\n02000\n00000\n03000\n,\n0000M\n0000F\n00000\n00000\n";
        let (notes, cols, blocks, st) = parse_notes(text, 5, false);
        assert_eq!(cols, 5);
        assert_eq!(blocks, 1);
        assert_eq!(st.orphan_tails, 0);
        assert_eq!(notes.len(), 4);
        assert_eq!(notes[0].kind, NoteKind::Tap);
        assert_eq!(notes[1].kind, NoteKind::Hold);
        assert_eq!(notes[1].row, 48);
        assert_eq!(notes[1].end_row, 144);
        assert_eq!(notes[2].kind, NoteKind::Mine);
        assert_eq!(notes[3].fake, true);
    }

    #[test]
    fn stepf2_tokens() {
        let text = "00{110}0000{120}00\n0000000000\n00{210}0000{220}00\n0030000300\n";
        let (notes, cols, _, _) = parse_notes(text, 10, false);
        assert_eq!(cols, 10);
        assert_eq!(notes.len(), 4);
        let p1: Vec<_> = notes.iter().filter(|n| n.player == 1).collect();
        let p2: Vec<_> = notes.iter().filter(|n| n.player == 2).collect();
        assert_eq!(p1.len(), 2);
        assert_eq!(p2.len(), 2);
        assert!(notes.iter().any(|n| n.kind == NoteKind::Hold && n.player == 2 && n.end_row == 144));
    }

    #[test]
    fn pipe_tokens_fake() {
        let (notes, _, _, _) = parse_notes("{2|n|1|0}0000\n00000\n{3|n|1|0}0000\n00000\n", 5, false);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].kind, NoteKind::Hold);
        assert!(notes[0].fake);
        assert_eq!(notes[0].tail_token.as_deref(), Some("{3|n|1|0}"));
    }

    #[test]
    fn separated_players() {
        let (notes, _, blocks, _) = parse_notes("10000\n00000\n00000\n00000\n&\n00001\n00000\n00000\n00000\n", 5, true);
        assert_eq!(blocks, 2);
        assert_eq!(notes[0].player, 1);
        assert_eq!(notes[1].player, 2);
        assert_eq!(notes[1].block, 1);
    }

    #[test]
    fn write_roundtrip() {
        let text = "{110}0000\n00000\n00{2m8}00\n00000\n,\n00{3m8}00\n0000M\n00000\n00000\n0100F\n00000\n00000\n00000\n";
        let (notes, cols, blocks, _) = parse_notes(text, 5, false);
        let written = write_notes(&notes, cols, blocks);
        let (again, _, _, _) = parse_notes(&written, 5, false);
        assert!(same_notes(&notes, &again), "{}", written);
    }
}
