//! Carga y escritura de archivos .ssc (StepMania 5 / StepF2).

use super::msd;
use super::notes;
use super::style;
use super::timing::{self, TimingData};
use super::{is_timing_key, Chart, SimFormat, Song, Tag, TIMING_KEYS};

/// Aplica un tag de timing a `td`. Devuelve `true` si era un tag de timing.
pub fn apply_timing_tag(td: &mut TimingData, key: &str, value: &str) -> bool {
    match key {
        "OFFSET" => td.offset = value.trim().parse().unwrap_or(0.0),
        "BPMS" => td.bpms = timing::parse_bpms(value),
        "STOPS" | "FREEZES" => td.stops = timing::parse_stops(value),
        "DELAYS" => td.delays = timing::parse_stops(value),
        "WARPS" => td.warps = timing::parse_warps(value),
        "TIMESIGNATURES" => td.time_sigs = timing::parse_time_sigs(value),
        "TICKCOUNTS" => td.tick_counts = timing::parse_ticks(value),
        "COMBOS" => td.combos = timing::parse_combos(value),
        "SPEEDS" => td.speeds = timing::parse_speeds(value),
        "SCROLLS" => td.scrolls = timing::parse_scrolls(value),
        "FAKES" => td.fakes = timing::parse_warps(value),
        "LABELS" => td.labels = timing::parse_labels(value),
        _ => return false,
    }
    true
}

/// Valor a escribir para un tag de timing: el texto original si representa
/// exactamente los mismos datos, o uno regenerado si cambió.
pub fn timing_tag_value(td: &TimingData, key: &str, raw: Option<&str>) -> String {
    if let Some(raw) = raw {
        let mut probe = td.clone();
        apply_timing_tag(&mut probe, key, raw);
        probe.sort();
        let mut cur = td.clone();
        cur.sort();
        if probe == cur {
            return raw.to_string();
        }
    }
    match key {
        "OFFSET" => format!("{:.6}", td.offset),
        "BPMS" => timing::write_bpms(&td.bpms),
        "STOPS" | "FREEZES" => timing::write_stops(&td.stops),
        "DELAYS" => timing::write_stops(&td.delays),
        "WARPS" => timing::write_warps(&td.warps),
        "TIMESIGNATURES" => timing::write_time_sigs(&td.time_sigs),
        "TICKCOUNTS" => timing::write_ticks(&td.tick_counts),
        "COMBOS" => timing::write_combos(&td.combos),
        "SPEEDS" => timing::write_speeds(&td.speeds),
        "SCROLLS" => timing::write_scrolls(&td.scrolls),
        "FAKES" => timing::write_warps(&td.fakes),
        "LABELS" => timing::write_labels(&td.labels),
        _ => String::new(),
    }
}

fn timing_field_nonempty(td: &TimingData, key: &str) -> bool {
    match key {
        "OFFSET" => td.offset != 0.0,
        "BPMS" => !td.bpms.is_empty(),
        "STOPS" => !td.stops.is_empty(),
        "DELAYS" => !td.delays.is_empty(),
        "WARPS" => !td.warps.is_empty(),
        "TIMESIGNATURES" => !td.time_sigs.is_empty(),
        "TICKCOUNTS" => !td.tick_counts.is_empty(),
        "COMBOS" => !td.combos.is_empty(),
        "SPEEDS" => !td.speeds.is_empty(),
        "SCROLLS" => !td.scrolls.is_empty(),
        "FAKES" => !td.fakes.is_empty(),
        "LABELS" => !td.labels.is_empty(),
        _ => false,
    }
}

struct ChartBuilder {
    tags: Vec<Tag>,
    timing: TimingData,
    own_timing: bool,
}

pub fn finish_chart(
    tags: Vec<Tag>,
    timing: Option<TimingData>,
    notes_text: &str,
    song_timing: &TimingData,
    warnings: &mut Vec<String>,
) -> Chart {
    let get = |k: &str| tags.iter().rev().find(|t| t.key() == k).map(|t| t.value().trim().to_string());
    let steps_type = get("STEPSTYPE").unwrap_or_default();
    let known = style::find(&steps_type).copied();
    let expected_cols = known.map(|s| s.num_cols()).unwrap_or(0);

    let separate_tag = get("SEPARATENOTES").map(|v| v.eq_ignore_ascii_case("YES")).unwrap_or(false);
    let is_routine = steps_type.ends_with("-routine");
    let separated = separate_tag || is_routine;

    let (parsed, cols, blocks, stats) = notes::parse_notes(notes_text, expected_cols, separated);
    let style = known.unwrap_or_else(|| style::for_steps_type(&steps_type, cols));

    let tag_players: u8 = get("PLAYERS").and_then(|p| p.parse().ok()).unwrap_or(0);
    let max_note_player = parsed.iter().map(|n| n.player).max().unwrap_or(0);
    let players = tag_players.max(max_note_player).max(style.default_players).max(if separated { blocks as u8 } else { 1 });

    let mut timing = timing;
    if let Some(td) = timing.as_mut() {
        if td.bpms.is_empty() {
            td.bpms = song_timing.bpms.clone();
            warnings.push(format!("Chart {} tiene timing propio sin #BPMS; se usan los de la canción.", steps_type));
        }
        td.sort();
    }

    Chart {
        tags,
        style,
        players,
        separated,
        timing,
        notes_orig: parsed.clone(),
        notes: parsed,
        num_cols: if expected_cols > 0 { expected_cols } else { cols.max(1) },
        num_blocks: blocks,
        notes_raw: Some(notes_text.to_string()),
        stats,
    }
}

pub fn load_str(text: &str) -> Song {
    let values = msd::parse(text);
    let mut song = Song {
        path: None,
        format: SimFormat::Ssc,
        header: Vec::new(),
        timing: TimingData::default(),
        charts: Vec::new(),
        warnings: Vec::new(),
    };

    let mut current: Option<ChartBuilder> = None;
    for v in values {
        let key = v.key();
        let tag = Tag::from_msd(&v);
        if key == "NOTEDATA" {
            if current.is_some() {
                song.warnings.push("#NOTEDATA sin #NOTES; se descarta el chart incompleto.".into());
            }
            current = Some(ChartBuilder {
                tags: Vec::new(),
                timing: TimingData { offset: song.timing.offset, ..Default::default() },
                own_timing: false,
            });
            continue;
        }
        match current.as_mut() {
            None => {
                apply_timing_tag(&mut song.timing, &key, &tag.value());
                song.header.push(tag);
            }
            Some(cb) => {
                if key == "NOTES" || key == "NOTES2" {
                    let cb = current.take().unwrap();
                    let timing = if cb.own_timing { Some(cb.timing) } else { None };
                    let mut chart = finish_chart(cb.tags, timing, &tag.value(), &song.timing, &mut song.warnings);
                    // Texto original con comentarios, para reescribirlo idéntico si no cambia.
                    chart.notes_raw = Some(tag.source_text());
                    song.charts.push(chart);
                } else {
                    if apply_timing_tag(&mut cb.timing, &key, &tag.value()) {
                        cb.own_timing = true;
                    }
                    cb.tags.push(tag);
                }
            }
        }
    }
    if current.is_some() {
        song.warnings.push("El último #NOTEDATA no tiene #NOTES.".into());
    }
    if song.timing.bpms.is_empty() {
        song.warnings.push("La canción no tiene #BPMS; se usa 120.".into());
        song.timing.bpms.push(timing::BpmSeg { row: 0, bpm: 120.0 });
    }
    song.timing.sort();
    song
}

fn write_tag(out: &mut String, name: &str, value: &str) {
    out.push('#');
    out.push_str(name);
    out.push(':');
    out.push_str(value);
    out.push_str(";\n");
}

fn write_tags(out: &mut String, tags: &[Tag], timing: Option<&TimingData>, inherited_offset: f64) {
    for t in tags {
        let key = t.key();
        // Un chart que dejó de tener timing propio no escribe sus tags de timing viejos.
        if timing.is_none() && is_timing_key(&key) {
            continue;
        }
        let regenerated = match (timing, is_timing_key(&key)) {
            (Some(td), true) => {
                let cur = t.value();
                let v = timing_tag_value(td, &key, Some(&cur));
                if v == cur {
                    None
                } else {
                    Some(v)
                }
            }
            _ => None,
        };
        match regenerated {
            Some(v) => {
                let name = if key == "FREEZES" { "STOPS" } else { t.name.as_str() };
                write_tag(out, name, &msd::escape_value(&v));
            }
            None => write_tag(out, &t.name, &t.source_text()),
        }
    }
    // Tags de timing que no existían pero ahora tienen datos.
    if let Some(td) = timing {
        for k in TIMING_KEYS {
            let present = tags.iter().any(|t| {
                let tk = t.key();
                tk == *k || (*k == "STOPS" && tk == "FREEZES")
            });
            let inherited = *k == "OFFSET" && td.offset == inherited_offset;
            if !present && !inherited && timing_field_nonempty(td, k) {
                write_tag(out, k, &msd::escape_value(&timing_tag_value(td, k, None)));
            }
        }
    }
}

pub fn write_string(song: &Song) -> String {
    let mut out = String::new();
    write_tags(&mut out, &song.header, Some(&song.timing), 0.0);

    for chart in &song.charts {
        out.push_str(&format!(
            "\n//---------------{} - {}----------------\n",
            chart.steps_type(),
            chart.description()
        ));
        write_tag(&mut out, "NOTEDATA", "");
        write_tags(&mut out, &chart.tags, chart.timing.as_ref(), song.timing.offset);

        let notes_text = match (&chart.notes_raw, chart.notes_changed()) {
            (Some(raw), false) => raw.clone(),
            _ => format!("\n{}", notes::write_notes(&chart.notes, chart.num_cols, chart.num_blocks)),
        };
        write_tag(&mut out, "NOTES", &notes_text);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "#VERSION:0.83;\n#TITLE:Prueba;\n#OFFSET:-0.129000;\n#BPMS:0.000000=142.000000;\n#STOPS:;\n\
//--------------- pump-single ----------------\n#NOTEDATA:;\n#STEPSTYPE:pump-single;\n#DESCRIPTION:S17;\n#METER:17;\n\
#NOTES:\n10000\n00000\n00000\n00000\n;\n\
#NOTEDATA:;\n#STEPSTYPE:pump-double-p;\n#SEPARATENOTES:YES;\n#PLAYERS:2;\n#BPMS:0.000000=71.000000;\n\
#NOTES:\n00{110}0000000\n0000000000\n0000000000\n0000000000\n&\n0000000{120}00\n0000000000\n0000000000\n0000000000\n;\n";

    #[test]
    fn load_sample() {
        let s = load_str(SAMPLE);
        assert_eq!(s.title(), "Prueba");
        assert_eq!(s.charts.len(), 2);
        assert_eq!(s.charts[0].style.id, "pump-single");
        assert!(s.charts[0].timing.is_none());
        assert_eq!(s.charts[1].players, 2);
        assert!(s.charts[1].separated);
        let td = s.charts[1].timing.as_ref().unwrap();
        assert_eq!(td.bpms[0].bpm, 71.0);
        assert_eq!(td.offset, -0.129);
    }

    #[test]
    fn roundtrip() {
        let s = load_str(SAMPLE);
        let w = write_string(&s);
        let s2 = load_str(&w);
        assert_eq!(s.header, s2.header);
        assert_eq!(s.timing, s2.timing);
        assert_eq!(s.charts.len(), s2.charts.len());
        for (a, b) in s.charts.iter().zip(s2.charts.iter()) {
            assert_eq!(a.tags, b.tags);
            assert_eq!(a.timing, b.timing);
            assert!(notes::same_notes(&a.notes, &b.notes));
        }
    }

    #[test]
    fn new_song_and_charts_roundtrip() {
        use crate::simfile::{style, Song};
        let mut s = Song::new_for_audio(std::path::Path::new("C:/x/mi cancion.ogg"));
        s.set_params("DISPLAYBPM", vec!["120".into(), "180".into()]);
        s.set("TITLE", "Re:Start // 2");
        let mut c = Chart::new(*style::find("pump-double-p").unwrap());
        c.set("METER", "12");
        c.set_players(3);
        c.notes.push(notes::Note::new(0, 2, notes::NoteKind::Tap));
        s.charts.push(c);
        let mut single = Chart::new(*style::find("pump-single").unwrap());
        assert!(!single.set_style(*style::find("pump-double").unwrap()));
        assert!(single.set_style(*style::find("pump-single-p").unwrap()));
        s.charts.push(single);
        let text = write_string(&s);
        let s2 = load_str(&text);
        assert_eq!(s2.title(), "Re:Start // 2");
        assert_eq!(s2.get("DISPLAYBPM").as_deref(), Some("120:180"));
        assert_eq!(s2.get("MUSIC").as_deref(), Some("mi cancion.ogg"));
        assert_eq!(s2.timing.bpms[0].bpm, 120.0);
        assert_eq!(s2.charts.len(), 2);
        assert_eq!(s2.charts[0].style.id, "pump-double-p");
        assert_eq!(s2.charts[0].players, 3);
        assert_eq!(s2.charts[0].meter(), 12);
        assert_eq!(s2.charts[0].notes.len(), 1);
        assert_eq!(s2.charts[1].style.id, "pump-single-p");
    }

    #[test]
    fn edited_song_timing_is_saved() {
        let mut s = load_str(SAMPLE);
        s.timing.offset = 0.25;
        s.timing.bpms[0].bpm = 150.0;
        s.timing.bpms.push(timing::BpmSeg { row: 192, bpm: 75.0 });
        s.timing.stops.push(timing::StopSeg { row: 96, seconds: 0.5 });
        s.timing.scrolls.push(timing::ScrollSeg { row: 48, ratio: 2.0 });
        let w = write_string(&s);
        let s2 = load_str(&w);
        assert_eq!(s2.timing.offset, 0.25);
        assert_eq!(s2.timing.bpms.len(), 2);
        assert_eq!(s2.timing.bpms[1].bpm, 75.0);
        assert_eq!(s2.timing.stops[0].seconds, 0.5);
        assert_eq!(s2.timing.scrolls[0].ratio, 2.0);
        // El chart con timing propio no cambia.
        assert_eq!(s2.charts[1].timing, s.charts[1].timing);
    }

    #[test]
    fn own_timing_create_and_drop() {
        let mut s = load_str(SAMPLE);
        // Crear timing propio en el chart 0 con un BPM distinto.
        let mut td = s.timing.clone();
        td.bpms[0].bpm = 200.0;
        s.charts[0].timing = Some(td);
        let s2 = load_str(&write_string(&s));
        assert_eq!(s2.charts[0].timing.as_ref().unwrap().bpms[0].bpm, 200.0);
        // Volver al timing de la canción en el chart 1 (que tenía timing propio).
        let mut s3 = s2.clone();
        s3.charts[1].timing = None;
        let s4 = load_str(&write_string(&s3));
        assert!(s4.charts[1].timing.is_none());
        assert_eq!(s4.charts[1].notes.len(), s2.charts[1].notes.len());
    }

    #[test]
    fn modified_notes_are_regenerated() {
        let mut s = load_str(SAMPLE);
        s.charts[0].notes.push(notes::Note::new(48, 4, notes::NoteKind::Tap));
        let w = write_string(&s);
        let s2 = load_str(&w);
        assert_eq!(s2.charts[0].notes.len(), 2);
        assert!(notes::same_notes(&s.charts[0].notes, &s2.charts[0].notes));
        // Los bloques separados se mantienen.
        assert_eq!(s2.charts[1].num_blocks, 2);
    }
}
