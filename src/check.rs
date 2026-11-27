//! Modo `--check <carpeta|archivo>`: carga todos los simfiles, prueba el guardado
//! (ida y vuelta) y muestra un resumen. Sirve para validar el parser contra una
//! biblioteca real de canciones.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::simfile::{self, notes, ssc};

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        out.push(path.to_path_buf());
        return;
    }
    if let Ok(rd) = std::fs::read_dir(path) {
        let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                collect(&p, out);
            } else if p
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("ssc") || e.eq_ignore_ascii_case("sm"))
                .unwrap_or(false)
            {
                out.push(p);
            }
        }
    }
}

pub fn run(target: &str) -> i32 {
    let mut files = Vec::new();
    collect(Path::new(target), &mut files);
    // Si hay .ssc y .sm con el mismo nombre, sólo el .ssc (como StepMania).
    let ssc_set: std::collections::HashSet<PathBuf> = files
        .iter()
        .filter(|p| p.extension().map(|e| e.eq_ignore_ascii_case("ssc")).unwrap_or(false))
        .map(|p| p.with_extension(""))
        .collect();
    files.retain(|p| {
        p.extension().map(|e| e.eq_ignore_ascii_case("ssc")).unwrap_or(false) || !ssc_set.contains(&p.with_extension(""))
    });

    let t0 = std::time::Instant::now();
    let mut ok = 0usize;
    let mut failed = 0usize;
    let mut charts = 0usize;
    let mut by_type: BTreeMap<String, usize> = BTreeMap::new();
    let mut players: BTreeMap<u8, usize> = BTreeMap::new();
    let mut total = notes::ParseStats::default();
    let mut roundtrip_fail = 0usize;
    let mut regen_fail = 0usize;
    let mut own_timing = 0usize;
    let mut warnings = 0usize;
    let mut no_music = 0usize;
    let mut notes_text_changed = 0usize;
    let mut examples: Vec<String> = Vec::new();

    for f in &files {
        let song = match simfile::load(f) {
            Ok(s) => s,
            Err(e) => {
                failed += 1;
                examples.push(e);
                continue;
            }
        };
        ok += 1;
        warnings += song.warnings.len();
        if song.music_path().is_none() {
            no_music += 1;
        }
        for c in &song.charts {
            charts += 1;
            *by_type.entry(c.steps_type()).or_insert(0) += 1;
            *players.entry(c.players).or_insert(0) += 1;
            if c.timing.is_some() {
                own_timing += 1;
            }
            let s = &c.stats;
            total.measures += s.measures;
            total.odd_measure_sizes += s.odd_measure_sizes;
            total.collisions += s.collisions;
            total.orphan_tails += s.orphan_tails;
            total.unclosed_holds += s.unclosed_holds;
            total.unknown_chars += s.unknown_chars;
            total.special_tokens += s.special_tokens;
            total.width_mismatch += s.width_mismatch;
            if s.collisions > 0 && examples.len() < 30 {
                examples.push(format!("colisiones={} en {} [{}]", s.collisions, f.display(), c.short_name()));
            }
        }

        // Ida y vuelta 1: sin cambios (debe conservar el texto original de notas).
        let text = ssc::write_string(&song);
        let again = ssc::load_str(&text);

        // El texto fuente de cada #NOTES (con comentarios) debe quedar idéntico.
        if song.format == simfile::SimFormat::Ssc {
            let notes_src = |t: &str| -> Vec<String> {
                simfile::msd::parse(t).into_iter().filter(|v| v.key() == "NOTES" || v.key() == "NOTES2").map(|v| v.raw).collect()
            };
            if let Ok(orig_text) = simfile::read_text(f) {
                if notes_src(&orig_text) != notes_src(&text) {
                    notes_text_changed += 1;
                    if examples.len() < 30 {
                        examples.push(format!("texto de #NOTES alterado: {}", f.display()));
                    }
                }
            }
        }
        let same = song.header == again.header
            && song.timing == again.timing
            && song.charts.len() == again.charts.len()
            && song.charts.iter().zip(again.charts.iter()).all(|(a, b)| {
                a.tags == b.tags && a.timing == b.timing && notes::same_notes(&a.notes, &b.notes)
            });
        if !same {
            roundtrip_fail += 1;
            if examples.len() < 30 {
                let mut why = String::new();
                if song.header != again.header {
                    for (a, b) in song.header.iter().zip(again.header.iter()) {
                        if a != b {
                            why = format!("header {:?} vs {:?}", a, b);
                            break;
                        }
                    }
                    if why.is_empty() {
                        why = format!("header len {} vs {}", song.header.len(), again.header.len());
                    }
                } else if song.timing != again.timing {
                    why = "timing canción".into();
                } else if song.charts.len() != again.charts.len() {
                    why = format!("charts {} vs {}", song.charts.len(), again.charts.len());
                } else {
                    for (i, (a, b)) in song.charts.iter().zip(again.charts.iter()).enumerate() {
                        if a.tags != b.tags {
                            why = format!("chart {} tags {:?} vs {:?}", i, a.tags.len(), b.tags.len());
                            for (x, y) in a.tags.iter().zip(b.tags.iter()) {
                                if x != y {
                                    why = format!("chart {} tag {:?} vs {:?}", i, x, y);
                                    break;
                                }
                            }
                            break;
                        }
                        if a.timing != b.timing {
                            why = format!("chart {} timing", i);
                            break;
                        }
                        if !notes::same_notes(&a.notes, &b.notes) {
                            why = format!("chart {} notas", i);
                            break;
                        }
                    }
                }
                let why: String = why.chars().take(300).collect();
                examples.push(format!("ida/vuelta distinta: {} -> {}", f.display(), why));
            }
        }

        // Ida y vuelta 2: forzando a regenerar las notas desde el modelo.
        let mut forced = song.clone();
        for c in &mut forced.charts {
            c.notes_raw = None;
        }
        let text2 = ssc::write_string(&forced);
        let again2 = ssc::load_str(&text2);
        let same2 = forced.charts.len() == again2.charts.len()
            && forced.charts.iter().zip(again2.charts.iter()).all(|(a, b)| notes::same_notes(&a.notes, &b.notes));
        if !same2 {
            regen_fail += 1;
            if examples.len() < 30 {
                let bad = forced
                    .charts
                    .iter()
                    .zip(again2.charts.iter())
                    .position(|(a, b)| !notes::same_notes(&a.notes, &b.notes));
                examples.push(format!("regeneración distinta: {} (chart {:?})", f.display(), bad));
            }
        }
    }

    println!("Archivos: {} cargados, {} con error  ({:.2?})", ok, failed, t0.elapsed());
    println!("Charts: {}  (con timing propio: {})", charts, own_timing);
    println!("Por tipo:");
    for (k, v) in &by_type {
        println!("  {:<20} {}", k, v);
    }
    println!("Por jugadores: {:?}", players);
    println!(
        "Medidas: {}  (tamaño no divisor de 192: {}), colisiones: {}, colas huérfanas: {}, holds sin cola: {}",
        total.measures, total.odd_measure_sizes, total.collisions, total.orphan_tails, total.unclosed_holds
    );
    println!(
        "Tokens especiales StepF2: {}, caracteres desconocidos: {}, líneas con ancho distinto: {}",
        total.special_tokens, total.unknown_chars, total.width_mismatch
    );
    println!("Advertencias: {}, canciones sin audio encontrado: {}", warnings, no_music);
    println!("Ida y vuelta sin cambios fallidas: {}   regenerando notas fallidas: {}", roundtrip_fail, regen_fail);
    println!("Archivos .ssc cuyo texto de #NOTES cambiaría al guardar sin editar: {}", notes_text_changed);
    for e in examples.iter().take(30) {
        println!("  - {}", e);
    }
    if failed > 0 || roundtrip_fail > 0 || regen_fail > 0 {
        1
    } else {
        0
    }
}

/// `--debug-regen <archivo>`: muestra la primera diferencia al regenerar las notas.
pub fn debug_regen(path: &str) -> i32 {
    let song = match simfile::load(Path::new(path)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e);
            return 1;
        }
    };
    for (i, c) in song.charts.iter().enumerate() {
        let text = notes::write_notes(&c.notes, c.num_cols, c.num_blocks);
        let (again, _, _, _) = notes::parse_notes(&text, c.num_cols, c.separated);
        if !notes::same_notes(&c.notes, &again) {
            println!("chart {} ({}) : {} notas vs {} regeneradas", i, c.short_name(), c.notes.len(), again.len());
            let mut a = c.notes.clone();
            let mut b = again.clone();
            a.sort_by_key(|n| (n.block, n.row, n.col));
            b.sort_by_key(|n| (n.block, n.row, n.col));
            for (x, y) in a.iter().zip(b.iter()) {
                if x != y {
                    println!("  original:    {:?}\n  regenerada:  {:?}", x, y);
                    break;
                }
            }
            println!("  stats: {:?}", c.stats);
        }
    }
    0
}
