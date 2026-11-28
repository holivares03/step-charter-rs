//! Datos de timing (BPMs, stops, delays, warps, ...) y la conversión beat <-> tiempo.
//!
//! La conversión replica el algoritmo de StepMania 5 (`TimingData::GetElapsedTimeInternal`),
//! incluyendo el orden de procesamiento cuando varios eventos caen en la misma fila:
//! destino de warp, cambio de BPM, delay, (la nota), stop, inicio de warp.

pub const ROWS_PER_BEAT: i32 = 48;
pub const BEATS_PER_MEASURE: i32 = 4;
pub const ROWS_PER_MEASURE: i32 = ROWS_PER_BEAT * BEATS_PER_MEASURE;

#[inline]
pub fn beat_to_row(beat: f64) -> i32 {
    (beat * ROWS_PER_BEAT as f64).round() as i32
}

#[inline]
pub fn row_to_beat(row: i32) -> f64 {
    row as f64 / ROWS_PER_BEAT as f64
}

#[derive(Debug, Clone, PartialEq)]
pub struct BpmSeg {
    pub row: i32,
    pub bpm: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct StopSeg {
    pub row: i32,
    pub seconds: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct WarpSeg {
    pub row: i32,
    pub beats: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct TimeSigSeg {
    pub row: i32,
    pub num: i32,
    pub den: i32,
}
#[derive(Debug, Clone, PartialEq)]
pub struct TickSeg {
    pub row: i32,
    pub ticks: i32,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ComboSeg {
    pub row: i32,
    pub hit: i32,
    pub miss: i32,
}
#[derive(Debug, Clone, PartialEq)]
pub struct SpeedSeg {
    pub row: i32,
    pub ratio: f64,
    pub duration: f64,
    /// 0 = beats, 1 = segundos
    pub unit: i32,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ScrollSeg {
    pub row: i32,
    pub ratio: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct LabelSeg {
    pub row: i32,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TimingData {
    pub offset: f64,
    pub bpms: Vec<BpmSeg>,
    pub stops: Vec<StopSeg>,
    pub delays: Vec<StopSeg>,
    pub warps: Vec<WarpSeg>,
    pub time_sigs: Vec<TimeSigSeg>,
    pub tick_counts: Vec<TickSeg>,
    pub combos: Vec<ComboSeg>,
    pub speeds: Vec<SpeedSeg>,
    pub scrolls: Vec<ScrollSeg>,
    pub fakes: Vec<WarpSeg>,
    pub labels: Vec<LabelSeg>,
}

// ---------------------------------------------------------------------------
// Parseo / escritura de listas de segmentos
// ---------------------------------------------------------------------------

fn entries(s: &str) -> impl Iterator<Item = Vec<&str>> {
    s.split(',')
        .map(|e| e.trim())
        .filter(|e| !e.is_empty())
        .map(|e| e.split('=').map(|p| p.trim()).collect::<Vec<_>>())
}

fn num(s: &str) -> Option<f64> {
    let v: f64 = s.trim().parse().ok()?;
    if v.is_finite() {
        Some(v)
    } else {
        None
    }
}

fn int(s: &str) -> Option<i32> {
    let s = s.trim();
    s.parse::<i32>().ok().or_else(|| num(s).map(|v| v.round() as i32))
}

pub fn parse_bpms(s: &str) -> Vec<BpmSeg> {
    entries(s)
        .filter_map(|p| Some(BpmSeg { row: beat_to_row(num(p.first()?)?), bpm: num(p.get(1)?)? }))
        .collect()
}
pub fn parse_stops(s: &str) -> Vec<StopSeg> {
    entries(s)
        .filter_map(|p| Some(StopSeg { row: beat_to_row(num(p.first()?)?), seconds: num(p.get(1)?)? }))
        .collect()
}
pub fn parse_warps(s: &str) -> Vec<WarpSeg> {
    entries(s)
        .filter_map(|p| Some(WarpSeg { row: beat_to_row(num(p.first()?)?), beats: num(p.get(1)?)? }))
        .collect()
}
pub fn parse_time_sigs(s: &str) -> Vec<TimeSigSeg> {
    entries(s)
        .filter_map(|p| {
            Some(TimeSigSeg {
                row: beat_to_row(num(p.first()?)?),
                num: int(p.get(1)?)?,
                den: int(p.get(2).copied().unwrap_or("4"))?,
            })
        })
        .collect()
}
pub fn parse_ticks(s: &str) -> Vec<TickSeg> {
    entries(s)
        .filter_map(|p| Some(TickSeg { row: beat_to_row(num(p.first()?)?), ticks: int(p.get(1)?)? }))
        .collect()
}
pub fn parse_combos(s: &str) -> Vec<ComboSeg> {
    entries(s)
        .filter_map(|p| {
            let hit = int(p.get(1)?)?;
            let miss = p.get(2).and_then(|m| int(m)).unwrap_or(hit);
            Some(ComboSeg { row: beat_to_row(num(p.first()?)?), hit, miss })
        })
        .collect()
}
pub fn parse_speeds(s: &str) -> Vec<SpeedSeg> {
    entries(s)
        .filter_map(|p| {
            Some(SpeedSeg {
                row: beat_to_row(num(p.first()?)?),
                ratio: num(p.get(1)?)?,
                duration: p.get(2).and_then(|d| num(d)).unwrap_or(0.0),
                unit: p.get(3).and_then(|u| int(u)).unwrap_or(0),
            })
        })
        .collect()
}
pub fn parse_scrolls(s: &str) -> Vec<ScrollSeg> {
    entries(s)
        .filter_map(|p| Some(ScrollSeg { row: beat_to_row(num(p.first()?)?), ratio: num(p.get(1)?)? }))
        .collect()
}
pub fn parse_labels(s: &str) -> Vec<LabelSeg> {
    s.split(',')
        .map(|e| e.trim())
        .filter(|e| !e.is_empty())
        .filter_map(|e| {
            let (b, t) = e.split_once('=')?;
            Some(LabelSeg { row: beat_to_row(num(b)?), text: t.trim().to_string() })
        })
        .collect()
}

fn f6(v: f64) -> String {
    format!("{:.6}", v)
}
fn b6(row: i32) -> String {
    f6(row_to_beat(row))
}

fn join_lines(items: Vec<String>) -> String {
    // Formato de StepMania: un segmento por línea, separados por coma al inicio.
    if items.is_empty() {
        return String::new();
    }
    let mut s = String::new();
    for (i, it) in items.iter().enumerate() {
        if i > 0 {
            s.push_str("\n,");
        }
        s.push_str(it);
    }
    s.push('\n');
    s
}

pub fn write_bpms(v: &[BpmSeg]) -> String {
    join_lines(v.iter().map(|x| format!("{}={}", b6(x.row), f6(x.bpm))).collect())
}
pub fn write_stops(v: &[StopSeg]) -> String {
    join_lines(v.iter().map(|x| format!("{}={}", b6(x.row), f6(x.seconds))).collect())
}
pub fn write_warps(v: &[WarpSeg]) -> String {
    join_lines(v.iter().map(|x| format!("{}={}", b6(x.row), f6(x.beats))).collect())
}
pub fn write_time_sigs(v: &[TimeSigSeg]) -> String {
    join_lines(v.iter().map(|x| format!("{}={}={}", b6(x.row), x.num, x.den)).collect())
}
pub fn write_ticks(v: &[TickSeg]) -> String {
    join_lines(v.iter().map(|x| format!("{}={}", b6(x.row), x.ticks)).collect())
}
pub fn write_combos(v: &[ComboSeg]) -> String {
    join_lines(
        v.iter()
            .map(|x| {
                if x.hit == x.miss {
                    format!("{}={}", b6(x.row), x.hit)
                } else {
                    format!("{}={}={}", b6(x.row), x.hit, x.miss)
                }
            })
            .collect(),
    )
}
pub fn write_speeds(v: &[SpeedSeg]) -> String {
    join_lines(
        v.iter()
            .map(|x| format!("{}={}={}={}", b6(x.row), f6(x.ratio), f6(x.duration), x.unit))
            .collect(),
    )
}
pub fn write_scrolls(v: &[ScrollSeg]) -> String {
    join_lines(v.iter().map(|x| format!("{}={}", b6(x.row), f6(x.ratio))).collect())
}
pub fn write_labels(v: &[LabelSeg]) -> String {
    join_lines(v.iter().map(|x| format!("{}={}", b6(x.row), x.text)).collect())
}

impl TimingData {
    /// Fila del último segmento de cualquier tipo (0 si no hay).
    pub fn last_segment_row(&self) -> i32 {
        let rows = [
            self.bpms.iter().map(|s| s.row).max(),
            self.stops.iter().map(|s| s.row).max(),
            self.delays.iter().map(|s| s.row).max(),
            self.warps.iter().map(|s| s.row).max(),
            self.time_sigs.iter().map(|s| s.row).max(),
            self.tick_counts.iter().map(|s| s.row).max(),
            self.combos.iter().map(|s| s.row).max(),
            self.speeds.iter().map(|s| s.row).max(),
            self.scrolls.iter().map(|s| s.row).max(),
            self.fakes.iter().map(|s| s.row).max(),
            self.labels.iter().map(|s| s.row).max(),
        ];
        rows.into_iter().flatten().max().unwrap_or(0).max(0)
    }

    /// Ordena los segmentos por fila (orden estable) — StepMania hace lo mismo al cargar.
    pub fn sort(&mut self) {
        self.bpms.sort_by_key(|s| s.row);
        self.stops.sort_by_key(|s| s.row);
        self.delays.sort_by_key(|s| s.row);
        self.warps.sort_by_key(|s| s.row);
        self.time_sigs.sort_by_key(|s| s.row);
        self.tick_counts.sort_by_key(|s| s.row);
        self.combos.sort_by_key(|s| s.row);
        self.speeds.sort_by_key(|s| s.row);
        self.scrolls.sort_by_key(|s| s.row);
        self.fakes.sort_by_key(|s| s.row);
        self.labels.sort_by_key(|s| s.row);
    }

    pub fn bpm_at_row(&self, row: i32) -> f64 {
        let mut bpm = self.bpms.first().map(|b| b.bpm).unwrap_or(120.0);
        for b in &self.bpms {
            if b.row > row {
                break;
            }
            bpm = b.bpm;
        }
        bpm
    }

    pub fn time_sig_at_row(&self, row: i32) -> (i32, i32) {
        let mut ts = (4, 4);
        for t in &self.time_sigs {
            if t.row > row {
                break;
            }
            ts = (t.num, t.den);
        }
        ts
    }

    /// Rango (min, max) de BPMs usados.
    pub fn bpm_range(&self) -> (f64, f64) {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for b in &self.bpms {
            lo = lo.min(b.bpm);
            hi = hi.max(b.bpm);
        }
        if !lo.is_finite() {
            (120.0, 120.0)
        } else {
            (lo, hi)
        }
    }

    /// ¿La fila cae dentro de un warp o un fake (nota no evaluable)?
    pub fn is_unjudgable(&self, row: i32) -> bool {
        let inside = |r0: i32, beats: f64| {
            let r1 = r0 + (beats * ROWS_PER_BEAT as f64).round() as i32;
            row >= r0 && row < r1
        };
        // Una nota justo en el inicio de un warp sí se evalúa.
        self.warps.iter().any(|w| row > w.row && inside(w.row, w.beats))
            || self.fakes.iter().any(|f| inside(f.row, f.beats))
    }

    pub fn build_table(&self) -> TimingTable {
        TimingTable::new(self)
    }
}

// ---------------------------------------------------------------------------
// Tabla de conversión beat <-> tiempo
// ---------------------------------------------------------------------------

/// Prioridad de procesamiento de eventos en una misma fila (orden de StepMania).
const PRIO_WARP_DEST: u8 = 0;
const PRIO_BPM: u8 = 1;
const PRIO_DELAY: u8 = 2;
const PRIO_MARKER: u8 = 3;
const PRIO_STOP: u8 = 4;
const PRIO_WARP: u8 = 5;

#[derive(Debug, Clone, Copy)]
struct Processed {
    beat: f64,
    prio: u8,
    time_before: f64,
    time_after: f64,
    bps_after: f64,
    warping_after: bool,
}

#[derive(Debug, Clone)]
pub struct TimingTable {
    offset: f64,
    initial_bps: f64,
    events: Vec<Processed>,
}

fn safe_bps(bpm: f64) -> f64 {
    let bps = bpm / 60.0;
    if bps.abs() < 1e-9 {
        1e-9
    } else {
        bps
    }
}

impl TimingTable {
    pub fn new(td: &TimingData) -> Self {
        let mut bpms = td.bpms.clone();
        if bpms.is_empty() {
            bpms.push(BpmSeg { row: 0, bpm: 120.0 });
        }
        bpms.sort_by_key(|b| b.row);
        let mut stops = td.stops.clone();
        stops.sort_by_key(|s| s.row);
        let mut delays = td.delays.clone();
        delays.sort_by_key(|s| s.row);
        let mut warps = td.warps.clone();
        warps.sort_by_key(|s| s.row);

        // Igual que StepMania: el BPM inicial es el vigente en la fila 0
        // (o el primero de la lista si empieza más tarde).
        let mut initial_bps = safe_bps(bpms[0].bpm);
        for b in &bpms {
            if b.row > 0 {
                break;
            }
            initial_bps = safe_bps(b.bpm);
        }

        let (mut ib, mut is, mut id, mut iw) = (0usize, 0usize, 0usize, 0usize);
        let mut last_row: i32 = 0;
        let mut last_time = 0.0f64;
        let mut bps = initial_bps;
        let mut warping = false;
        let mut warp_dest = 0.0f64; // en beats

        let mut events = Vec::new();
        loop {
            let mut ev_row = i32::MAX;
            let mut ev_prio = u8::MAX;
            let mut consider = |row: i32, prio: u8| {
                if row < ev_row || (row == ev_row && prio < ev_prio) {
                    ev_row = row;
                    ev_prio = prio;
                }
            };
            if warping {
                consider(beat_to_row(warp_dest), PRIO_WARP_DEST);
            }
            if ib < bpms.len() {
                consider(bpms[ib].row, PRIO_BPM);
            }
            if id < delays.len() {
                consider(delays[id].row, PRIO_DELAY);
            }
            if is < stops.len() {
                consider(stops[is].row, PRIO_STOP);
            }
            if iw < warps.len() {
                consider(warps[iw].row, PRIO_WARP);
            }
            if ev_prio == u8::MAX {
                break;
            }

            let dt = if warping { 0.0 } else { row_to_beat(ev_row - last_row) / bps };
            last_time += dt;
            let time_before = last_time;
            match ev_prio {
                PRIO_WARP_DEST => warping = false,
                PRIO_BPM => {
                    bps = safe_bps(bpms[ib].bpm);
                    ib += 1;
                }
                PRIO_DELAY => {
                    last_time += delays[id].seconds;
                    id += 1;
                }
                PRIO_STOP => {
                    last_time += stops[is].seconds;
                    is += 1;
                }
                PRIO_WARP => {
                    warping = true;
                    let sum = warps[iw].beats + row_to_beat(warps[iw].row);
                    if sum > warp_dest {
                        warp_dest = sum;
                    }
                    iw += 1;
                }
                _ => unreachable!(),
            }
            last_row = ev_row;
            events.push(Processed {
                beat: row_to_beat(ev_row),
                prio: ev_prio,
                time_before,
                time_after: last_time,
                bps_after: bps,
                warping_after: warping,
            });
        }

        TimingTable { offset: td.offset, initial_bps, events }
    }

    /// Tiempo (segundos de audio) en que suena una nota ubicada en `beat`.
    pub fn time_at_beat(&self, beat: f64) -> f64 {
        let idx = self
            .events
            .partition_point(|e| e.beat < beat || (e.beat == beat && e.prio < PRIO_MARKER));
        let elapsed = if idx == 0 {
            beat / self.initial_bps
        } else {
            let e = &self.events[idx - 1];
            if e.warping_after {
                e.time_after
            } else {
                e.time_after + (beat - e.beat) / e.bps_after
            }
        };
        elapsed - self.offset
    }

    pub fn time_at_row(&self, row: i32) -> f64 {
        self.time_at_beat(row_to_beat(row))
    }

    /// Beat visible en un tiempo dado (durante un stop el beat se congela,
    /// en un warp salta).
    pub fn beat_at_time(&self, time: f64) -> f64 {
        let e_time = time + self.offset;
        let idx = self.events.partition_point(|e| e.time_before <= e_time);
        if idx == 0 {
            return e_time * self.initial_bps;
        }
        let e = &self.events[idx - 1];
        if e_time < e.time_after {
            return e.beat;
        }
        if e.warping_after {
            e.beat
        } else {
            e.beat + (e_time - e.time_after) * e.bps_after
        }
    }

    pub fn offset(&self) -> f64 {
        self.offset
    }
}

// ---------------------------------------------------------------------------
// Posición de scroll (#SCROLLS): cómo se ven las notas en el juego
// ---------------------------------------------------------------------------

/// Un segmento de `#SPEEDS` ya convertido a tiempos.
#[derive(Debug, Clone, Copy)]
struct SpeedPoint {
    beat: f64,
    /// Tiempo en que se llega al beat del segmento (antes de un delay en esa fila).
    start: f64,
    /// Fin de la transición desde el ratio anterior.
    end: f64,
    ratio: f64,
    duration: f64,
}

/// Convierte beats en "posición de scroll": la integral de los multiplicadores de
/// `#SCROLLS`. Con ratio 2 las notas se separan el doble; con 0 se quedan quietas.
/// También guarda `#SPEEDS` (multiplicador de toda la pantalla según la posición actual).
#[derive(Debug, Clone)]
pub struct ScrollMap {
    /// (beat inicial, posición acumulada en ese beat, ratio)
    segs: Vec<(f64, f64, f64)>,
    speeds: Vec<SpeedPoint>,
}

impl ScrollMap {
    pub fn new(td: &TimingData, table: &TimingTable) -> Self {
        let mut map = Self::scrolls_only(td);
        let delay_at = |beat: f64| -> f64 {
            let row = beat_to_row(beat);
            td.delays.iter().filter(|d| d.row == row).map(|d| d.seconds).sum()
        };
        let mut speeds = td.speeds.clone();
        speeds.sort_by_key(|s| s.row);
        map.speeds = speeds
            .iter()
            .map(|s| {
                let beat = row_to_beat(s.row);
                let start = table.time_at_beat(beat) - delay_at(beat);
                let end = if s.unit == 1 {
                    start + s.duration
                } else {
                    let b = beat + s.duration;
                    table.time_at_beat(b) - delay_at(b)
                };
                SpeedPoint { beat, start, end, ratio: s.ratio, duration: s.duration }
            })
            .collect();
        map
    }

    /// Multiplicador de `#SPEEDS` cuando la canción está en `beat` / `time`
    /// (réplica de `TimingData::GetDisplayedSpeedPercent` de StepMania 5).
    pub fn speed_at(&self, beat: f64, time: f64) -> f64 {
        if self.speeds.is_empty() {
            return 1.0;
        }
        // Antes del primer segmento StepMania usa igualmente el índice 0.
        let idx = self.speeds.partition_point(|s| s.beat <= beat).saturating_sub(1);
        let seg = self.speeds[idx];
        let first = self.speeds[0];
        if idx == 0 && first.duration > 0.0 && time < seg.start {
            1.0
        } else if seg.end >= time && (idx > 0 || first.duration > 0.0) {
            let prior = if idx == 0 { 1.0 } else { self.speeds[idx - 1].ratio };
            let dur = seg.end - seg.start;
            let used = if dur == 0.0 { 1.0 } else { ((time - seg.start) / dur).clamp(0.0, 1.0) };
            prior + (seg.ratio - prior) * used
        } else {
            seg.ratio
        }
    }

    /// Ratio de #SCROLLS vigente en un beat.
    pub fn ratio_at(&self, beat: f64) -> f64 {
        self.segs[self.seg_index(beat)].2
    }

    fn scrolls_only(td: &TimingData) -> Self {
        let mut list: Vec<(f64, f64)> = td.scrolls.iter().map(|s| (row_to_beat(s.row), s.ratio)).collect();
        list.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        // Antes del primer segmento el ratio es 1 (como en StepMania).
        let mut segs = vec![(f64::NEG_INFINITY, 0.0, 1.0)];
        let pos_at = |b: f64, segs: &Vec<(f64, f64, f64)>| -> f64 {
            let &(b0, p0, r) = segs.last().unwrap();
            if b0 == f64::NEG_INFINITY {
                b
            } else {
                p0 + (b - b0) * r
            }
        };
        for (b, r) in list {
            let p = pos_at(b, &segs);
            if segs.len() > 1 && segs.last().unwrap().0 == b {
                segs.pop();
            }
            segs.push((b, p, r));
        }
        ScrollMap { segs, speeds: Vec::new() }
    }

    fn seg_index(&self, beat: f64) -> usize {
        self.segs.partition_point(|s| s.0 <= beat).saturating_sub(1)
    }

    pub fn pos(&self, beat: f64) -> f64 {
        let (b0, p0, r) = self.segs[self.seg_index(beat)];
        if b0 == f64::NEG_INFINITY {
            beat
        } else {
            p0 + (beat - b0) * r
        }
    }

    /// Beats cuyo scroll cae dentro de [p_lo, p_hi]: devuelve el rango (min, max).
    pub fn beats_in_pos_range(&self, p_lo: f64, p_hi: f64) -> Option<(f64, f64)> {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for i in 0..self.segs.len() {
            let (b0, p0, r) = self.segs[i];
            let b1 = self.segs.get(i + 1).map(|s| s.0).unwrap_or(f64::INFINITY);
            let b0 = if b0 == f64::NEG_INFINITY { f64::NEG_INFINITY } else { b0 };
            if r.abs() < 1e-12 {
                if p0 >= p_lo && p0 <= p_hi {
                    lo = lo.min(b0);
                    hi = hi.max(b1);
                }
                continue;
            }
            // Beat donde la posición vale p dentro del segmento (recta).
            let beat_of = |p: f64| if b0 == f64::NEG_INFINITY { p } else { b0 + (p - p0) / r };
            let (mut a, mut b) = (beat_of(p_lo), beat_of(p_hi));
            if a > b {
                std::mem::swap(&mut a, &mut b);
            }
            let a = a.max(b0);
            let b = b.min(b1);
            if a <= b {
                lo = lo.min(a);
                hi = hi.max(b);
            }
        }
        if lo <= hi {
            Some((lo.max(-1e6), hi.min(1e7)))
        } else {
            None
        }
    }

    /// Beat con posición `p` más cercano a `near`.
    pub fn beat_at_pos(&self, p: f64, near: f64) -> f64 {
        let mut best = near;
        let mut best_d = f64::INFINITY;
        for i in 0..self.segs.len() {
            let (b0, p0, r) = self.segs[i];
            let b1 = self.segs.get(i + 1).map(|s| s.0).unwrap_or(f64::INFINITY);
            if r.abs() < 1e-12 {
                continue;
            }
            let b = if b0 == f64::NEG_INFINITY { p } else { b0 + (p - p0) / r };
            if b >= b0 && b <= b1 && (b - near).abs() < best_d {
                best_d = (b - near).abs();
                best = b;
            }
        }
        best
    }
}

/// Última fila hasta la que se puede mover el cursor (como `getEndRow` de ArrowVortex): lo
/// más lejano entre la última nota, el último segmento de timing y el final de la música,
/// redondeado al compás siguiente con al menos medio compás de margen (o, si ese compás queda
/// a más de 10 s, al beat siguiente con al menos medio beat).
pub fn end_row(last_note_row: i32, td: &TimingData, table: &TimingTable, music_len: Option<f64>) -> i32 {
    let mut end = last_note_row.max(td.last_segment_row()).max(0);
    if let Some(len) = music_len.filter(|l| *l > 0.0) {
        end = end.max(beat_to_row(table.beat_at_time(len)));
    }
    let round_up = |unit: i32| {
        let r = end + unit * 3 / 2 - 1;
        r - r.rem_euclid(unit)
    };
    let beat_row = round_up(ROWS_PER_BEAT);
    let measure_row = round_up(ROWS_PER_MEASURE);
    if table.time_at_row(measure_row) - table.time_at_row(end) < 10.0 {
        measure_row
    } else {
        beat_row
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_row_like_arrowvortex() {
        let td = TimingData { bpms: vec![BpmSeg { row: 0, bpm: 120.0 }], ..Default::default() };
        let table = td.build_table();
        // Última nota justo al inicio del compás 10: termina en el 11 (un compás de margen).
        assert_eq!(end_row(10 * ROWS_PER_MEASURE, &td, &table, None), 11 * ROWS_PER_MEASURE);
        // A mitad del compás 10: también el 11 (medio compás de margen).
        assert_eq!(end_row(10 * ROWS_PER_MEASURE + 96, &td, &table, None), 11 * ROWS_PER_MEASURE);
        // La música manda si termina después (120 BPM: 60 s = 120 beats = compás 30).
        assert_eq!(end_row(0, &td, &table, Some(60.0)), 31 * ROWS_PER_MEASURE);
        // Un segmento de timing más lejano también cuenta.
        let mut td2 = td.clone();
        td2.labels.push(LabelSeg { row: 50 * ROWS_PER_MEASURE, text: "fin".into() });
        assert_eq!(end_row(0, &td2, &td2.build_table(), None), 51 * ROWS_PER_MEASURE);
        // Con un BPM muy lento el compás queda a más de 10 s: se redondea al beat.
        let slow = TimingData { bpms: vec![BpmSeg { row: 0, bpm: 10.0 }], ..Default::default() };
        assert_eq!(end_row(4 * ROWS_PER_BEAT, &slow, &slow.build_table(), None), 5 * ROWS_PER_BEAT);
    }

    fn td(bpms: &str, stops: &str, delays: &str, warps: &str, offset: f64) -> TimingData {
        TimingData {
            offset,
            bpms: parse_bpms(bpms),
            stops: parse_stops(stops),
            delays: parse_stops(delays),
            warps: parse_warps(warps),
            ..Default::default()
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn constant_bpm() {
        let t = td("0=120", "", "", "", 0.0).build_table();
        assert!(close(t.time_at_beat(4.0), 2.0));
        assert!(close(t.beat_at_time(2.0), 4.0));
        assert!(close(t.time_at_beat(-2.0), -1.0));
    }

    #[test]
    fn offset_sign() {
        // OFFSET negativo => el beat 0 ocurre después del inicio del audio.
        let t = td("0=120", "", "", "", -0.5).build_table();
        assert!(close(t.time_at_beat(0.0), 0.5));
        assert!(close(t.beat_at_time(0.5), 0.0));
    }

    #[test]
    fn bpm_change() {
        let t = td("0=120,4=60", "", "", "", 0.0).build_table();
        assert!(close(t.time_at_beat(4.0), 2.0));
        assert!(close(t.time_at_beat(5.0), 3.0));
        assert!(close(t.beat_at_time(3.0), 5.0));
    }

    #[test]
    fn stop_after_note_delay_before_note() {
        let t = td("0=120", "4=1", "", "", 0.0).build_table();
        // La nota en el stop suena antes de la pausa.
        assert!(close(t.time_at_beat(4.0), 2.0));
        assert!(close(t.time_at_beat(5.0), 3.5));
        assert!(close(t.beat_at_time(2.5), 4.0));

        let t = td("0=120", "", "4=1", "", 0.0).build_table();
        // La nota en el delay suena después de la pausa.
        assert!(close(t.time_at_beat(4.0), 3.0));
        assert!(close(t.time_at_beat(5.0), 3.5));
    }

    #[test]
    fn warp() {
        let t = td("0=120", "", "", "4=2", 0.0).build_table();
        assert!(close(t.time_at_beat(4.0), 2.0));
        assert!(close(t.time_at_beat(5.0), 2.0)); // dentro del warp
        assert!(close(t.time_at_beat(6.0), 2.0));
        assert!(close(t.time_at_beat(7.0), 2.5));
        assert!(close(t.beat_at_time(2.0), 6.0));
        assert!(close(t.beat_at_time(2.5), 7.0));
    }

    #[test]
    fn scroll_map() {
        let td = TimingData { scrolls: parse_scrolls("0=1,4=2,8=0,10=1"), ..Default::default() };
        let m = ScrollMap::new(&td, &td.build_table());
        assert!(close(m.pos(2.0), 2.0));
        assert!(close(m.pos(6.0), 8.0));
        assert!(close(m.pos(9.0), 12.0));
        assert!(close(m.pos(11.0), 13.0));
        assert!(close(m.beat_at_pos(8.0, 0.0), 6.0));
        let (a, b) = m.beats_in_pos_range(10.0, 12.5).unwrap();
        assert!(close(a, 7.0) && close(b, 10.5), "{} {}", a, b);
    }

    #[test]
    fn speeds() {
        // 120 BPM: 1 beat = 0.5 s. Speed 2x en 2 beats desde el beat 4, luego 0.5x instantáneo en 8,
        // y 1x en 1 segundo desde el beat 12.
        let mut td = td("0=120", "", "", "", 0.0);
        td.speeds = parse_speeds("0=1=0=0,4=2=2=0,8=0.5=0=0,12=1=1=1");
        let t = td.build_table();
        let m = ScrollMap::new(&td, &t);
        let at = |b: f64| m.speed_at(b, t.time_at_beat(b));
        assert!(close(at(2.0), 1.0));
        assert!(close(at(4.0), 1.0));
        assert!(close(at(5.0), 1.5)); // a mitad de la transición
        assert!(close(at(6.0), 2.0));
        assert!(close(at(7.0), 2.0));
        assert!(close(at(8.0), 0.5));
        assert!(close(at(13.0), 0.75)); // 0.5 s de 1 s
        assert!(close(at(15.0), 1.0));
        // Sin segmentos: siempre 1.
        let none = TimingData::default();
        assert!(close(ScrollMap::new(&none, &none.build_table()).speed_at(3.0, 1.5), 1.0));
    }

    #[test]
    fn roundtrip_formats() {
        let b = parse_bpms("0.000000=195.000000\n,4.500000=97.500000\n");
        assert_eq!(write_bpms(&b), "0.000000=195.000000\n,4.500000=97.500000\n");
    }
}
