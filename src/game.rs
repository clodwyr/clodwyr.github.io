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

pub struct Bullet {
    pub x: f64,
    pub y: f64,
}

pub struct GameState {
    pub width: u32,
    pub height: u32,
    pub aliens: Vec<Alien>,
    pub ship: Ship,
    pub bullet: Option<Bullet>,
}

impl GameState {
    pub fn new(width: u32, height: u32) -> Self {
        GameState {
            width,
            height,
            aliens: Vec::new(),
            ship: Ship { x: width as f64 / 2.0, y: height as f64 - 40.0 },
            bullet: None,
        }
    }
}

/// How many pixels the ship moves per step — easy to tune.
pub const SHIP_STEP: f64 = 4.0;

/// How many pixels the bullet travels upward per frame — easy to tune.
pub const BULLET_STEP: f64 = 8.0;

/// Half the ship sprite width, used for boundary clamping.
/// Ship sprite is 55px wide drawn at natural size; half = 27.5.
pub const SHIP_HALF_W: f64 = 27.5;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Direction {
    Left,
    Right,
}

/// Pluggable movement strategy — swap this out for momentum, acceleration, etc.
pub trait MovementStrategy {
    fn step(&self) -> f64;
}

/// Crisp arcade movement: fixed pixel step per frame, no momentum.
pub struct CrispMovement {
    pub step_px: f64,
}

impl MovementStrategy for CrispMovement {
    fn step(&self) -> f64 {
        self.step_px
    }
}

/// Move the ship one step in `direction`, clamping to [left_bound, right_bound].
/// Both bounds are canvas x positions for the ship's centre point.
pub fn move_ship(ship: &mut Ship, direction: Direction, strategy: &dyn MovementStrategy, left_bound: f64, right_bound: f64) {
    match direction {
        Direction::Left  => ship.x = (ship.x - strategy.step()).max(left_bound),
        Direction::Right => ship.x = (ship.x + strategy.step()).min(right_bound),
    }
}

/// Fire a bullet from the ship's current position.
/// Does nothing if a bullet is already in flight.
pub fn fire(state: &mut GameState) {
    if state.bullet.is_none() {
        state.bullet = Some(Bullet { x: state.ship.x, y: state.ship.y });
    }
}

/// Advance the bullet upward by BULLET_STEP.
/// Clears the bullet if it has moved above `boundary_top`.
pub fn step_bullet(state: &mut GameState, boundary_top: f64) {
    if let Some(ref mut b) = state.bullet {
        b.y -= BULLET_STEP;
    }
    if state.bullet.as_ref().map_or(false, |b| b.y < boundary_top) {
        state.bullet = None;
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

    // ── Movement tests ────────────────────────────────────────────────────────

    fn crisp() -> CrispMovement { CrispMovement { step_px: SHIP_STEP } }

    // Helpers: arbitrary play-area bounds for basic movement tests
    const LEFT:  f64 = SHIP_HALF_W;
    const RIGHT: f64 = 800.0 - SHIP_HALF_W;

    #[test]
    fn ship_moves_right_by_step() {
        let mut ship = Ship { x: 400.0, y: 560.0 };
        move_ship(&mut ship, Direction::Right, &crisp(), LEFT, RIGHT);
        assert_eq!(ship.x, 400.0 + SHIP_STEP);
    }

    #[test]
    fn ship_moves_left_by_step() {
        let mut ship = Ship { x: 400.0, y: 560.0 };
        move_ship(&mut ship, Direction::Left, &crisp(), LEFT, RIGHT);
        assert_eq!(ship.x, 400.0 - SHIP_STEP);
    }

    #[test]
    fn ship_clamps_at_left_boundary() {
        let mut ship = Ship { x: LEFT + 1.0, y: 560.0 };
        move_ship(&mut ship, Direction::Left, &crisp(), LEFT, RIGHT);
        assert_eq!(ship.x, LEFT);
    }

    #[test]
    fn ship_clamps_at_right_boundary() {
        let mut ship = Ship { x: RIGHT - 1.0, y: 560.0 };
        move_ship(&mut ship, Direction::Right, &crisp(), LEFT, RIGHT);
        assert_eq!(ship.x, RIGHT);
    }

    #[test]
    fn ship_clamps_to_grid_left_bound() {
        // Grid 704px wide centred on 1280px canvas → grid_left = 288
        let grid_left: f64 = 288.0;
        let grid_right = grid_left + 704.0;
        let l_bound = grid_left + SHIP_HALF_W;
        let r_bound = grid_right - SHIP_HALF_W;
        let mut ship = Ship { x: l_bound + 1.0, y: 560.0 };
        move_ship(&mut ship, Direction::Left, &crisp(), l_bound, r_bound);
        assert_eq!(ship.x, l_bound);
    }

    #[test]
    fn ship_clamps_to_grid_right_bound() {
        let grid_left: f64 = 288.0;
        let grid_right = grid_left + 704.0;
        let l_bound = grid_left + SHIP_HALF_W;
        let r_bound = grid_right - SHIP_HALF_W;
        let mut ship = Ship { x: r_bound - 1.0, y: 560.0 };
        move_ship(&mut ship, Direction::Right, &crisp(), l_bound, r_bound);
        assert_eq!(ship.x, r_bound);
    }

    // ── Shooting tests ────────────────────────────────────────────────────────

    #[test]
    fn fire_spawns_bullet_at_ship_position() {
        let mut state = GameState::new(800, 600);
        fire(&mut state);
        let b = state.bullet.as_ref().expect("bullet should exist after firing");
        assert_eq!(b.x, state.ship.x);
        assert_eq!(b.y, state.ship.y);
    }

    #[test]
    fn fire_does_nothing_when_bullet_already_in_flight() {
        let mut state = GameState::new(800, 600);
        fire(&mut state);
        let first_y = state.bullet.as_ref().unwrap().y;
        // Move ship so we can detect if a new bullet was spawned
        state.ship.x = 100.0;
        fire(&mut state);
        // Bullet x should still be the original (not the new ship position)
        assert_eq!(state.bullet.as_ref().unwrap().x, 400.0);
        let _ = first_y;
    }

    #[test]
    fn step_bullet_moves_upward() {
        let mut state = GameState::new(800, 600);
        fire(&mut state);
        let start_y = state.bullet.as_ref().unwrap().y;
        step_bullet(&mut state, 0.0); // boundary_top = 0 (won't clear)
        assert_eq!(state.bullet.as_ref().unwrap().y, start_y - BULLET_STEP);
    }

    #[test]
    fn step_bullet_clears_when_above_boundary_top() {
        let mut state = GameState::new(800, 600);
        fire(&mut state);
        // Place bullet just above the boundary top
        state.bullet.as_mut().unwrap().y = 100.0;
        step_bullet(&mut state, 110.0); // boundary_top = 110, bullet at 100 → already past
        assert!(state.bullet.is_none());
    }

    #[test]
    fn can_fire_again_after_bullet_clears() {
        let mut state = GameState::new(800, 600);
        fire(&mut state);
        state.bullet.as_mut().unwrap().y = 50.0;
        step_bullet(&mut state, 100.0); // clears bullet
        assert!(state.bullet.is_none());
        fire(&mut state); // should spawn a new bullet
        assert!(state.bullet.is_some());
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
