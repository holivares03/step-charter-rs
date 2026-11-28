//! Dibujo del notefield (la vista principal del editor, como en ArrowVortex).

use egui::{pos2, vec2, Align2, Color32, FontId, Mesh, Painter, Pos2, Rect, Stroke};
use serde::{Deserialize, Serialize};

use crate::audio::Waveform;
use crate::edit::{SegKind, SegRef, Selection};
use crate::noteskin::{self, draw_sheet, draw_sheet_body, draw_sprite, pump_sprite, ExternalSkin, Noteskin};
use crate::simfile::notes::{Note, NoteKind};
use crate::simfile::style::{Arrow, Game, PlayerMode};
use crate::simfile::timing::{row_to_beat, ScrollMap, TimingData, TimingTable, ROWS_PER_BEAT, ROWS_PER_MEASURE};
use crate::simfile::Chart;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewSettings {
    /// Vista basada en tiempo (true) o en filas/beats (false).
    pub time_based: bool,
    /// Vista de juego: en la vista por filas, aplicar los multiplicadores de #SCROLLS
    /// (y de #SPEEDS si `apply_speeds`), como se ve en XSanity.
    pub apply_scrolls: bool,
    /// En la vista de juego, aplicar también #SPEEDS (zoom de toda la pantalla).
    pub apply_speeds: bool,
    /// Zonas de gimmicks sobre el notefield: warps, fakes, stops, delays, speeds y scrolls.
    pub show_gimmick_zones: bool,
    /// Píxeles por beat (en la vista por tiempo: píxeles por segundo = zoom * 2).
    pub zoom: f32,
    /// Escala de las notas (1.0 = 64 px).
    pub note_scale: f32,
    /// Distancia en píxeles de los receptores al borde (arriba, o abajo si es reverse).
    pub receptor_offset: f32,
    pub reverse: bool,
    pub show_waveform: bool,
    pub show_beat_lines: bool,
    pub show_tempo: bool,
    pub show_background: bool,
    pub background_alpha: f32,
    /// Halo de color por jugador en charts co-op.
    pub color_players: bool,
    /// Usar el noteskin vectorial aunque haya texturas.
    pub vector_skin: bool,
    /// Noteskin externo elegido (nombre de su carpeta); vacío = integrado.
    pub noteskin: String,
    /// Carpeta de noteskins pump (vacío = detectar automáticamente).
    pub skins_dir: String,
    /// En charts co-op, cada jugador con el noteskin de `#PRELOADNOTESKIN`.
    pub coop_skins: bool,
    /// Animar notas, holds y receptores con el beat durante la reproducción.
    pub beat_anim: bool,
    /// Minimapa a la derecha.
    pub show_minimap: bool,
    /// Minimapa en modo densidad (notas por segundo) en vez de notas.
    pub minimap_density: bool,
    /// Escala del minimapa en modo notas, en píxeles por segundo (en la vista por filas, por cada 2 beats).
    pub minimap_scale: f32,
    /// Modo notas: ajustar todo el chart al alto del minimapa en vez de desplazarlo.
    pub minimap_fit: bool,
    /// Escala del minimapa en modo densidad (independiente de la de notas).
    pub minimap_density_scale: f32,
    /// Modo densidad: ajustar todo el chart al alto del minimapa.
    pub minimap_density_fit: bool,
}

impl ViewSettings {
    /// Escala del modo de minimapa activo (notas o densidad).
    pub fn minimap_scale_active(&self) -> f32 {
        if self.minimap_density {
            self.minimap_density_scale
        } else {
            self.minimap_scale
        }
    }
    pub fn minimap_scale_mut(&mut self) -> &mut f32 {
        if self.minimap_density {
            &mut self.minimap_density_scale
        } else {
            &mut self.minimap_scale
        }
    }
    /// "Todo el chart en pantalla" del modo de minimapa activo.
    pub fn minimap_fit_active(&self) -> bool {
        if self.minimap_density {
            self.minimap_density_fit
        } else {
            self.minimap_fit
        }
    }
    pub fn minimap_fit_mut(&mut self) -> &mut bool {
        if self.minimap_density {
            &mut self.minimap_density_fit
        } else {
            &mut self.minimap_fit
        }
    }
}

impl Default for ViewSettings {
    fn default() -> Self {
        ViewSettings {
            time_based: true,
            apply_scrolls: false,
            apply_speeds: true,
            show_gimmick_zones: true,
            zoom: 160.0,
            note_scale: 0.75,
            receptor_offset: 140.0,
            reverse: false,
            show_waveform: true,
            show_beat_lines: true,
            show_tempo: true,
            show_background: true,
            background_alpha: 0.25,
            color_players: true,
            vector_skin: false,
            noteskin: String::new(),
            skins_dir: String::new(),
            coop_skins: true,
            beat_anim: true,
            show_minimap: true,
            minimap_density: false,
            minimap_scale: 12.0,
            minimap_fit: false,
            minimap_density_scale: 4.0,
            minimap_density_fit: false,
        }
    }
}

pub struct FieldInput<'a> {
    pub chart: &'a Chart,
    pub timing: &'a TimingData,
    pub table: &'a TimingTable,
    pub cursor_time: f64,
    pub cursor_beat: f64,
    pub snap: i32,
    pub view: &'a ViewSettings,
    pub skin: Option<&'a Noteskin>,
    pub waveform: Option<&'a Waveform>,
    pub playing: bool,
    pub scroll: Option<std::sync::Arc<ScrollMap>>,
    /// Región seleccionada (filas y columnas).
    pub selection: Option<&'a Selection>,
    /// Noteskin externo (XSanity/StepMania), si hay uno elegido.
    pub ext: Option<&'a ExternalSkin>,
    /// Noteskins por jugador en co-op (P1 = índice 0).
    pub player_skins: &'a [std::sync::Arc<ExternalSkin>],
    /// Cajas de tempo seleccionadas.
    pub tempo_sel: &'a [SegRef],
    /// Posición del mouse (para resaltar la caja de tempo bajo él).
    pub hover: Option<Pos2>,
    /// Se está arrastrando un rectángulo de selección: no se dibuja el recuadro de la región
    /// (sólo el del mouse y el borde de las notas seleccionadas).
    pub dragging: bool,
}

/// Caja de un cambio de tempo dibujada a un lado del notefield.
#[derive(Clone, Debug)]
pub struct TempoBox {
    pub kind: SegKind,
    pub row: i32,
    pub rect: Rect,
}

/// Estado de la animación sincronizada con el beat.
#[derive(Clone, Copy)]
struct Anim {
    /// Fracción del beat actual (0..1); 0 si no se anima.
    frac: f32,
    /// Destello que nace en cada beat y se apaga antes del siguiente (0..1).
    pulse: f32,
}

/// Geometría del notefield para convertir posiciones del mouse en (columna, fila).
#[derive(Clone)]
pub struct Layout {
    pub rect: Rect,
    pub base_y: f32,
    pub dir: f32,
    pub left: f32,
    pub col_w: f32,
    pub num_cols: usize,
    pub time_based: bool,
    pub pps: f64,
    pub ppb: f64,
    pub cursor_time: f64,
    pub cursor_beat: f64,
    /// Mapa de #SCROLLS si la vista por filas los aplica.
    pub scroll: Option<std::sync::Arc<ScrollMap>>,
    /// Multiplicador de #SPEEDS en la posición actual (1 fuera de la vista de juego).
    pub speed: f64,
    /// Cajas de tempo visibles, con su rectángulo en pantalla (para seleccionarlas con el mouse).
    pub boxes: Vec<TempoBox>,
    cursor_pos: f64,
}

impl Layout {
    pub fn new(
        rect: Rect,
        num_cols: usize,
        input_view: &ViewSettings,
        pump: bool,
        cursor_time: f64,
        cursor_beat: f64,
        scroll: Option<std::sync::Arc<ScrollMap>>,
    ) -> Self {
        let scroll = if input_view.time_based || !input_view.apply_scrolls { None } else { scroll };
        let cursor_pos = scroll.as_ref().map(|m| m.pos(cursor_beat)).unwrap_or(cursor_beat);
        let speed = match &scroll {
            Some(m) if input_view.apply_speeds => m.speed_at(cursor_beat, cursor_time),
            _ => 1.0,
        };
        let col_w = if pump { 60.0 } else { 64.0 } * input_view.note_scale;
        let field_w = col_w * num_cols as f32;
        let left = rect.center().x - field_w * 0.5;
        let (base_y, dir) = if input_view.reverse {
            (rect.bottom() - input_view.receptor_offset, -1.0)
        } else {
            (rect.top() + input_view.receptor_offset, 1.0)
        };
        Layout {
            rect,
            base_y,
            dir,
            left,
            col_w,
            num_cols,
            time_based: input_view.time_based,
            pps: input_view.zoom as f64 * 2.0,
            ppb: input_view.zoom as f64,
            cursor_time,
            cursor_beat,
            scroll,
            speed,
            boxes: Vec::new(),
            cursor_pos,
        }
    }

    /// Píxeles por beat (o por unidad de scroll) con el multiplicador de #SPEEDS.
    /// Con speed 0 las notas se juntan en los receptores; se deja un mínimo para
    /// poder seguir convirtiendo posiciones del mouse.
    fn ppb_eff(&self) -> f64 {
        let s = if self.speed.abs() < 0.02 { 0.02f64.copysign(self.speed) } else { self.speed };
        self.ppb * s
    }

    pub fn field_width(&self) -> f32 {
        self.col_w * self.num_cols as f32
    }
    pub fn right(&self) -> f32 {
        self.left + self.field_width()
    }
    pub fn col_x(&self, col: usize) -> f32 {
        self.left + (col as f32 + 0.5) * self.col_w
    }
    /// Columna bajo `x`, ajustada a la más cercana si está fuera del notefield.
    pub fn col_at_x_clamped(&self, x: f32) -> usize {
        let c = ((x - self.left) / self.col_w).floor().max(0.0) as usize;
        c.min(self.num_cols.saturating_sub(1))
    }

    pub fn y_of_time(&self, t: f64) -> f32 {
        self.base_y + self.dir * ((t - self.cursor_time) * self.pps) as f32
    }
    pub fn y_of_beat(&self, b: f64) -> f32 {
        match &self.scroll {
            Some(m) => self.base_y + self.dir * ((m.pos(b) - self.cursor_pos) * self.ppb_eff()) as f32,
            None => self.base_y + self.dir * ((b - self.cursor_beat) * self.ppb) as f32,
        }
    }
    pub fn y_of_row(&self, table: &TimingTable, row: i32) -> f32 {
        if self.time_based {
            self.y_of_time(table.time_at_row(row))
        } else {
            self.y_of_beat(row_to_beat(row))
        }
    }

    /// Tiempo correspondiente a una coordenada y (sólo vista por tiempo).
    pub fn time_at_y(&self, y: f32) -> f64 {
        self.cursor_time + ((y - self.base_y) * self.dir) as f64 / self.pps
    }
    pub fn beat_at_y(&self, table: &TimingTable, y: f32) -> f64 {
        let delta = ((y - self.base_y) * self.dir) as f64 / self.ppb;
        if self.time_based {
            table.beat_at_time(self.time_at_y(y))
        } else if let Some(m) = &self.scroll {
            let delta = ((y - self.base_y) * self.dir) as f64 / self.ppb_eff();
            m.beat_at_pos(self.cursor_pos + delta, self.cursor_beat)
        } else {
            self.cursor_beat + delta
        }
    }

    /// Rango de beats visible (min, max).
    pub fn visible_beats(&self, table: &TimingTable) -> (f64, f64) {
        if let (false, Some(m)) = (self.time_based, &self.scroll) {
            let d0 = ((self.rect.top() - self.base_y) * self.dir) as f64 / self.ppb_eff();
            let d1 = ((self.rect.bottom() - self.base_y) * self.dir) as f64 / self.ppb_eff();
            let (p0, p1) = (self.cursor_pos + d0.min(d1), self.cursor_pos + d0.max(d1));
            // Límite para no recorrer rangos enormes cuando el ratio es casi 0.
            return m
                .beats_in_pos_range(p0, p1)
                .map(|(a, b)| (a.max(self.cursor_beat - 2000.0), b.min(self.cursor_beat + 2000.0)))
                .unwrap_or((self.cursor_beat, self.cursor_beat));
        }
        let a = self.beat_at_y(table, self.rect.top());
        let b = self.beat_at_y(table, self.rect.bottom());
        (a.min(b), a.max(b))
    }
}

fn with_alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * a.clamp(0.0, 1.0)) as u8)
}

pub fn draw(painter: &Painter, rect: Rect, inp: &FieldInput) -> Layout {
    let chart = inp.chart;
    let view = inp.view;
    let pump = chart.style.game == Game::Pump;
    let num_cols = chart.num_cols.max(1);
    let mut lay = Layout::new(rect, num_cols, view, pump, inp.cursor_time, inp.cursor_beat, inp.scroll.clone());
    let table = inp.table;
    let scale = view.note_scale;
    let note_size = 64.0 * scale;
    let small_font = FontId::proportional(11.0);

    let field_rect = Rect::from_min_max(pos2(lay.left, rect.top()), pos2(lay.right(), rect.bottom()));
    painter.rect_filled(field_rect.expand2(vec2(6.0, 0.0)), 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 150));

    // --- Waveform -----------------------------------------------------------
    if view.show_waveform && lay.scroll.is_none() {
        if let Some(wf) = inp.waveform {
            let mut mesh = Mesh::default();
            let half = lay.field_width() * 0.5;
            let cx = lay.left + half;
            let color = Color32::from_rgba_unmultiplied(100, 155, 100, 170);
            let step = 2.0f32;
            let mut y = rect.top();
            let time_of = |y: f32| -> f64 {
                if lay.time_based {
                    lay.time_at_y(y)
                } else {
                    table.time_at_beat(lay.beat_at_y(table, y))
                }
            };
            while y < rect.bottom() {
                let (t0, t1) = {
                    let a = time_of(y);
                    let b = time_of(y + step);
                    (a.min(b), a.max(b))
                };
                let (l, r) = wf.range(t0, t1);
                if l > 0.002 || r > 0.002 {
                    let rr = Rect::from_min_max(pos2(cx - l.min(1.0) * half, y), pos2(cx + r.min(1.0) * half, y + step));
                    mesh.add_colored_rect(rr, color);
                }
                y += step;
            }
            painter.add(egui::Shape::mesh(mesh));
        }
    }

    let (b_top, b_bot) = lay.visible_beats(table);
    let row_min = (b_top * ROWS_PER_BEAT as f64).floor() as i32 - 1;
    let row_max = (b_bot * ROWS_PER_BEAT as f64).ceil() as i32 + 1;

    // --- Líneas de compás / beat / snap --------------------------------------
    if view.show_beat_lines {
        // Líneas de snap (tenues) si hay espacio suficiente.
        let snap_rows = (ROWS_PER_MEASURE / inp.snap.max(1)).max(1);
        let cur_row = crate::simfile::timing::beat_to_row(inp.cursor_beat);
        let spacing = (lay.y_of_row(table, cur_row + snap_rows) - lay.y_of_row(table, cur_row)).abs();
        let r0 = row_min.max(0);
        if snap_rows < ROWS_PER_BEAT && spacing > 7.0 {
            let mut r = (r0 / snap_rows) * snap_rows;
            while r <= row_max {
                if r % ROWS_PER_BEAT != 0 {
                    let y = lay.y_of_row(table, r);
                    painter.line_segment([pos2(lay.left, y), pos2(lay.right(), y)], Stroke::new(1.0f32, Color32::from_white_alpha(18)));
                }
                r += snap_rows;
            }
        }

        // Compases según las signaturas de tiempo.
        let mut measure_rows: Vec<(i32, i32)> = Vec::new(); // (fila, número de compás)
        let mut segs: Vec<(i32, i32, i32)> = inp.timing.time_sigs.iter().map(|t| (t.row, t.num, t.den)).collect();
        if segs.first().map(|s| s.0 > 0).unwrap_or(true) {
            segs.insert(0, (0, 4, 4));
        }
        let mut measure_no = 0;
        'segs: for (k, &(start, num, den)) in segs.iter().enumerate() {
            let len = ((ROWS_PER_BEAT * 4 * num.max(1)) / den.max(1)).max(1);
            let end = segs.get(k + 1).map(|s| s.0).unwrap_or(i32::MAX);
            let mut r = start;
            while r < end {
                if r > row_max {
                    break 'segs;
                }
                if r >= row_min {
                    measure_rows.push((r, measure_no));
                }
                r += len;
                measure_no += 1;
            }
        }

        // Beats
        let mut r = (r0 / ROWS_PER_BEAT) * ROWS_PER_BEAT;
        while r <= row_max {
            if !measure_rows.iter().any(|(m, _)| *m == r) {
                let y = lay.y_of_row(table, r);
                painter.line_segment([pos2(lay.left, y), pos2(lay.right(), y)], Stroke::new(1.0f32, Color32::from_white_alpha(45)));
            }
            r += ROWS_PER_BEAT;
        }
        for (r, m) in measure_rows {
            let y = lay.y_of_row(table, r);
            painter.line_segment([pos2(lay.left, y), pos2(lay.right(), y)], Stroke::new(1.5f32, Color32::from_white_alpha(120)));
            painter.text(pos2(lay.left - 8.0, y), Align2::RIGHT_CENTER, format!("{}", m), small_font.clone(), Color32::from_white_alpha(140));
        }
    }

    // Separador entre pads (doubles / couple / routine).
    if num_cols > chart.style.pad_cols && chart.style.pad_cols > 0 && num_cols % chart.style.pad_cols == 0 {
        let mut c = chart.style.pad_cols;
        while c < num_cols {
            let x = lay.left + c as f32 * lay.col_w;
            painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.0f32, Color32::from_white_alpha(40)));
            c += chart.style.pad_cols;
        }
    }

    // --- Zonas de gimmicks ------------------------------------------------------
    if view.show_gimmick_zones {
        draw_gimmick_zones(painter, &lay, table, inp.timing, row_min, row_max);
    }

    // --- Región seleccionada -------------------------------------------------
    if let Some(sel) = inp.selection.filter(|_| !inp.dragging) {
        let y0 = lay.y_of_row(table, sel.r0);
        let y1 = lay.y_of_row(table, sel.r1);
        let pad = note_size * 0.5;
        let x0 = lay.left + sel.c0 as f32 * lay.col_w;
        let x1 = lay.left + (sel.c1 + 1).min(num_cols) as f32 * lay.col_w;
        let r = Rect::from_min_max(pos2(x0, y0.min(y1) - pad), pos2(x1, y0.max(y1) + pad));
        painter.rect_filled(r, 4.0, Color32::from_rgba_unmultiplied(70, 130, 255, 40));
        painter.rect_stroke(r, 4.0, Stroke::new(1.5f32, Color32::from_rgba_unmultiplied(120, 180, 255, 190)), egui::StrokeKind::Inside);
    }

    // --- Segmentos de tempo --------------------------------------------------
    if view.show_tempo {
        lay.boxes = draw_tempo(painter, &lay, table, inp, row_min, row_max, &small_font);
    }

    // --- Receptores ----------------------------------------------------------
    let receptor_y = lay.base_y;
    let flash_window = 0.12;
    let mut flash: Vec<f32> = vec![0.0; num_cols];
    if inp.playing {
        let t = inp.cursor_time;
        let ra = crate::simfile::timing::beat_to_row(table.beat_at_time(t - flash_window)) - 1;
        let rb = crate::simfile::timing::beat_to_row(table.beat_at_time(t)) + 1;
        let start = chart.notes.partition_point(|n| n.row < ra);
        for n in &chart.notes[start..] {
            if n.row > rb {
                break;
            }
            if matches!(n.kind, NoteKind::Mine) || n.fake {
                continue;
            }
            let nt = table.time_at_row(n.row);
            let dt = t - nt;
            if (0.0..flash_window).contains(&dt) && (n.col as usize) < num_cols {
                let v = 1.0 - (dt / flash_window) as f32;
                flash[n.col as usize] = flash[n.col as usize].max(v);
            }
        }
        // Holds activos: brillo constante.
        for n in chart.notes.iter().filter(|n| n.is_long() && !n.fake) {
            if (n.col as usize) < num_cols && table.time_at_row(n.row) <= t && table.time_at_row(n.end_row) >= t {
                flash[n.col as usize] = flash[n.col as usize].max(0.6);
            }
        }
    }
    let use_tex = inp.skin.is_some() && pump && !view.vector_skin;
    let anim = if inp.playing && view.beat_anim {
        let frac = inp.cursor_beat.rem_euclid(1.0) as f32;
        Anim { frac, pulse: (1.0 - frac).powi(3) }
    } else {
        Anim { frac: 0.0, pulse: 0.0 }
    };
    let ext_main = if view.vector_skin { None } else { inp.ext };
    // Noteskin de cada jugador (co-op) o el principal.
    let ext_for = |player: u8| -> Option<&ExternalSkin> {
        if view.vector_skin {
            return None;
        }
        if view.coop_skins && player > 0 {
            if let Some(s) = inp.player_skins.get(player as usize - 1) {
                return Some(s.as_ref());
            }
        }
        ext_main
    };

    for c in 0..num_cols {
        let arrow = chart.style.arrows.get(c).copied().unwrap_or(Arrow::Generic);
        let center = pos2(lay.col_x(c), receptor_y);
        let dst = Rect::from_center_size(center, egui::Vec2::splat(note_size));
        if let Some((spr, mirror)) = ext_main.and_then(|e| e.get(arrow)) {
            match &spr.receptor {
                Some(rec) => {
                    draw_sheet(painter, rec, 0, dst, mirror, false, Color32::WHITE);
                    if anim.pulse > 0.0 && rec.frames() >= 3 {
                        draw_sheet(painter, rec, 2, dst, mirror, false, Color32::from_white_alpha((anim.pulse * 200.0) as u8));
                    }
                    if flash[c] > 0.0 && rec.frames() >= 2 {
                        draw_sheet(painter, rec, 1, dst, mirror, false, Color32::from_white_alpha((flash[c] * 255.0) as u8));
                    }
                }
                None => {
                    if let Some(tap) = &spr.tap {
                        draw_sheet(painter, tap, 0, dst, mirror, false, Color32::from_white_alpha(90 + (flash[c] * 165.0) as u8));
                    }
                }
            }
            continue;
        }
        match (use_tex, inp.skin, pump_sprite(arrow)) {
            (true, Some(skin), Some((sx, mirror))) => {
                draw_sprite(painter, &skin.receptors, Rect::from_min_size(pos2(sx, 0.0), vec2(64.0, 64.0)), dst, mirror, Color32::WHITE);
                let glow = flash[c].max(anim.pulse * 0.45);
                if glow > 0.0 {
                    let gx = match arrow {
                        Arrow::DownLeft | Arrow::DownRight => 0.0,
                        Arrow::UpLeft | Arrow::UpRight => 96.0,
                        _ => 192.0,
                    };
                    let gdst = Rect::from_center_size(center, egui::Vec2::splat(96.0 * scale));
                    draw_sprite(painter, &skin.glow, Rect::from_min_size(pos2(gx, 0.0), vec2(96.0, 96.0)), gdst, mirror, Color32::from_white_alpha((glow * 255.0) as u8));
                }
            }
            _ => {
                let fill = with_alpha(noteskin::arrow_color(arrow), 0.25 + flash[c].max(anim.pulse * 0.4) * 0.6);
                noteskin::draw_vector_note(painter, arrow, center, note_size * 0.9, fill, Color32::from_white_alpha(150));
            }
        }
    }

    // --- Notas -----------------------------------------------------------------
    let max_len = chart.notes.iter().filter(|n| n.is_long()).map(|n| n.end_row - n.row).max().unwrap_or(0);
    let start = chart.notes.partition_point(|n| n.row < row_min - max_len);
    let end = chart.notes.partition_point(|n| n.row <= row_max);
    let visible: &[Note] = &chart.notes[start..end.max(start)];
    let coop = chart.players > 1 || chart.style.mode != PlayerMode::Single;
    let builtin = inp.skin.filter(|_| use_tex);

    // Cuerpos de holds/rolls primero.
    for n in visible.iter().filter(|n| n.is_long()) {
        if n.end_row < row_min || (n.col as usize) >= num_cols {
            continue;
        }
        let arrow = chart.style.arrows.get(n.col as usize).copied().unwrap_or(Arrow::Generic);
        let x = lay.col_x(n.col as usize);
        let y0 = lay.y_of_row(table, n.row);
        let y1 = lay.y_of_row(table, n.end_row);
        let alpha = note_alpha(n, inp.timing);
        draw_hold_body(painter, builtin, ext_for(n.player), anim, arrow, n, x, y0, y1, note_size, alpha);
    }

    // Cabezas, de la última a la primera para que las primeras queden encima.
    for n in visible.iter().rev() {
        if n.row < row_min || (n.col as usize) >= num_cols {
            continue;
        }
        let arrow = chart.style.arrows.get(n.col as usize).copied().unwrap_or(Arrow::Generic);
        let center = pos2(lay.col_x(n.col as usize), lay.y_of_row(table, n.row));
        let alpha = note_alpha(n, inp.timing);
        // Con noteskin propio del jugador no hace falta el halo de color.
        let own_skin = view.coop_skins && n.player > 0 && inp.player_skins.get(n.player as usize - 1).is_some() && !view.vector_skin;
        if coop && view.color_players && n.player > 0 && !own_skin {
            painter.circle_filled(center, note_size * 0.52, with_alpha(noteskin::player_color(n.player), 0.45 * alpha));
        }
        draw_head(painter, builtin, ext_for(n.player), anim, arrow, n, center, note_size, alpha);
        if inp.selection.map(|s| s.contains(n)).unwrap_or(false) {
            let r = Rect::from_center_size(center, egui::Vec2::splat(note_size * 0.96));
            painter.rect_stroke(r, note_size * 0.15, Stroke::new(2.0f32, Color32::from_rgb(130, 210, 255)), egui::StrokeKind::Outside);
        }
        if coop && n.player > 0 {
            let bc = center + vec2(note_size * 0.32, note_size * 0.30);
            let r = (note_size * 0.17).max(6.0);
            painter.circle(bc, r, noteskin::player_color(n.player), Stroke::new(1.0f32, Color32::BLACK));
            painter.text(bc, Align2::CENTER_CENTER, n.player.to_string(), FontId::proportional(r * 1.5), Color32::BLACK);
        }
    }

    // Línea del cursor (en la posición de los receptores).
    let q_color = noteskin::quant_color(inp.snap);
    painter.line_segment(
        [pos2(lay.left - 4.0, receptor_y), pos2(lay.right() + 4.0, receptor_y)],
        Stroke::new(1.0f32, with_alpha(q_color, 0.8)),
    );

    lay
}

fn note_alpha(n: &Note, timing: &TimingData) -> f32 {
    let mut a: f32 = 1.0;
    if n.fake {
        a *= 0.45;
    }
    if timing.is_unjudgable(n.row) {
        a *= 0.5;
    }
    a
}

#[allow(clippy::too_many_arguments)]
fn draw_hold_body(
    painter: &Painter,
    skin: Option<&Noteskin>,
    ext: Option<&ExternalSkin>,
    anim: Anim,
    arrow: Arrow,
    n: &Note,
    x: f32,
    y0: f32,
    y1: f32,
    size: f32,
    alpha: f32,
) {
    let (top, bot) = (y0.min(y1), y0.max(y1));
    let downward = y1 >= y0;
    let roll = n.kind == NoteKind::Roll;
    let mut zigzag = roll;
    match (ext.and_then(|e| e.get(arrow)), skin, pump_sprite(arrow)) {
        (Some((spr, mirror)), _, _) => {
            // Noteskin de XSanity: cuerpo y remate animados con el beat.
            let own_roll = roll && spr.roll_body.is_some();
            zigzag = roll && !own_roll;
            let (body, cap) = if own_roll { (&spr.roll_body, spr.roll_cap.as_ref().or(spr.hold_cap.as_ref())) } else { (&spr.hold_body, spr.hold_cap.as_ref()) };
            let tint = with_alpha(if zigzag { Color32::from_rgb(190, 255, 190) } else { Color32::WHITE }, alpha);
            if let Some(b) = body {
                draw_sheet_body(painter, b, b.frame_at(anim.frac), x, size, top, bot, mirror, tint);
            }
            if let Some(c) = cap {
                // Remate centrado en la fila final (como ArrowVortex), no debajo como en el
                // juego: así el final del hold coincide con la línea de snap donde termina.
                let h = size * c.frame_px.y / c.frame_px.x.max(1.0);
                let r = Rect::from_center_size(pos2(x, if downward { bot } else { top }), vec2(size, h));
                draw_sheet(painter, c, c.frame_at(anim.frac), r, mirror, !downward, tint);
            }
        }
        (None, Some(skin), Some((sx, mirror))) => {
            let tint = with_alpha(if roll { Color32::from_rgb(170, 255, 170) } else { Color32::WHITE }, alpha);
            let body = Rect::from_min_max(pos2(x - size * 0.5, top), pos2(x + size * 0.5, bot));
            if bot - top > 0.5 {
                draw_sprite(painter, &skin.notes, Rect::from_min_max(pos2(sx, 70.0), pos2(sx + 64.0, 122.0)), body, mirror, tint);
            }
            let tail = Rect::from_center_size(pos2(x, if downward { bot } else { top }), egui::Vec2::splat(size));
            draw_sprite(painter, &skin.notes, Rect::from_min_size(pos2(sx, 128.0), vec2(64.0, 64.0)), tail, mirror, tint);
        }
        _ => {
            let w = size * 0.55;
            let body = Rect::from_min_max(pos2(x - w * 0.5, top), pos2(x + w * 0.5, bot));
            let base = noteskin::arrow_color(arrow);
            let fill = if roll { Color32::from_rgb(90, 200, 110) } else { base };
            painter.rect_filled(body, w * 0.2, with_alpha(fill, 0.55 * alpha));
            painter.rect_stroke(body, w * 0.2, Stroke::new(1.0f32, with_alpha(Color32::WHITE, 0.5 * alpha)), egui::StrokeKind::Inside);
        }
    }
    // Brillo con el beat (sólo durante la reproducción).
    if anim.pulse > 0.0 && bot - top > 1.0 {
        let w = size * 0.42;
        let r = Rect::from_min_max(pos2(x - w * 0.5, top), pos2(x + w * 0.5, bot));
        painter.rect_filled(r, w * 0.3, Color32::from_white_alpha((anim.pulse * 85.0 * alpha) as u8));
    }
    if zigzag && bot - top > 4.0 {
        // Zigzag para distinguir rolls.
        let w = size * 0.22;
        let step = (size * 0.25).max(6.0);
        let mut y = top;
        let mut left = true;
        let stroke = Stroke::new(2.0f32, with_alpha(Color32::from_rgb(30, 120, 40), alpha));
        while y + step <= bot {
            let (xa, xb) = if left { (x - w, x + w) } else { (x + w, x - w) };
            painter.line_segment([pos2(xa, y), pos2(xb, y + step)], stroke);
            y += step;
            left = !left;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_head(painter: &Painter, skin: Option<&Noteskin>, ext: Option<&ExternalSkin>, anim: Anim, arrow: Arrow, n: &Note, center: Pos2, size: f32, alpha: f32) {
    let dst = Rect::from_center_size(center, egui::Vec2::splat(size));
    let white = with_alpha(Color32::WHITE, alpha);
    let label = |painter: &Painter, txt: &str, color: Color32| {
        painter.text(center, Align2::CENTER_CENTER, txt, FontId::proportional((size * 0.42).max(9.0)), color);
    };
    match n.kind {
        NoteKind::Mine => match skin {
            Some(skin) => draw_sprite(painter, &skin.notes, Rect::from_min_size(pos2(192.0, 0.0), vec2(64.0, 64.0)), dst, false, white),
            None => {
                painter.circle(center, size * 0.32, with_alpha(Color32::from_rgb(60, 60, 60), alpha), Stroke::new(2.0f32, with_alpha(Color32::from_rgb(230, 60, 60), alpha)));
                painter.circle_filled(center, size * 0.12, with_alpha(Color32::from_rgb(230, 60, 60), alpha));
            }
        },
        NoteKind::Special(c) => {
            painter.circle(center, size * 0.36, with_alpha(Color32::from_rgb(120, 70, 170), alpha), Stroke::new(1.5f32, white));
            label(painter, &c.to_string(), white);
        }
        _ => {
            let ext_tap = ext.and_then(|e| e.get(arrow)).and_then(|(spr, mirror)| spr.tap.as_ref().map(|t| (t, mirror)));
            match (ext_tap, skin, pump_sprite(arrow)) {
                (Some((tap, mirror)), _, _) => {
                    let q = crate::simfile::notes::quantization_of_row(n.row);
                    draw_sheet(painter, tap, tap.note_frame(anim.frac, q), dst, mirror, false, white)
                }
                (None, Some(skin), Some((sx, mirror))) => {
                    draw_sprite(painter, &skin.notes, Rect::from_min_size(pos2(sx, 0.0), vec2(64.0, 64.0)), dst, mirror, white);
                }
                _ => {
                    let fill = with_alpha(noteskin::arrow_color(arrow), alpha);
                    noteskin::draw_vector_note(painter, arrow, center, size * 0.9, fill, with_alpha(Color32::WHITE, 0.9 * alpha));
                }
            }
            match n.kind {
                NoteKind::Lift => label(painter, "L", Color32::from_rgb(255, 255, 255)),
                NoteKind::AutoKeysound => label(painter, "K", Color32::from_rgb(255, 255, 255)),
                _ => {}
            }
            if n.fake {
                label(painter, "F", Color32::from_rgb(255, 255, 255));
            }
        }
    }
}

pub const WARP_COLOR: Color32 = Color32::from_rgb(200, 110, 240);
pub const FAKE_COLOR: Color32 = Color32::from_rgb(170, 170, 170);
pub const STOP_COLOR: Color32 = Color32::from_rgb(240, 90, 90);
pub const DELAY_COLOR: Color32 = Color32::from_rgb(240, 160, 70);
pub const SPEED_COLOR: Color32 = Color32::from_rgb(90, 200, 240);

/// Color de la franja de #SCROLLS según el ratio (None si es 1: scroll normal).
pub fn scroll_color(ratio: f64) -> Option<Color32> {
    if (ratio - 1.0).abs() < 1e-9 {
        None
    } else if ratio < 0.0 {
        Some(Color32::from_rgb(235, 90, 205)) // hacia atrás
    } else if ratio.abs() < 1e-9 {
        Some(Color32::from_rgb(240, 220, 80)) // detenido
    } else if ratio < 1.0 {
        Some(with_alpha(Color32::from_rgb(80, 190, 170), 0.55 + 0.45 * (1.0 - ratio as f32)))
    } else {
        Some(with_alpha(Color32::from_rgb(90, 235, 235), (0.45 + (ratio as f32 - 1.0) * 0.2).min(1.0)))
    }
}

/// Rayas a 45° dentro de un rectángulo.
fn hatch_rect(painter: &Painter, rect: Rect, color: Color32, spacing: f32) {
    let h = rect.height();
    if h <= 0.0 || rect.width() <= 0.0 {
        return;
    }
    let stroke = Stroke::new(1.0f32, color);
    // Cada línea va de (x, abajo) a (x + h, arriba); se recorta a [left, right].
    let mut x = rect.left() - h + (rect.top() - rect.left()).rem_euclid(spacing);
    while x < rect.right() {
        let u0 = ((rect.left() - x) / h).clamp(0.0, 1.0);
        let u1 = ((rect.right() - x) / h).clamp(0.0, 1.0);
        if u1 > u0 {
            let p = |u: f32| pos2(x + u * h, rect.bottom() - u * h);
            painter.line_segment([p(u0), p(u1)], stroke);
        }
        x += spacing;
    }
}

/// Zonas de gimmicks: bandas sobre el notefield para warps, fakes, stops y delays, y
/// franjas a los lados para las transiciones de #SPEEDS (derecha) y los #SCROLLS (izquierda).
fn draw_gimmick_zones(painter: &Painter, lay: &Layout, table: &TimingTable, td: &TimingData, row_min: i32, row_max: i32) {
    let (l, r) = (lay.left, lay.right());
    let (top, bottom) = (lay.rect.top() - 4.0, lay.rect.bottom() + 4.0);
    let rows_of = |beats: f64| (beats * ROWS_PER_BEAT as f64).round() as i32;
    let overlaps = |a: i32, b: i32| a.max(b) >= row_min && a.min(b) <= row_max;
    let on_screen = |y: f32| y >= top && y <= bottom;

    // Banda entre dos alturas; si casi no tiene alto (un warp en la vista por tiempo,
    // un stop en la vista por filas), una línea gruesa.
    let band = |y0: f32, y1: f32, fill: Color32, edge: Color32, hatch: bool| {
        let (a, b) = (y0.min(y1), y0.max(y1));
        if b < top || a > bottom {
            return;
        }
        if b - a < 3.0 {
            painter.line_segment([pos2(l - 6.0, a), pos2(r + 6.0, a)], Stroke::new(3.0f32, edge));
            return;
        }
        let rect = Rect::from_min_max(pos2(l, a.max(top)), pos2(r, b.min(bottom)));
        painter.rect_filled(rect, 0.0, fill);
        if hatch {
            hatch_rect(painter, rect, with_alpha(edge, 0.30), 14.0);
        }
        let s = Stroke::new(1.5f32, edge);
        for y in [a, b] {
            if on_screen(y) {
                painter.line_segment([pos2(l - 6.0, y), pos2(r + 6.0, y)], s);
            }
        }
    };

    for w in &td.warps {
        let r1 = w.row + rows_of(w.beats);
        if overlaps(w.row, r1) {
            band(lay.y_of_row(table, w.row), lay.y_of_row(table, r1), with_alpha(WARP_COLOR, 0.16), WARP_COLOR, false);
        }
    }
    for f in &td.fakes {
        let r1 = f.row + rows_of(f.beats);
        if overlaps(f.row, r1) {
            band(lay.y_of_row(table, f.row), lay.y_of_row(table, r1), with_alpha(FAKE_COLOR, 0.10), FAKE_COLOR, true);
        }
    }
    // Stops y delays sólo ocupan espacio en la vista por tiempo.
    let pause = |row: i32, secs: f64, before: bool, color: Color32| {
        if !overlaps(row, row) {
            return;
        }
        let (y0, y1) = if lay.time_based {
            let t = table.time_at_row(row);
            // Un delay ocurre antes de la nota; un stop, después.
            if before {
                (lay.y_of_time(t - secs), lay.y_of_time(t))
            } else {
                (lay.y_of_time(t), lay.y_of_time(t + secs))
            }
        } else {
            let y = lay.y_of_row(table, row);
            (y, y)
        };
        band(y0, y1, with_alpha(color, 0.16), color, false);
    };
    for s in &td.stops {
        pause(s.row, s.seconds, false, STOP_COLOR);
    }
    for d in &td.delays {
        pause(d.row, d.seconds, true, DELAY_COLOR);
    }

    // #SPEEDS: marca en cada cambio y franja durante la transición (a la derecha).
    for s in &td.speeds {
        let end = if s.duration <= 0.0 {
            s.row
        } else if s.unit == 1 {
            crate::simfile::timing::beat_to_row(table.beat_at_time(table.time_at_row(s.row) + s.duration))
        } else {
            s.row + rows_of(s.duration)
        };
        if !overlaps(s.row, end) {
            continue;
        }
        let y0 = lay.y_of_row(table, s.row);
        if end > s.row {
            let y1 = lay.y_of_row(table, end);
            let rect = Rect::from_min_max(pos2(r + 1.0, y0.min(y1).max(top)), pos2(r + 6.0, y0.max(y1).min(bottom)));
            if rect.height() > 0.0 {
                painter.rect_filled(rect, 2.0, with_alpha(SPEED_COLOR, 0.85));
            }
        }
        if on_screen(y0) {
            painter.line_segment([pos2(r - 10.0, y0), pos2(r + 8.0, y0)], Stroke::new(2.0f32, SPEED_COLOR));
        }
    }

    // #SCROLLS: franja a la izquierda coloreada según el ratio.
    let mut scrolls: Vec<(i32, f64)> = td.scrolls.iter().map(|s| (s.row, s.ratio)).collect();
    scrolls.sort_by_key(|s| s.0);
    for (i, &(row, ratio)) in scrolls.iter().enumerate() {
        let end = scrolls.get(i + 1).map(|s| s.0).unwrap_or(row_max + ROWS_PER_MEASURE);
        let Some(color) = scroll_color(ratio) else { continue };
        if !overlaps(row, end) {
            continue;
        }
        let (y0, y1) = (lay.y_of_row(table, row), lay.y_of_row(table, end.min(row_max + ROWS_PER_MEASURE)));
        let (a, b) = (y0.min(y1).max(top), y0.max(y1).min(bottom));
        if b - a < 2.0 {
            // Scroll 0 en la vista de juego: todo el tramo cae en la misma línea.
            if on_screen(y0) {
                painter.line_segment([pos2(l - 7.0, y0), pos2(l - 1.0, y0)], Stroke::new(3.0f32, color));
            }
        } else {
            painter.rect_filled(Rect::from_min_max(pos2(l - 6.0, a), pos2(l - 1.0, b)), 2.0, color);
        }
    }
}

/// Color de la caja de cada tipo de cambio de tempo.
pub fn seg_color(kind: SegKind) -> Color32 {
    match kind {
        SegKind::Bpm => Color32::from_rgb(110, 220, 110),
        SegKind::Stop => STOP_COLOR,
        SegKind::Delay => DELAY_COLOR,
        SegKind::Warp => WARP_COLOR,
        SegKind::TimeSig => Color32::from_rgb(230, 230, 230),
        SegKind::Ticks => Color32::from_rgb(150, 190, 240),
        SegKind::Combo => Color32::from_rgb(240, 200, 120),
        SegKind::Speed => SPEED_COLOR,
        SegKind::Scroll => Color32::from_rgb(90, 230, 200),
        SegKind::Fake => FAKE_COLOR,
        SegKind::Label => Color32::from_rgb(240, 230, 110),
    }
}

/// Dibuja las cajas de los cambios de tempo visibles y devuelve su posición en pantalla.
fn draw_tempo(painter: &Painter, lay: &Layout, table: &TimingTable, inp: &FieldInput, row_min: i32, row_max: i32, font: &FontId) -> Vec<TempoBox> {
    let td = inp.timing;
    // Columnas a la izquierda (BPM, stops, ...) y a la derecha (speeds, scrolls, ...).
    let mut left: Vec<(i32, SegKind, String)> = Vec::new();
    let mut right: Vec<(i32, SegKind, String)> = Vec::new();
    let in_range = |r: i32| r >= row_min && r <= row_max;
    let mut add = |row: i32, kind: SegKind, text: String| {
        if kind.left_side() {
            left.push((row, kind, text));
        } else {
            right.push((row, kind, text));
        }
    };
    for s in td.bpms.iter().filter(|s| in_range(s.row)) {
        add(s.row, SegKind::Bpm, format!("{:.3}", s.bpm));
    }
    for s in td.stops.iter().filter(|s| in_range(s.row)) {
        add(s.row, SegKind::Stop, format!("Stop {:.3}", s.seconds));
    }
    for s in td.delays.iter().filter(|s| in_range(s.row)) {
        add(s.row, SegKind::Delay, format!("Delay {:.3}", s.seconds));
    }
    for s in td.warps.iter().filter(|s| in_range(s.row)) {
        add(s.row, SegKind::Warp, format!("Warp {:.3}", s.beats));
    }
    for s in td.time_sigs.iter().filter(|s| in_range(s.row)) {
        add(s.row, SegKind::TimeSig, format!("{}/{}", s.num, s.den));
    }
    for s in td.tick_counts.iter().filter(|s| in_range(s.row)) {
        add(s.row, SegKind::Ticks, format!("Ticks {}", s.ticks));
    }
    for s in td.combos.iter().filter(|s| in_range(s.row)) {
        let t = if s.hit == s.miss { format!("x{}", s.hit) } else { format!("x{}/{}", s.hit, s.miss) };
        add(s.row, SegKind::Combo, t);
    }
    for s in td.speeds.iter().filter(|s| in_range(s.row)) {
        let unit = if s.unit == 1 { "s" } else { "b" };
        add(s.row, SegKind::Speed, format!("Speed {:.2}x {:.2}{}", s.ratio, s.duration, unit));
    }
    for s in td.scrolls.iter().filter(|s| in_range(s.row)) {
        add(s.row, SegKind::Scroll, format!("Scroll {:.2}x", s.ratio));
    }
    for s in td.fakes.iter().filter(|s| in_range(s.row)) {
        add(s.row, SegKind::Fake, format!("Fake {:.3}", s.beats));
    }
    for s in td.labels.iter().filter(|s| in_range(s.row)) {
        add(s.row, SegKind::Label, s.text.clone());
    }

    // Etiquetas en la misma fila se apilan horizontalmente.
    let mut boxes = Vec::new();
    let mut place = |items: Vec<(i32, SegKind, String)>, is_left: bool| {
        let mut last_row = i32::MIN;
        let mut x = 0.0;
        for (row, kind, text) in items {
            let y = lay.y_of_row(table, row);
            if row != last_row {
                x = if is_left { lay.left - 34.0 } else { lay.right() + 12.0 };
                last_row = row;
            }
            let align = if is_left { Align2::RIGHT_CENTER } else { Align2::LEFT_CENTER };
            let selected = inp.tempo_sel.contains(&(kind, row));
            let r = tempo_box(painter, pos2(x, y), align, &text, seg_color(kind), font, selected, inp.hover);
            x = if is_left { r.left() - 4.0 } else { r.right() + 4.0 };
            boxes.push(TempoBox { kind, row, rect: r });
        }
    };
    left.sort_by_key(|i| i.0);
    right.sort_by_key(|i| i.0);
    place(left, true);
    place(right, false);
    boxes
}

/// Caja de un cambio de tempo: seleccionada (relleno de su color y borde blanco) o con el
/// mouse encima (borde más grueso).
#[allow(clippy::too_many_arguments)]
fn tempo_box(painter: &Painter, pos: Pos2, align: Align2, text: &str, color: Color32, font: &FontId, selected: bool, hover: Option<Pos2>) -> Rect {
    let galley = painter.layout_no_wrap(text.to_string(), font.clone(), Color32::WHITE);
    let size = galley.size() + vec2(8.0, 2.0);
    let rect = align.anchor_size(pos, size);
    let hovered = hover.map(|p| rect.contains(p)).unwrap_or(false);
    if selected {
        let fill = Color32::from_rgb(color.r() / 2, color.g() / 2, color.b() / 2);
        painter.rect_filled(rect.expand(1.0), 3.0, fill);
        painter.rect_stroke(rect.expand(1.0), 3.0, Stroke::new(2.0f32, Color32::WHITE), egui::StrokeKind::Outside);
        painter.galley(rect.min + vec2(4.0, 1.0), galley, Color32::WHITE);
    } else {
        let fill = if hovered { Color32::from_rgb(45, 45, 52) } else { with_alpha(Color32::from_rgb(20, 20, 24), 0.85) };
        painter.rect_filled(rect, 3.0, fill);
        painter.rect_stroke(rect, 3.0, Stroke::new(if hovered { 2.0f32 } else { 1.0 }, color), egui::StrokeKind::Inside);
        painter.galley(rect.min + vec2(4.0, 1.0), galley, color);
    }
    rect
}
