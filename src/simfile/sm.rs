//! Carga de archivos .sm (StepMania 3.9 / 4 / 5). Se guardan luego como .ssc.

use super::msd;
use super::ssc::{apply_timing_tag, finish_chart};
use super::timing::{self, TimingData};
use super::{SimFormat, Song, Tag};

pub fn load_str(text: &str) -> Song {
    let values = msd::parse(text);
    let mut song = Song {
        path: None,
        format: SimFormat::Sm,
        header: Vec::new(),
        timing: TimingData::default(),
        charts: Vec::new(),
        warnings: Vec::new(),
    };

    let mut pending_notes: Vec<Vec<String>> = Vec::new();
    for v in values {
        let key = v.key();
        if key == "NOTES" || key == "NOTES2" {
            pending_notes.push(v.params.clone());
            continue;
        }
        let mut tag = Tag::from_msd(&v);
        if key == "FREEZES" {
            tag.name = "STOPS".into();
        }
        apply_timing_tag(&mut song.timing, &key, &tag.value());
        song.header.push(tag);
    }
    if song.timing.bpms.is_empty() {
        song.timing.bpms.push(timing::BpmSeg { row: 0, bpm: 120.0 });
    }
    song.timing.sort();

    for params in pending_notes {
        // #NOTES:tipo:descripción:dificultad:nivel:radar:notas;
        if params.len() < 6 {
            song.warnings.push(format!("#NOTES con {} parámetros (se esperaban 6); se ignora.", params.len()));
            continue;
        }
        let p = |i: usize| params[i].trim().to_string();
        let tags = vec![
            Tag::new("STEPSTYPE", &p(0)),
            Tag::new("DESCRIPTION", &p(1)),
            Tag::new("DIFFICULTY", &p(2)),
            Tag::new("METER", &p(3)),
            Tag::new("RADARVALUES", &p(4)),
            Tag::new("CREDIT", &p(1)),
        ];
        let notes_text = params[5..].join(":");
        let chart = finish_chart(tags, None, &notes_text, &song.timing, &mut song.warnings);
        song.charts.push(chart);
    }
    song
}

#[cfg(test)]
mod tests {
    #[test]
    fn load_sm() {
        let s = super::load_str(
            "#TITLE:Hola;\n#OFFSET:0.1;\n#BPMS:0=150;\n#NOTES:\n     pump-single:\n     Yo:\n     Hard:\n     9:\n     0,0,0,0,0:\n10000\n00000\n00000\n00000\n;",
        );
        assert_eq!(s.charts.len(), 1);
        assert_eq!(s.charts[0].meter(), 9);
        assert_eq!(s.charts[0].notes.len(), 1);
    }
}
