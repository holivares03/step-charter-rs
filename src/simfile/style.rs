//! Estilos de juego (steps types): número de columnas, jugadores y flechas de cada columna.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrow {
    // Pump
    DownLeft,
    UpLeft,
    Center,
    UpRight,
    DownRight,
    // Dance
    Left,
    Down,
    Up,
    Right,
    // Genérica
    Generic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Game {
    Pump,
    Dance,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerMode {
    /// Un jugador.
    Single,
    /// Dos jugadores, cada uno en su mitad (couple).
    Couple,
    /// Varios jugadores en las mismas columnas (routine / co-op de StepF2).
    Shared,
}

#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub id: &'static str,
    pub name: &'static str,
    pub short: &'static str,
    pub game: Game,
    pub arrows: &'static [Arrow],
    pub mode: PlayerMode,
    /// Número de jugadores por defecto.
    pub default_players: u8,
    /// Columnas por "pad" (5 en pump, 4 en dance) — para dibujar el separador.
    pub pad_cols: usize,
}

impl Style {
    pub fn num_cols(&self) -> usize {
        self.arrows.len()
    }

    /// Permutación de columnas para el espejo horizontal (izq↔der) o vertical (arriba↔abajo),
    /// igual que `mirrorH` / `mirrorV` de `settings/styles.txt` de ArrowVortex.
    pub fn mirror_perm(&self, horizontal: bool) -> Vec<usize> {
        let n = self.num_cols();
        let fixed: Option<&[usize]> = match (self.id, horizontal) {
            ("pump-single" | "pump-single-p", true) => Some(&[4, 3, 2, 1, 0]),
            ("pump-single" | "pump-single-p", false) => Some(&[1, 0, 2, 4, 3]),
            ("pump-halfdouble", true) => Some(&[5, 4, 3, 2, 1, 0]),
            ("pump-halfdouble", false) => Some(&[0, 2, 1, 4, 3, 5]),
            ("pump-double" | "pump-couple" | "pump-routine" | "pump-double-p", true) => Some(&[9, 8, 7, 6, 5, 4, 3, 2, 1, 0]),
            ("pump-double" | "pump-couple" | "pump-routine" | "pump-double-p", false) => Some(&[1, 0, 2, 4, 3, 6, 5, 7, 9, 8]),
            ("dance-single", true) => Some(&[3, 1, 2, 0]),
            ("dance-single", false) => Some(&[0, 2, 1, 3]),
            ("dance-double" | "dance-couple" | "dance-routine", true) => Some(&[7, 5, 6, 4, 3, 1, 2, 0]),
            ("dance-double" | "dance-couple" | "dance-routine", false) => Some(&[0, 2, 1, 3, 4, 6, 5, 7]),
            ("dance-solo", true) => Some(&[5, 4, 2, 3, 1, 0]),
            ("dance-solo", false) => Some(&[0, 1, 3, 2, 4, 5]),
            ("dance-threepanel", true) => Some(&[2, 1, 0]),
            _ => None,
        };
        match fixed {
            Some(p) if p.len() == n => p.to_vec(),
            _ if horizontal => (0..n).rev().collect(),
            _ => (0..n).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_perms_are_involutions() {
        for s in STYLES {
            for h in [true, false] {
                let p = s.mirror_perm(h);
                assert_eq!(p.len(), s.num_cols(), "{}", s.id);
                for (i, &c) in p.iter().enumerate() {
                    assert_eq!(p[c], i, "{} h={}", s.id, h);
                }
            }
        }
    }

    #[test]
    fn pump_mirror_matches_arrows() {
        // El espejo horizontal de un DownLeft es un DownRight, etc.
        let s = find("pump-double").unwrap();
        let h = s.mirror_perm(true);
        let v = s.mirror_perm(false);
        let flip_h = |a: Arrow| match a {
            Arrow::DownLeft => Arrow::DownRight,
            Arrow::DownRight => Arrow::DownLeft,
            Arrow::UpLeft => Arrow::UpRight,
            Arrow::UpRight => Arrow::UpLeft,
            x => x,
        };
        let flip_v = |a: Arrow| match a {
            Arrow::DownLeft => Arrow::UpLeft,
            Arrow::UpLeft => Arrow::DownLeft,
            Arrow::DownRight => Arrow::UpRight,
            Arrow::UpRight => Arrow::DownRight,
            x => x,
        };
        for c in 0..10 {
            assert_eq!(s.arrows[h[c]], flip_h(s.arrows[c]));
            assert_eq!(s.arrows[v[c]], flip_v(s.arrows[c]));
        }
        let hd = find("pump-halfdouble").unwrap();
        let v = hd.mirror_perm(false);
        for c in 0..6 {
            assert_eq!(hd.arrows[v[c]], flip_v(hd.arrows[c]));
        }
    }
}

use Arrow::*;

const PUMP_SINGLE: &[Arrow] = &[DownLeft, UpLeft, Center, UpRight, DownRight];
const PUMP_HALF: &[Arrow] = &[Center, UpRight, DownRight, DownLeft, UpLeft, Center];
const PUMP_DOUBLE: &[Arrow] =
    &[DownLeft, UpLeft, Center, UpRight, DownRight, DownLeft, UpLeft, Center, UpRight, DownRight];
const DANCE_SINGLE: &[Arrow] = &[Left, Down, Up, Right];
const DANCE_DOUBLE: &[Arrow] = &[Left, Down, Up, Right, Left, Down, Up, Right];
const DANCE_SOLO: &[Arrow] = &[Left, UpLeft, Down, Up, UpRight, Right];
const DANCE_THREE: &[Arrow] = &[Left, Down, Right];

pub static STYLES: &[Style] = &[
    Style { id: "pump-single", name: "Pump Single", short: "S", game: Game::Pump, arrows: PUMP_SINGLE, mode: PlayerMode::Single, default_players: 1, pad_cols: 5 },
    Style { id: "pump-halfdouble", name: "Pump Half-Double", short: "HD", game: Game::Pump, arrows: PUMP_HALF, mode: PlayerMode::Single, default_players: 1, pad_cols: 3 },
    Style { id: "pump-double", name: "Pump Double", short: "D", game: Game::Pump, arrows: PUMP_DOUBLE, mode: PlayerMode::Single, default_players: 1, pad_cols: 5 },
    Style { id: "pump-couple", name: "Pump Couple", short: "CP", game: Game::Pump, arrows: PUMP_DOUBLE, mode: PlayerMode::Couple, default_players: 2, pad_cols: 5 },
    Style { id: "pump-routine", name: "Pump Routine", short: "RT", game: Game::Pump, arrows: PUMP_DOUBLE, mode: PlayerMode::Shared, default_players: 2, pad_cols: 5 },
    Style { id: "pump-single-p", name: "Pump Single Co-op", short: "SP", game: Game::Pump, arrows: PUMP_SINGLE, mode: PlayerMode::Shared, default_players: 1, pad_cols: 5 },
    Style { id: "pump-double-p", name: "Pump Double Co-op", short: "DP", game: Game::Pump, arrows: PUMP_DOUBLE, mode: PlayerMode::Shared, default_players: 2, pad_cols: 5 },
    Style { id: "dance-single", name: "Dance Single", short: "S", game: Game::Dance, arrows: DANCE_SINGLE, mode: PlayerMode::Single, default_players: 1, pad_cols: 4 },
    Style { id: "dance-double", name: "Dance Double", short: "D", game: Game::Dance, arrows: DANCE_DOUBLE, mode: PlayerMode::Single, default_players: 1, pad_cols: 4 },
    Style { id: "dance-couple", name: "Dance Couple", short: "CP", game: Game::Dance, arrows: DANCE_DOUBLE, mode: PlayerMode::Couple, default_players: 2, pad_cols: 4 },
    Style { id: "dance-routine", name: "Dance Routine", short: "RT", game: Game::Dance, arrows: DANCE_DOUBLE, mode: PlayerMode::Shared, default_players: 2, pad_cols: 4 },
    Style { id: "dance-solo", name: "Dance Solo", short: "SL", game: Game::Dance, arrows: DANCE_SOLO, mode: PlayerMode::Single, default_players: 1, pad_cols: 6 },
    Style { id: "dance-threepanel", name: "Dance Three Panel", short: "3P", game: Game::Dance, arrows: DANCE_THREE, mode: PlayerMode::Single, default_players: 1, pad_cols: 3 },
];

/// Estilos genéricos para steps types desconocidos (por número de columnas).
const GENERIC: [Arrow; 16] = [Generic; 16];

pub fn find(id: &str) -> Option<&'static Style> {
    let id = id.trim().to_ascii_lowercase();
    STYLES.iter().find(|s| s.id == id)
}

/// Estilo para un steps type; si es desconocido usa uno genérico con `cols` columnas.
pub fn for_steps_type(id: &str, cols: usize) -> Style {
    if let Some(s) = find(id) {
        return *s;
    }
    let n = cols.clamp(1, GENERIC.len());
    // Leak controlado: sólo ocurre una vez por combinación desconocida y es diminuto.
    let name: &'static str = Box::leak(id.to_string().into_boxed_str());
    Style {
        id: name,
        name,
        short: "?",
        game: Game::Other,
        arrows: &GENERIC[..n],
        mode: PlayerMode::Single,
        default_players: 1,
        pad_cols: n,
    }
}

/// Estilos que el editor ofrece para charts nuevos (enfocado en Pump).
pub fn pump_styles() -> impl Iterator<Item = &'static Style> {
    STYLES.iter().filter(|s| s.game == Game::Pump)
}
