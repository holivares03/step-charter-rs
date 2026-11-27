//! Noteskin: texturas de flechas pump (las del noteskin "pump" de ArrowVortex,
//! incrustadas en el ejecutable) y dibujo vectorial de respaldo para otros estilos.

use egui::{pos2, vec2, Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, TextureHandle, Vec2};

use crate::simfile::style::Arrow;

const NOTES_PNG: &[u8] = include_bytes!("../assets/noteskins/pump/tex notes.png");
const RECEPTORS_PNG: &[u8] = include_bytes!("../assets/noteskins/pump/tex receptors.png");
const GLOW_PNG: &[u8] = include_bytes!("../assets/noteskins/pump/tex glow.png");

pub struct Tex {
    pub handle: TextureHandle,
    pub size: Vec2,
}

pub struct Noteskin {
    pub notes: Tex,
    pub receptors: Tex,
    pub glow: Tex,
}

fn load_png(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<Tex> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    let handle = ctx.load_texture(name, color, egui::TextureOptions::LINEAR);
    Some(Tex { handle, size: vec2(size[0] as f32, size[1] as f32) })
}

/// Columna del atlas pump (x en píxeles) y si hay que espejar horizontalmente.
pub fn pump_sprite(arrow: Arrow) -> Option<(f32, bool)> {
    match arrow {
        Arrow::DownLeft => Some((0.0, false)),
        Arrow::UpLeft => Some((64.0, false)),
        Arrow::Center => Some((128.0, false)),
        Arrow::UpRight => Some((64.0, true)),
        Arrow::DownRight => Some((0.0, true)),
        _ => None,
    }
}

impl Noteskin {
    pub fn load(ctx: &egui::Context) -> Option<Self> {
        Some(Noteskin {
            notes: load_png(ctx, "skin-notes", NOTES_PNG)?,
            receptors: load_png(ctx, "skin-receptors", RECEPTORS_PNG)?,
            glow: load_png(ctx, "skin-glow", GLOW_PNG)?,
        })
    }
}

/// Dibuja una región `src` (en píxeles de la textura) en `dst`.
pub fn draw_sprite(painter: &Painter, tex: &Tex, src: Rect, dst: Rect, mirror: bool, tint: Color32) {
    let (mut u0, u1) = (src.min.x / tex.size.x, src.max.x / tex.size.x);
    let (v0, v1) = (src.min.y / tex.size.y, src.max.y / tex.size.y);
    let mut u1 = u1;
    if mirror {
        std::mem::swap(&mut u0, &mut u1);
    }
    let mut mesh = Mesh::with_texture(tex.handle.id());
    let uv = Rect { min: pos2(u0, v0), max: pos2(u1, v1) };
    mesh.add_rect_with_uv(dst, uv, tint);
    painter.add(Shape::mesh(mesh));
}

fn rotate(p: Vec2, angle: f32) -> Vec2 {
    let (s, c) = angle.sin_cos();
    vec2(p.x * c - p.y * s, p.x * s + p.y * c)
}

fn arrow_angle(arrow: Arrow) -> Option<f32> {
    use std::f32::consts::PI;
    Some(match arrow {
        Arrow::Up => 0.0,
        Arrow::Right => PI * 0.5,
        Arrow::Down => PI,
        Arrow::Left => PI * 1.5,
        Arrow::UpLeft => -PI * 0.25,
        Arrow::UpRight => PI * 0.25,
        Arrow::DownRight => PI * 0.75,
        Arrow::DownLeft => PI * 1.25,
        _ => return None,
    })
}

/// Flecha vectorial (cabeza triangular + tallo) o cuadrado/círculo para centro/genérica.
pub fn draw_vector_note(painter: &Painter, arrow: Arrow, center: Pos2, size: f32, fill: Color32, outline: Color32) {
    let s = size * 0.5;
    match arrow_angle(arrow) {
        Some(angle) => {
            let tf = |x: f32, y: f32| center + rotate(vec2(x * s, y * s), angle);
            let head = vec![tf(0.0, -0.9), tf(0.85, 0.05), tf(-0.85, 0.05)];
            let shaft = vec![tf(-0.38, 0.0), tf(0.38, 0.0), tf(0.38, 0.85), tf(-0.38, 0.85)];
            let stroke = Stroke::new((size * 0.06).max(1.0), outline);
            painter.add(Shape::convex_polygon(shaft.clone(), fill, stroke));
            painter.add(Shape::convex_polygon(head.clone(), fill, stroke));
            // Tapa la línea interna entre cabeza y tallo.
            let inner = vec![tf(-0.33, 0.12), tf(0.33, 0.12), tf(0.33, -0.02), tf(-0.33, -0.02)];
            painter.add(Shape::convex_polygon(inner, fill, Stroke::NONE));
        }
        None => match arrow {
            Arrow::Center => {
                let r = Rect::from_center_size(center, Vec2::splat(size * 0.78));
                painter.rect(r, size * 0.12, fill, Stroke::new((size * 0.06).max(1.0), outline), egui::StrokeKind::Middle);
            }
            _ => {
                painter.circle(center, size * 0.38, fill, Stroke::new((size * 0.06).max(1.0), outline));
            }
        },
    }
}

/// Color base vectorial por flecha (estilo Pump: azul abajo, rojo arriba, amarillo centro).
pub fn arrow_color(arrow: Arrow) -> Color32 {
    match arrow {
        Arrow::DownLeft | Arrow::DownRight => Color32::from_rgb(70, 130, 240),
        Arrow::UpLeft | Arrow::UpRight => Color32::from_rgb(230, 80, 80),
        Arrow::Center => Color32::from_rgb(235, 200, 60),
        Arrow::Left | Arrow::Right => Color32::from_rgb(220, 90, 200),
        Arrow::Up | Arrow::Down => Color32::from_rgb(80, 170, 240),
        Arrow::Generic => Color32::from_rgb(180, 180, 180),
    }
}

/// Color de cada jugador en charts co-op.
pub fn player_color(player: u8) -> Color32 {
    match player {
        1 => Color32::from_rgb(255, 105, 90),
        2 => Color32::from_rgb(80, 165, 255),
        3 => Color32::from_rgb(110, 220, 110),
        4 => Color32::from_rgb(255, 210, 70),
        5 => Color32::from_rgb(200, 120, 255),
        6 => Color32::from_rgb(255, 140, 200),
        7 => Color32::from_rgb(90, 230, 220),
        8 => Color32::from_rgb(255, 160, 60),
        9 => Color32::from_rgb(210, 210, 210),
        _ => Color32::from_rgb(200, 200, 200),
    }
}

/// Color por cuantización (estilo ITG/ArrowVortex) para la línea de snap.
pub fn quant_color(q: i32) -> Color32 {
    match q {
        4 => Color32::from_rgb(230, 60, 60),
        8 => Color32::from_rgb(60, 110, 240),
        12 => Color32::from_rgb(170, 60, 230),
        16 => Color32::from_rgb(240, 220, 60),
        24 => Color32::from_rgb(230, 80, 200),
        32 => Color32::from_rgb(240, 150, 50),
        48 => Color32::from_rgb(60, 220, 230),
        64 => Color32::from_rgb(80, 200, 80),
        _ => Color32::from_rgb(160, 160, 160),
    }
}

// ===========================================================================
// Noteskins externos de StepMania / XSanity (carpeta NoteSkins/pump/<nombre>)
// ===========================================================================

use std::path::{Path, PathBuf};

/// Una hoja de sprites animada (`... 3x2.png` = 3 columnas × 2 filas de cuadros).
pub struct Sheet {
    pub tex: TextureHandle,
    pub cols: u32,
    pub rows: u32,
    /// Tamaño de cada cuadro en píxeles.
    pub frame_px: Vec2,
    /// Hoja con un color por cuantización: cada fila es un ritmo (4th, 8th, 12th...)
    /// y las columnas son los cuadros de la animación (noteskins "rhythm").
    pub note_color: bool,
}

/// Fila de color de StepMania para una cuantización (4th=0, 8th=1, ... 64th=7, 192nd=8).
pub fn quant_row(q: i32) -> u32 {
    match q {
        4 => 0,
        8 => 1,
        12 => 2,
        16 => 3,
        24 => 4,
        32 => 5,
        48 => 6,
        64 => 7,
        _ => 8,
    }
}

impl Sheet {
    pub fn frames(&self) -> u32 {
        (self.cols * self.rows).max(1)
    }

    /// Coordenadas UV de un cuadro (se repite cíclicamente).
    pub fn uv(&self, frame: u32) -> Rect {
        let f = frame % self.frames();
        let (c, r) = (f % self.cols, f / self.cols);
        let (w, h) = (1.0 / self.cols as f32, 1.0 / self.rows as f32);
        Rect::from_min_size(pos2(c as f32 * w, r as f32 * h), vec2(w, h))
    }

    /// Cuadro según la fracción del beat: una vuelta completa de la animación por beat.
    pub fn frame_at(&self, beat_frac: f32) -> u32 {
        ((beat_frac.clamp(0.0, 0.9999) * self.frames() as f32) as u32).min(self.frames() - 1)
    }

    /// Cuadro de una nota: en hojas por color, la fila según la cuantización de la nota.
    pub fn note_frame(&self, beat_frac: f32, quant: i32) -> u32 {
        if !self.note_color {
            return self.frame_at(beat_frac);
        }
        let col = ((beat_frac.clamp(0.0, 0.9999) * self.cols as f32) as u32).min(self.cols - 1);
        let row = quant_row(quant).min(self.rows - 1);
        row * self.cols + col
    }
}

/// Dibuja un cuadro de una hoja de sprites. `flip_v` invierte verticalmente (reverse).
pub fn draw_sheet(painter: &Painter, sheet: &Sheet, frame: u32, dst: Rect, mirror: bool, flip_v: bool, tint: Color32) {
    let mut uv = sheet.uv(frame);
    if mirror {
        std::mem::swap(&mut uv.min.x, &mut uv.max.x);
    }
    if flip_v {
        std::mem::swap(&mut uv.min.y, &mut uv.max.y);
    }
    let mut mesh = Mesh::with_texture(sheet.tex.id());
    mesh.add_rect_with_uv(dst, uv, tint);
    painter.add(Shape::mesh(mesh));
}

/// Cuerpo de hold: si el cuadro es una franja fina se estira; si es un azulejo, se repite.
#[allow(clippy::too_many_arguments)]
pub fn draw_sheet_body(painter: &Painter, sheet: &Sheet, frame: u32, x: f32, width: f32, top: f32, bot: f32, mirror: bool, tint: Color32) {
    if bot - top <= 0.5 {
        return;
    }
    let uv = sheet.uv(frame);
    let (mut u0, mut u1) = (uv.min.x, uv.max.x);
    if mirror {
        std::mem::swap(&mut u0, &mut u1);
    }
    let mut mesh = Mesh::with_texture(sheet.tex.id());
    if sheet.frame_px.y <= 4.0 {
        let v = (uv.min.y + uv.max.y) * 0.5;
        mesh.add_rect_with_uv(
            Rect::from_min_max(pos2(x - width * 0.5, top), pos2(x + width * 0.5, bot)),
            Rect { min: pos2(u0, v), max: pos2(u1, v) },
            tint,
        );
    } else {
        // Azulejo: alto proporcional al ancho; el último trozo se recorta.
        let seg = (width * sheet.frame_px.y / sheet.frame_px.x).max(4.0);
        let mut y = top;
        let mut n = 0;
        while y < bot && n < 2000 {
            let h = (bot - y).min(seg);
            let v1 = uv.min.y + (uv.max.y - uv.min.y) * (h / seg);
            mesh.add_rect_with_uv(
                Rect::from_min_max(pos2(x - width * 0.5, y), pos2(x + width * 0.5, y + h)),
                Rect { min: pos2(u0, uv.min.y), max: pos2(u1, v1) },
                tint,
            );
            y += h;
            n += 1;
        }
    }
    painter.add(Shape::mesh(mesh));
}

/// Sprites de una dirección.
#[derive(Default)]
pub struct ArrowSprites {
    pub tap: Option<Sheet>,
    pub hold_body: Option<Sheet>,
    pub hold_cap: Option<Sheet>,
    pub roll_body: Option<Sheet>,
    pub roll_cap: Option<Sheet>,
    pub receptor: Option<Sheet>,
}

pub struct ExternalSkin {
    pub name: String,
    /// Por dirección pump: ↙ ↖ centro ↗ ↘.
    arrows: [ArrowSprites; 5],
}

const PUMP_DIRS: [&str; 5] = ["DownLeft", "UpLeft", "Center", "UpRight", "DownRight"];

fn pump_index(arrow: Arrow) -> Option<usize> {
    Some(match arrow {
        Arrow::DownLeft => 0,
        Arrow::UpLeft => 1,
        Arrow::Center => 2,
        Arrow::UpRight => 3,
        Arrow::DownRight => 4,
        _ => return None,
    })
}

impl ExternalSkin {
    /// Sprites de una flecha; si falta la dirección se usa la opuesta espejada.
    pub fn get(&self, arrow: Arrow) -> Option<(&ArrowSprites, bool)> {
        let i = pump_index(arrow)?;
        if self.arrows[i].tap.is_some() {
            return Some((&self.arrows[i], false));
        }
        let opposite = [4, 3, 2, 1, 0][i];
        if self.arrows[opposite].tap.is_some() {
            return Some((&self.arrows[opposite], true));
        }
        None
    }
}

/// Columnas×filas de cuadros a partir del nombre (`Tap Note (res 192x128) 3x2.png` → 3×2).
pub fn parse_grid(file_name: &str) -> (u32, u32) {
    let mut s = String::new();
    let mut depth = 0i32;
    for ch in file_name.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ if depth == 0 => s.push(ch),
            _ => {}
        }
    }
    let stem = s.rsplit_once('.').map(|x| x.0).unwrap_or(&s);
    for tok in stem.split_whitespace().rev() {
        if let Some((a, b)) = tok.split_once(['x', 'X']) {
            if let (Ok(a), Ok(b)) = (a.parse::<u32>(), b.parse::<u32>()) {
                if a > 0 && b > 0 && a <= 64 && b <= 64 {
                    return (a, b);
                }
            }
        }
    }
    (1, 1)
}

/// Archivo que corresponde a un elemento ("DownLeft Tap Note"), sin distinguir mayúsculas.
/// Prefiere las versiones animadas (con NxM en el nombre).
fn match_file(files: &[String], element: &str) -> Option<String> {
    let p = element.to_ascii_lowercase();
    let mut c: Vec<&String> = files
        .iter()
        .filter(|f| {
            let l = f.to_ascii_lowercase();
            l.ends_with(".png") && l.starts_with(&p) && matches!(l[p.len()..].chars().next(), Some(' ') | Some('.') | Some('('))
        })
        .collect();
    c.sort_by_key(|f| (parse_grid(f) == (1, 1), f.len()));
    c.first().map(|s| s.to_string())
}

fn list_files(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok()).filter_map(|e| e.file_name().to_str().map(|s| s.to_string())).collect())
        .unwrap_or_default()
}

fn load_sheet(ctx: &egui::Context, path: &Path) -> Option<Sheet> {
    let img = image::open(path).ok()?.to_rgba8();
    let (w, h) = (img.width(), img.height());
    let name = path.file_name()?.to_str()?;
    let (cols, rows) = parse_grid(name);
    let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
    let tex = ctx.load_texture(path.to_string_lossy(), color, egui::TextureOptions::LINEAR);
    Some(Sheet { tex, cols, rows, frame_px: vec2(w as f32 / cols as f32, h as f32 / rows as f32), note_color: false })
}

/// Busca un elemento en una carpeta, siguiendo redirecciones `.redir` (hasta 3 saltos).
fn find_element(ctx: &egui::Context, dir: &Path, files: &[String], element: &str, depth: u32) -> Option<Sheet> {
    if let Some(f) = match_file(files, element) {
        return load_sheet(ctx, &dir.join(f));
    }
    // Imágenes que usan los scripts Lua de algunos noteskins: "_DownLeft Tap Note 6x8.png".
    if let Some(f) = match_file(files, &format!("_{}", element)) {
        return load_sheet(ctx, &dir.join(f));
    }
    if depth >= 3 {
        return None;
    }
    let redir = format!("{}.redir", element.to_ascii_lowercase());
    let file = files.iter().find(|f| f.to_ascii_lowercase() == redir)?;
    let target = std::fs::read_to_string(dir.join(file)).ok()?;
    let target = target.lines().next()?.trim().to_string();
    if target.is_empty() || target.eq_ignore_ascii_case(element) {
        return None;
    }
    find_element(ctx, dir, files, &target, depth + 1)
}

/// Valor de una clave de `metrics.ini` (la primera que aparezca, en cualquier sección).
fn metric(dir: &Path, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("metrics.ini")).ok()?;
    text.lines().find_map(|l| {
        let l = l.split("//").next()?.trim();
        let (k, v) = l.split_once('=')?;
        if k.trim().eq_ignore_ascii_case(key) {
            let v = v.trim();
            (!v.is_empty()).then(|| v.to_string())
        } else {
            None
        }
    })
}

/// `FallbackNoteSkin=` de `metrics.ini`.
fn fallback_of(dir: &Path) -> Option<String> {
    metric(dir, "FallbackNoteSkin")
}

/// Carga un noteskin pump de StepMania/XSanity.
pub fn load_external(ctx: &egui::Context, dir: &Path) -> Result<ExternalSkin, String> {
    let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string();
    let files = list_files(dir);
    if files.is_empty() {
        return Err(format!("No se pudo leer el noteskin {}", dir.display()));
    }
    // Carpetas de respaldo (para cuerpos, colas y receptores que falten).
    let mut fallbacks: Vec<PathBuf> = Vec::new();
    if let Some(parent) = dir.parent() {
        if let Some(fb) = fallback_of(dir) {
            fallbacks.push(parent.join(fb));
        }
        fallbacks.push(parent.join("default"));
    }
    fallbacks.retain(|p| p.is_dir() && p != dir);
    let fb_files: Vec<(PathBuf, Vec<String>)> = fallbacks.iter().map(|p| (p.clone(), list_files(p))).collect();

    // Noteskins "rhythm": las flechas tienen una fila de color por cuantización.
    let note_color = metric(dir, "TapNoteNoteColorTextureCoordSpacingY")
        .and_then(|v| v.parse::<f32>().ok())
        .map(|v| v > 0.0)
        .unwrap_or(false);
    let mut arrows: [ArrowSprites; 5] = Default::default();
    for (i, d) in PUMP_DIRS.iter().enumerate() {
        let get = |el: &str, use_fallback: bool| -> Option<Sheet> {
            let element = format!("{} {}", d, el);
            find_element(ctx, dir, &files, &element, 0).or_else(|| {
                if !use_fallback {
                    return None;
                }
                fb_files.iter().find_map(|(p, f)| find_element(ctx, p, f, &element, 0))
            })
        };
        let tap = get("Tap Note", false).map(|mut t| {
            t.note_color = note_color && t.rows > 1;
            t
        });
        if tap.is_none() {
            continue;
        }
        arrows[i] = ArrowSprites {
            tap,
            hold_body: get("Hold Body Active", true),
            hold_cap: get("Hold BottomCap Active", true),
            // Los rolls sólo del mismo noteskin; si no hay, se dibujan como holds.
            roll_body: get("Roll Body Active", false),
            roll_cap: get("Roll BottomCap Active", false),
            receptor: get("Ready Receptor", true),
        };
    }
    if arrows.iter().all(|a| a.tap.is_none()) {
        return Err(format!("{} no parece un noteskin pump (no tiene 'Tap Note')", name));
    }
    Ok(ExternalSkin { name, arrows })
}

/// Carpeta donde se copian los noteskins importados de un simulador
/// (`%APPDATA%\Step Charter\data\noteskins\pump` en Windows).
pub fn imported_skins_dir() -> Option<PathBuf> {
    eframe::storage_dir(crate::APP_NAME).map(|d| d.join("noteskins").join("pump"))
}

/// Carpeta `noteskins/pump` junto al ejecutable (instalación portátil) o en alguna carpeta
/// superior (en desarrollo el .exe está en `target/release`), o en la carpeta de trabajo.
pub fn portable_skins_dir() -> Option<PathBuf> {
    let mut bases: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        bases.extend(exe.ancestors().skip(1).take(4).map(|p| p.to_path_buf()));
    }
    if let Ok(cwd) = std::env::current_dir() {
        bases.push(cwd);
    }
    bases.into_iter().map(|b| b.join("noteskins").join("pump")).find(|p| p.is_dir())
}

/// Raíz del simulador de una canción: la carpeta que contiene a `Songs`.
fn sim_root_of(song_path: &Path) -> Option<PathBuf> {
    song_path
        .ancestors()
        .find(|a| a.file_name().and_then(|n| n.to_str()).map(|n| n.eq_ignore_ascii_case("Songs")).unwrap_or(false))
        .and_then(|a| a.parent())
        .map(|p| p.to_path_buf())
}

/// Carpeta de noteskins pump del simulador de la canción abierta (hermana de `Songs`).
pub fn guess_skins_dir(song_path: Option<&Path>) -> Option<PathBuf> {
    resolve_skins_dir(&sim_root_of(song_path?)?)
}

/// Carpeta de noteskins pump a partir de una carpeta elegida: la raíz del simulador
/// (con `NoteSkins/pump`), la carpeta `NoteSkins`, o una que ya contenga noteskins.
pub fn resolve_skins_dir(dir: &Path) -> Option<PathBuf> {
    [dir.join("NoteSkins").join("pump"), dir.join("pump"), dir.to_path_buf()]
        .into_iter()
        .find(|p| p.is_dir() && !list_skins(p).is_empty())
}

/// La carpeta es un noteskin (tiene al menos un "Tap Note").
fn is_skin(dir: &Path) -> bool {
    list_files(dir).iter().any(|f| f.to_ascii_lowercase().contains("tap note"))
}

/// Qué copiar al importar desde una carpeta elegida: la carpeta de noteskins pump y los
/// nombres de los noteskins. Si la carpeta es un solo noteskin, sólo ese. Se suman los
/// noteskins de respaldo que usan (`FallbackNoteSkin` y `default`).
pub fn import_plan(picked: &Path) -> Option<(PathBuf, Vec<String>)> {
    let (base, mut names) = if is_skin(picked) {
        let name = picked.file_name()?.to_str()?.to_string();
        (picked.parent()?.to_path_buf(), vec![name])
    } else {
        let base = resolve_skins_dir(picked)?;
        let names = list_skins(&base);
        (base, names)
    };
    let mut i = 0;
    while i < names.len() {
        let dir = base.join(&names[i]);
        for fb in fallback_of(&dir).into_iter().chain(["default".to_string()]) {
            if base.join(&fb).is_dir() && !names.iter().any(|n| n.eq_ignore_ascii_case(&fb)) {
                names.push(fb);
            }
        }
        i += 1;
    }
    Some((base, names))
}

/// Copia una carpeta completa (sin seguir enlaces) y suma los bytes copiados.
fn copy_dir(src: &Path, dst: &Path, bytes: &mut u64) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&entry.path(), &to, bytes)?;
        } else if kind.is_file() {
            *bytes += std::fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

/// Resultado de una importación: noteskins y bytes copiados.
pub struct ImportResult {
    pub skins: usize,
    pub bytes: u64,
}

/// Copia los noteskins `names` de `base` a `dest`. Un noteskin que ya estaba en `dest`
/// se reemplaza entero (así no quedan archivos viejos de otra versión).
pub fn import_skins(base: &Path, names: &[String], dest: &Path) -> Result<ImportResult, String> {
    std::fs::create_dir_all(dest).map_err(|e| format!("No se pudo crear {}: {}", dest.display(), e))?;
    let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    if canon(base) == canon(dest) {
        return Err("Esa carpeta es la de los noteskins ya importados.".to_string());
    }
    let mut bytes = 0;
    for name in names {
        let to = dest.join(name);
        if to.exists() {
            std::fs::remove_dir_all(&to).map_err(|e| format!("No se pudo reemplazar {}: {}", to.display(), e))?;
        }
        copy_dir(&base.join(name), &to, &mut bytes).map_err(|e| format!("Error al copiar {}: {}", name, e))?;
    }
    Ok(ImportResult { skins: names.len(), bytes })
}

/// ¿El nombre de una carpeta parece el de un simulador?
fn looks_like_sim(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["xsanity", "stepf2", "stepp1", "stepmania", "itgmania", "outfox", "pump"].iter().any(|k| n.contains(k))
}

/// Busca simuladores instalados con noteskins pump (XSanity, StepF2/P1, StepMania,
/// ITGmania, OutFox…): los de las canciones dadas, y carpetas con nombre de simulador en
/// la raíz de cada unidad, en `Games`/`Juegos`, en Archivos de programa y en las carpetas
/// de datos del usuario. Devuelve (carpeta del simulador, carpeta de noteskins pump, cantidad).
pub fn find_simulators(songs: &[PathBuf]) -> Vec<(PathBuf, PathBuf, usize)> {
    let mut roots: Vec<PathBuf> = songs.iter().filter_map(|s| sim_root_of(s)).collect();
    let mut parents: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        for letter in b'C'..=b'Z' {
            let drive = PathBuf::from(format!("{}:\\", letter as char));
            if drive.is_dir() {
                for sub in ["Games", "Juegos", "Program Files", "Program Files (x86)"] {
                    parents.push(drive.join(sub));
                }
                parents.push(drive);
            }
        }
        for var in ["APPDATA", "LOCALAPPDATA"] {
            parents.extend(std::env::var_os(var).map(PathBuf::from));
        }
    }
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from) {
        parents.push(home.join(".local").join("share"));
        parents.push(home.join("Games"));
        parents.push(home);
    }
    for parent in &parents {
        let Ok(rd) = std::fs::read_dir(parent) else { continue };
        for e in rd.filter_map(|e| e.ok()) {
            if looks_like_sim(&e.file_name().to_string_lossy()) && e.path().is_dir() {
                roots.push(e.path());
            }
        }
    }
    let mut out: Vec<(PathBuf, PathBuf, usize)> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();
    for root in roots {
        let skins = root.join("NoteSkins").join("pump");
        if !skins.is_dir() {
            continue;
        }
        let key = skins.canonicalize().unwrap_or_else(|_| skins.clone());
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let n = list_skins(&skins).len();
        if n > 0 {
            out.push((root, skins, n));
        }
    }
    out
}

/// Noteskins disponibles en la carpeta (las que tienen al menos un "Tap Note").
pub fn list_skins(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter(|e| is_skin(&e.path()))
                .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|s| s.to_ascii_lowercase());
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grids_from_names() {
        assert_eq!(parse_grid("DownLeft Tap Note (res 192x128) 3x2.PNG"), (3, 2));
        assert_eq!(parse_grid("Center Hold Body Active (res 384x64) 6x1.png"), (6, 1));
        assert_eq!(parse_grid("DownLeft Tap Note 6x1 (res 384x64).png"), (6, 1));
        assert_eq!(parse_grid("UpLeft Hold Body Active (res 64x1).png"), (1, 1));
        assert_eq!(parse_grid("UpLeft Tap Mine 3x2 (doubleres).png"), (3, 2));
        assert_eq!(parse_grid("logo.png"), (1, 1));
    }

    /// Si hay noteskins en `noteskins/pump` (junto a la app, o en la raíz del proyecto fuera
    /// de git), todos se cargan.
    #[test]
    fn portable_skins_load() {
        let Some(dir) = portable_skins_dir() else {
            eprintln!("sin carpeta noteskins/pump; se omite");
            return;
        };
        let names = list_skins(&dir);
        assert!(names.len() >= 50, "sólo {} noteskins en {}", names.len(), dir.display());
        let ctx = egui::Context::default();
        let mut problems = Vec::new();
        for name in &names {
            match load_external(&ctx, &dir.join(name)) {
                Ok(skin) => {
                    for a in [Arrow::DownLeft, Arrow::UpLeft, Arrow::Center, Arrow::UpRight, Arrow::DownRight] {
                        match skin.get(a) {
                            Some((spr, _)) if spr.hold_body.is_some() && spr.hold_cap.is_some() && spr.receptor.is_some() => {}
                            Some(_) => problems.push(format!("{}: {:?} sin hold/receptor", name, a)),
                            None => problems.push(format!("{}: falta {:?}", name, a)),
                        }
                    }
                }
                Err(e) => problems.push(e),
            }
        }
        assert!(problems.is_empty(), "{:#?}", problems);
    }

    #[test]
    fn file_matching() {
        let files: Vec<String> = [
            "Center Tap Note (res 192x128) 3x2.png",
            "CenterLeft Ready Receptor (res 64x192) 1x3.png",
            "Center Ready Receptor (res 64x192) 1x3.png",
            "DownLeft Hold Body Active (res 64x1).png",
            "DownLeft Hold Body Active (res 384x64) 6x1.png",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(match_file(&files, "Center Ready Receptor").as_deref(), Some("Center Ready Receptor (res 64x192) 1x3.png"));
        assert_eq!(match_file(&files, "DownLeft Hold Body Active").as_deref(), Some("DownLeft Hold Body Active (res 384x64) 6x1.png"));
        assert!(match_file(&files, "UpLeft Tap Note").is_none());
    }

    /// Importar desde la raíz de un simulador copia todos los noteskins pump; desde un solo
    /// noteskin, ése y sus respaldos. Reimportar reemplaza la copia anterior.
    #[test]
    fn import_from_simulator() {
        let tmp = std::env::temp_dir().join(format!("step-charter-import-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let pump = tmp.join("Sim").join("NoteSkins").join("pump");
        let write = |p: PathBuf, text: &str| {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        write(pump.join("default").join("DownLeft Tap Note 3x2.png"), "x");
        write(pump.join("Skin A").join("Center Tap Note.png"), "x");
        write(pump.join("Skin A").join("metrics.ini"), "[NoteDisplay]\nFallbackNoteSkin=base\n");
        write(pump.join("base").join("Center Hold Body Active.png"), "x");
        write(pump.join("Skin B").join("UpLeft Tap Note.png"), "x");
        write(tmp.join("Sim").join("NoteSkins").join("dance").join("x").join("Down Tap Note.png"), "x");

        // Desde la raíz del simulador: los noteskins pump (no dance) y el respaldo "base".
        let (base, mut names) = import_plan(&tmp.join("Sim")).unwrap();
        assert_eq!(base, pump);
        names.sort();
        assert_eq!(names, ["Skin A", "Skin B", "base", "default"]);

        // Desde un solo noteskin: ése, su FallbackNoteSkin y default.
        let (_, names) = import_plan(&pump.join("Skin A")).unwrap();
        assert_eq!(names, ["Skin A", "base", "default"]);

        let dest = tmp.join("dest");
        write(dest.join("Skin A").join("viejo.png"), "x");
        let r = import_skins(&base, &names, &dest).unwrap();
        assert_eq!(r.skins, 3);
        assert!(dest.join("Skin A").join("Center Tap Note.png").is_file());
        assert!(!dest.join("Skin A").join("viejo.png").exists());
        assert_eq!(list_skins(&dest), ["default", "Skin A"]);

        // Importar desde la misma carpeta de destino no borra nada.
        assert!(import_skins(&dest, &names, &dest).is_err());
        assert!(dest.join("Skin A").is_dir());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
