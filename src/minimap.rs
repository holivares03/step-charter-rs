//! Minimapa: el chart completo a escala fija en una columna a la derecha, con un
//! recuadro que marca la parte visible del notefield. Si el chart es más largo que
//! el panel, el minimapa se desplaza en proporción al avance (como en ArrowVortex),
//! así que al llegar al final del chart se ve el final del minimapa.
//! Modo "notas" o "densidad".

use egui::{pos2, Align2, Color32, FontId, Mesh, Painter, Rect, Stroke};

use crate::noteskin;
use crate::simfile::notes::NoteKind;
use crate::simfile::style::{Arrow, PlayerMode};
use crate::simfile::timing::{row_to_beat, TimingTable};
use crate::simfile::Chart;

/// Ancho máximo de cada columna en el minimapa (px).
const MAX_LANE_W: f32 = 14.0;
/// Alto mínimo del recuadro de la vista (px).
const MIN_VIEW_H: f32 = 12.0;

pub struct MinimapInput<'a> {
    pub chart: &'a Chart,
    pub table: &'a TimingTable,
    /// Eje en segundos (vista por tiempo) o en beats (vista por filas).
    pub time_axis: bool,
    pub reverse: bool,
    /// Rango completo del eje (inicio y fin del chart).
    pub range: (f64, f64),
    /// Parte visible del notefield, en unidades del eje.
    pub view: Option<(f64, f64)>,
    pub cursor: f64,
    /// Filas de la región seleccionada.
    pub selection: Option<(i32, i32)>,
    pub density: bool,
    /// Píxeles por unidad del eje; `None` = ajustar todo el chart al alto del panel.
    pub scale: Option<f64>,
}

/// Conversión entre posiciones del eje y coordenadas y del minimapa.
#[derive(Clone, Copy)]
pub struct MinimapMap {
    pub rect: Rect,
    lo: f64,
    hi: f64,
    /// Píxeles por unidad del eje.
    scale: f64,
    /// Desplazamiento del contenido (px) cuando no cabe en el panel.
    offset: f64,
    reverse: bool,
}

impl MinimapMap {
    fn content_h(&self) -> f64 {
        (self.hi - self.lo) * self.scale
    }

    /// Distancia en px desde el borde de inicio del panel (arriba, o abajo si es reverse).
    fn local(&self, y: f32) -> f64 {
        if self.reverse {
            (self.rect.bottom() - y) as f64
        } else {
            (y - self.rect.top()) as f64
        }
    }

    pub fn y_of(&self, p: f64) -> f32 {
        let d = ((p - self.lo) * self.scale - self.offset) as f32;
        if self.reverse {
            self.rect.bottom() - d
        } else {
            self.rect.top() + d
        }
    }

    pub fn p_of(&self, y: f32) -> f64 {
        self.lo + (self.local(y) + self.offset) / self.scale
    }

    /// Nueva posición del cursor (en unidades del eje) para que el recuadro de la
    /// vista quede centrado en `y`. Funciona como una barra de scroll: depende sólo
    /// de `y`, así el contenido no "salta" bajo el mouse mientras se arrastra.
    pub fn cursor_for_pointer(&self, y: f32, view: Option<(f64, f64)>, cursor: f64) -> f64 {
        let Some((v0, v1)) = view else { return self.p_of(y) };
        let view_len = v1 - v0;
        let view_px = (view_len * self.scale).max(MIN_VIEW_H as f64);
        let panel = self.rect.height() as f64;
        let start = self.local(y) - view_px * 0.5;
        let new_v0 = if self.content_h() <= panel || panel <= view_px {
            self.lo + start / self.scale
        } else {
            let progress = (start / (panel - view_px)).clamp(0.0, 1.0);
            self.lo + progress * ((self.hi - self.lo) - view_len).max(0.0)
        };
        new_v0 + (cursor - v0)
    }
}

/// Eje del minimapa: el rango del chart más lo que la vista muestra antes del cursor (el
/// espacio sobre los receptores) y después de él. Así, con el recuadro de la vista en el tope
/// el cursor queda en el inicio del chart, y con el recuadro en el fondo, en el final.
fn axis_range(range: (f64, f64), view: Option<(f64, f64)>, cursor: f64) -> (f64, f64) {
    let (lo, hi) = range;
    let Some((v0, v1)) = view else { return range };
    // Límite por si la vista es enorme (scrolls casi en 0 en la vista de juego).
    let max = (hi - lo).max(0.0) * 0.5;
    (lo - (cursor - v0).clamp(0.0, max), hi + (v1 - cursor).clamp(0.0, max))
}

fn axis(inp: &MinimapInput, row: i32) -> f64 {
    if inp.time_axis {
        inp.table.time_at_row(row)
    } else {
        row_to_beat(row)
    }
}

fn density_color(t: f32) -> Color32 {
    let lerp = |a: (f32, f32, f32), b: (f32, f32, f32), k: f32| {
        Color32::from_rgb((a.0 + (b.0 - a.0) * k) as u8, (a.1 + (b.1 - a.1) * k) as u8, (a.2 + (b.2 - a.2) * k) as u8)
    };
    let green = (80.0, 200.0, 90.0);
    let yellow = (240.0, 210.0, 60.0);
    let red = (240.0, 70.0, 60.0);
    if t < 0.5 {
        lerp(green, yellow, t * 2.0)
    } else {
        lerp(yellow, red, (t - 0.5) * 2.0)
    }
}

pub fn draw(painter: &Painter, rect: Rect, inp: &MinimapInput) -> MinimapMap {
    painter.rect_filled(rect, 0.0, Color32::from_rgb(14, 14, 18));
    let (chart_lo, mut chart_hi) = inp.range;
    if chart_hi - chart_lo < 1e-6 {
        chart_hi = chart_lo + 1.0;
    }
    let (lo, hi) = axis_range((chart_lo, chart_hi), inp.view, inp.cursor);
    let inner = rect.shrink2(egui::vec2(4.0, 6.0));
    let panel = inner.height() as f64;
    let scale = inp.scale.unwrap_or(panel / (hi - lo)).max(1e-6);
    let content = (hi - lo) * scale;
    // Desplazamiento proporcional al avance de la vista.
    let offset = match inp.view {
        Some((v0, v1)) if content > panel => {
            let view_len = v1 - v0;
            let view_px = (view_len * scale).max(MIN_VIEW_H as f64);
            let scrollable = (hi - lo) - view_len;
            let progress = if scrollable > 0.0 { ((v0 - lo) / scrollable).clamp(0.0, 1.0) } else { 0.0 };
            // Posición del recuadro en el panel: de arriba a abajo según el avance.
            let rect_start = progress * (panel - view_px).max(0.0);
            ((v0 - lo) * scale - rect_start).clamp(0.0, content - panel)
        }
        _ => 0.0,
    };
    let map = MinimapMap { rect: inner, lo, hi, scale, offset, reverse: inp.reverse };
    let chart = inp.chart;
    let num_cols = chart.num_cols.max(1);

    // Rango visible del eje dentro del panel.
    let (pa, pb) = (map.p_of(inner.top()), map.p_of(inner.bottom()));
    let (p_min, p_max) = (pa.min(pb), pa.max(pb));

    // Carriles: ancho máximo por columna y centrados, para que en single no se peguen.
    let lane_w = (inner.width() / num_cols as f32).min(MAX_LANE_W);
    let lanes_left = inner.center().x - lane_w * num_cols as f32 * 0.5;
    let mark_w = (lane_w * 0.68).max(1.0);
    let lane_cx = |c: usize| lanes_left + (c as f32 + 0.5) * lane_w;
    let content_top = map.y_of(chart_lo).min(map.y_of(chart_hi)).max(inner.top());
    let content_bot = map.y_of(chart_lo).max(map.y_of(chart_hi)).min(inner.bottom());

    // Fondo tenue de cada carril (sólo donde hay chart, y no en modo densidad).
    for c in (0..num_cols).filter(|_| !inp.density) {
        let cx = lane_cx(c);
        painter.rect_filled(
            Rect::from_min_max(pos2(cx - mark_w * 0.5, content_top), pos2(cx + mark_w * 0.5, content_bot)),
            0.0,
            Color32::from_white_alpha(7),
        );
    }

    // Separador entre pads (doubles).
    let pad = chart.style.pad_cols;
    if !inp.density && pad > 0 && num_cols > pad && num_cols % pad == 0 {
        let mut c = pad;
        while c < num_cols {
            let x = lanes_left + c as f32 * lane_w;
            painter.line_segment([pos2(x, content_top), pos2(x, content_bot)], Stroke::new(1.0f32, Color32::from_white_alpha(35)));
            c += pad;
        }
    }

    // Región seleccionada.
    if let Some((r0, r1)) = inp.selection {
        let (y0, y1) = (map.y_of(axis(inp, r0)), map.y_of(axis(inp, r1)));
        let r = Rect::from_min_max(pos2(inner.left(), y0.min(y1) - 1.0), pos2(inner.right(), y0.max(y1) + 1.0));
        painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(70, 130, 255, 60));
    }

    if inp.density {
        draw_density(painter, &map, inp);
    } else {
        let coop = chart.players > 1 || chart.style.mode != PlayerMode::Single;
        // Si una columna tiene más notas que píxeles de alto, marcas de 1 px para que no se fundan.
        let mut per_col = vec![0usize; num_cols];
        for n in chart.notes.iter().filter(|n| (n.col as usize) < num_cols) {
            per_col[n.col as usize] += 1;
        }
        let busiest = per_col.iter().copied().max().unwrap_or(0) as f64;
        let half_h = if busiest > content * 0.6 { 0.5 } else { 1.0 };
        let visible = |a: f64, b: f64| a.max(b) >= p_min && a.min(b) <= p_max;

        let mut mesh = Mesh::default();
        // Cuerpos de holds/rolls (debajo de las cabezas).
        for n in chart.notes.iter().filter(|n| n.is_long() && (n.col as usize) < num_cols) {
            let (a, b) = (axis(inp, n.row), axis(inp, n.end_row));
            if !visible(a, b) {
                continue;
            }
            let (ya, yb) = (map.y_of(a), map.y_of(b));
            let cx = lane_cx(n.col as usize);
            let w = (mark_w * 0.4).max(1.0);
            let base = if coop && n.player > 0 { noteskin::player_color(n.player) } else { arrow_color(chart, n.col as usize) };
            let c = if n.kind == NoteKind::Roll { Color32::from_rgb(90, 200, 110) } else { base };
            mesh.add_colored_rect(
                Rect::from_min_max(pos2(cx - w * 0.5, ya.min(yb)), pos2(cx + w * 0.5, ya.max(yb))),
                Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 100),
            );
        }
        for n in chart.notes.iter().filter(|n| (n.col as usize) < num_cols) {
            let p = axis(inp, n.row);
            if !visible(p, p) {
                continue;
            }
            let y = map.y_of(p);
            let cx = lane_cx(n.col as usize);
            let c = match n.kind {
                NoteKind::Mine => Color32::from_rgb(150, 60, 60),
                NoteKind::Special(_) | NoteKind::AutoKeysound => Color32::from_rgb(150, 100, 200),
                _ if coop && n.player > 0 => noteskin::player_color(n.player),
                _ => arrow_color(chart, n.col as usize),
            };
            let c = if n.fake { Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 90) } else { c };
            // Las minas, más angostas para distinguirlas.
            let w = if n.kind == NoteKind::Mine { mark_w * 0.5 } else { mark_w };
            mesh.add_colored_rect(Rect::from_min_max(pos2(cx - w * 0.5, y - half_h), pos2(cx + w * 0.5, y + half_h)), c);
        }
        painter.add(egui::Shape::mesh(mesh));
    }

    // Cursor.
    let yc = map.y_of(inp.cursor);
    painter.line_segment([pos2(rect.left(), yc), pos2(rect.right(), yc)], Stroke::new(1.0f32, Color32::from_rgb(240, 220, 90)));

    // Recuadro de la vista actual.
    if let Some((v0, v1)) = inp.view {
        let (y0, y1) = (map.y_of(v0), map.y_of(v1));
        let (mut top, mut bot) = (y0.min(y1), y0.max(y1));
        if bot - top < MIN_VIEW_H {
            // Alto mínimo, anclado al inicio de la vista (igual que el cálculo del desplazamiento).
            if inp.reverse {
                top = bot - MIN_VIEW_H;
            } else {
                bot = top + MIN_VIEW_H;
            }
        }
        let r = Rect::from_min_max(pos2(rect.left() + 1.0, top), pos2(rect.right() - 1.0, bot));
        painter.rect_filled(r, 2.0, Color32::from_white_alpha(45));
        painter.rect_stroke(r, 2.0, Stroke::new(2.0f32, Color32::from_rgba_unmultiplied(255, 255, 255, 235)), egui::StrokeKind::Inside);
    }

    // Degradado en los bordes cuando hay más chart arriba / abajo.
    if content > panel {
        let more_before = offset > 0.5;
        let more_after = offset < content - panel - 0.5;
        let (top_more, bot_more) = if inp.reverse { (more_after, more_before) } else { (more_before, more_after) };
        let fade = |edge: f32, inward: f32| {
            let mut m = Mesh::default();
            let r = Rect::from_min_max(pos2(rect.left(), edge.min(inward)), pos2(rect.right(), edge.max(inward)));
            let dark = Color32::from_rgba_unmultiplied(14, 14, 18, 220);
            let clear = Color32::from_rgba_unmultiplied(14, 14, 18, 0);
            let (ct, cb) = if edge < inward { (dark, clear) } else { (clear, dark) };
            m.colored_vertex(r.left_top(), ct);
            m.colored_vertex(r.right_top(), ct);
            m.colored_vertex(r.right_bottom(), cb);
            m.colored_vertex(r.left_bottom(), cb);
            m.add_triangle(0, 1, 2);
            m.add_triangle(0, 2, 3);
            painter.add(egui::Shape::mesh(m));
        };
        if top_more {
            fade(rect.top(), rect.top() + 14.0);
        }
        if bot_more {
            fade(rect.bottom(), rect.bottom() - 14.0);
        }
    }
    map
}

fn arrow_color(chart: &Chart, col: usize) -> Color32 {
    noteskin::arrow_color(chart.style.arrows.get(col).copied().unwrap_or(Arrow::Generic))
}

/// Barras de densidad (notas por segundo), suavizadas. La escala de color es la del
/// chart completo, así no cambia al desplazarse.
fn draw_density(painter: &Painter, map: &MinimapMap, inp: &MinimapInput) {
    let inner = map.rect;
    let bucket_px = 2.0f64;
    let content = map.content_h();
    let nb = ((content / bucket_px).ceil() as usize).clamp(1, 200_000);
    let mut counts = vec![0f64; nb];
    let mut rows: Vec<(u8, i32)> = Vec::new();
    for n in &inp.chart.notes {
        if n.fake || matches!(n.kind, NoteKind::Mine | NoteKind::Special(_) | NoteKind::AutoKeysound) {
            continue;
        }
        rows.push((n.player, n.row));
    }
    // Un salto cuenta como una sola pisada.
    rows.sort_unstable();
    rows.dedup();
    let unit = (map.hi - map.lo) / nb as f64; // unidades del eje por cubeta
    for (_, r) in rows {
        let b = ((axis(inp, r) - map.lo) / unit).floor();
        if b >= 0.0 && (b as usize) < nb {
            counts[b as usize] += 1.0;
        }
    }
    // Notas por segundo de cada cubeta.
    let nps: Vec<f64> = counts
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let pa = map.lo + i as f64 * unit;
            let secs = if inp.time_axis { unit } else { (inp.table.time_at_beat(pa + unit) - inp.table.time_at_beat(pa)).abs() };
            if secs > 1e-6 {
                c / secs
            } else {
                0.0
            }
        })
        .collect();
    // Suavizado con una ventana de ~1 segundo (al menos 5 cubetas).
    let unit_secs = if inp.time_axis { unit } else { unit * 0.5 };
    let half = ((0.5 / unit_secs.max(1e-6)) as usize).clamp(2, 200);
    let smooth: Vec<f64> = (0..nb)
        .map(|i| {
            let a = i.saturating_sub(half);
            let b = (i + half + 1).min(nb);
            nps[a..b].iter().sum::<f64>() / (b - a) as f64
        })
        .collect();
    let max = smooth.iter().cloned().fold(0.0, f64::max);
    if max <= 0.0 {
        return;
    }
    let mut mesh = Mesh::default();
    for (i, v) in smooth.iter().enumerate() {
        if *v <= 0.0 {
            continue;
        }
        let (ya, yb) = (map.y_of(map.lo + i as f64 * unit), map.y_of(map.lo + (i + 1) as f64 * unit));
        if ya.max(yb) < inner.top() || ya.min(yb) > inner.bottom() {
            continue;
        }
        let t = (*v / max) as f32;
        mesh.add_colored_rect(
            Rect::from_min_max(pos2(inner.left(), ya.min(yb)), pos2(inner.left() + t * inner.width(), ya.max(yb))),
            density_color(t),
        );
    }
    painter.add(egui::Shape::mesh(mesh));
    painter.text(
        pos2(inner.right(), inner.top()),
        Align2::RIGHT_TOP,
        format!("máx {:.1} NPS", max),
        FontId::proportional(10.0),
        Color32::from_white_alpha(200),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(content_units: f64, scale: f64, offset: f64) -> MinimapMap {
        MinimapMap {
            rect: Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 500.0)),
            lo: 0.0,
            hi: content_units,
            scale,
            offset,
            reverse: false,
        }
    }

    #[test]
    fn y_and_p_are_inverse() {
        let m = map(300.0, 10.0, 1234.0);
        for p in [0.0, 50.5, 123.4, 299.0] {
            assert!((m.p_of(m.y_of(p)) - p).abs() < 1e-3);
        }
    }

    #[test]
    fn pointer_reaches_both_ends() {
        // Canción de 300 s a 10 px/s (3000 px) en un panel de 500 px; vista de 3 s.
        let m = map(300.0, 10.0, 0.0);
        let view = Some((10.0, 13.0));
        let cursor = 11.0; // receptores 1 s después del inicio de la vista
        let top = m.cursor_for_pointer(0.0, view, cursor);
        let bottom = m.cursor_for_pointer(500.0, view, cursor);
        assert!((top - 1.0).abs() < 1e-9, "{}", top); // vista en 0..3
        assert!((bottom - 298.0).abs() < 1e-9, "{}", bottom); // vista en 297..300
    }

    #[test]
    fn pointer_reaches_chart_start_and_end() {
        // Vista de 3 s con los receptores 1 s después de su inicio: el eje va de -1 (vista con
        // el cursor en 0) a 302 (vista con el cursor en 300).
        let view = Some((10.0, 13.0));
        let (lo, hi) = axis_range((0.0, 300.0), view, 11.0);
        assert!((lo + 1.0).abs() < 1e-9 && (hi - 302.0).abs() < 1e-9, "{} {}", lo, hi);
        // Canción larga (se desplaza): el tope y el fondo llevan el cursor al inicio y al final.
        let m = MinimapMap { lo, hi, ..map(300.0, 10.0, 0.0) };
        let top = m.cursor_for_pointer(0.0, view, 11.0);
        let bottom = m.cursor_for_pointer(500.0, view, 11.0);
        assert!(top.abs() < 1e-9, "tope: {}", top);
        assert!((bottom - 300.0).abs() < 1e-9, "fondo: {}", bottom);
        // Canción corta (cabe en el panel): en los bordes el cursor llega al menos al inicio y
        // al final (después se ajusta a esos límites).
        let (lo, hi) = axis_range((0.0, 30.0), view, 11.0);
        let m = MinimapMap { lo, hi, ..map(0.0, 500.0 / (hi - lo), 0.0) };
        assert!(m.cursor_for_pointer(0.0, view, 11.0) <= 0.0);
        assert!(m.cursor_for_pointer(500.0, view, 11.0) >= 30.0);
        // Sin vista, el eje es el del chart.
        assert_eq!(axis_range((0.0, 300.0), None, 5.0), (0.0, 300.0));
    }

    #[test]
    fn short_chart_does_not_scroll() {
        // 30 s a 10 px/s = 300 px: cabe en el panel; el click va directo a esa posición.
        let m = map(30.0, 10.0, 0.0);
        let c = m.cursor_for_pointer(100.0, Some((0.0, 2.0)), 0.5);
        // Vista de 2 s = 20 px, centrada en y=100 => empieza en 90 px = 9 s; el cursor va 0.5 s después.
        assert!((c - 9.5).abs() < 1e-9, "{}", c);
    }
}
