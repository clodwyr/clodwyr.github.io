pub struct Alien {
    pub col: u32,  // column index in grid (0-based)
    pub row: u32,  // row index in grid (0-based)
    pub alive: bool,
    pub sprite: AlienKind,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AlienKind {
    Crab,
    Squid,
    Octopus,
}

pub struct Ship {
    pub x: f64, // canvas x of ship centre
    pub y: f64, // canvas y of ship centre
}

pub struct GameState {
    pub width: u32,
    pub height: u32,
    pub aliens: Vec<Alien>,
    pub ship: Ship,
}

impl GameState {
    pub fn new(width: u32, height: u32) -> Self {
        GameState {
            width,
            height,
            aliens: Vec::new(),
            ship: Ship { x: width as f64 / 2.0, y: height as f64 - 40.0 },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Standard Space Invaders grid: 11 columns × 5 rows = 55 aliens
    #[test]
    fn build_alien_grid_produces_55_aliens() {
        let aliens = build_alien_grid(LEVEL_1);
        assert_eq!(aliens.len(), 55);
    }

    #[test]
    fn build_alien_grid_all_start_alive() {
        let aliens = build_alien_grid(LEVEL_1);
        assert!(aliens.iter().all(|a| a.alive));
    }

    #[test]
    fn build_alien_grid_row0_is_squid() {
        let aliens = build_alien_grid(LEVEL_1);
        let row0: Vec<_> = aliens.iter().filter(|a| a.row == 0).collect();
        assert!(row0.iter().all(|a| a.sprite == AlienKind::Squid));
    }

    #[test]
    fn build_alien_grid_row1_row2_are_crab() {
        let aliens = build_alien_grid(LEVEL_1);
        let mid: Vec<_> = aliens.iter().filter(|a| a.row == 1 || a.row == 2).collect();
        assert!(mid.iter().all(|a| a.sprite == AlienKind::Crab));
    }

    #[test]
    fn build_alien_grid_row3_row4_are_octopus() {
        let aliens = build_alien_grid(LEVEL_1);
        let bottom: Vec<_> = aliens.iter().filter(|a| a.row == 3 || a.row == 4).collect();
        assert!(bottom.iter().all(|a| a.sprite == AlienKind::Octopus));
    }

    #[test]
    fn ship_starts_horizontally_centred() {
        let state = GameState::new(800, 600);
        assert_eq!(state.ship.x, 400.0);
    }

    #[test]
    fn ship_starts_near_bottom() {
        let state = GameState::new(800, 600);
        assert_eq!(state.ship.y, 560.0);
    }
}

/// Level grid pattern: 5 rows × 11 columns.
/// Each char maps to an AlienKind: 'S' = Squid, 'C' = Crab, 'O' = Octopus.
/// Rows are top-to-bottom; all rows must be 11 chars wide.
pub type LevelPattern = &'static [&'static str];

pub const LEVEL_1: LevelPattern = &[
    "SSSSSSSSSSS",
    "CCCCCCCCCCC",
    "CCCCCCCCCCC",
    "OOOOOOOOOOO",
    "OOOOOOOOOOO",
];

pub fn build_alien_grid(pattern: LevelPattern) -> Vec<Alien> {
    let mut aliens = Vec::new();
    for (row, line) in pattern.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            let sprite = match ch {
                'S' => AlienKind::Squid,
                'C' => AlienKind::Crab,
                'O' => AlienKind::Octopus,
                _ => continue,
            };
            aliens.push(Alien {
                col: col as u32,
                row: row as u32,
                alive: true,
                sprite,
            });
        }
    }
    aliens
}

pub fn quarter_size(width: f64, height: f64) -> (f64, f64) {
    (width / 4.0, height / 4.0)
}

pub fn centered_position(canvas_w: f64, canvas_h: f64, img_w: f64, img_h: f64) -> (f64, f64) {
    ((canvas_w - img_w) / 2.0, (canvas_h - img_h) / 2.0)
}
