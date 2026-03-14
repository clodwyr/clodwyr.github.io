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

pub struct AlienBullet {
    pub x: f64,
    pub y: f64,
}

/// Tracks the alien grid's position and movement direction.
/// `offset_x` is the signed shift from the grid's centred position;
/// `offset_y` accumulates the downward drops on wall reversals.
/// `tick` counts every frame so `step_grid` can gate movement to every N frames.
/// `anim_frame` toggles on every actual move — renderers use it to alternate sprite frames.
pub struct GridMotion {
    pub offset_x: f64,
    pub offset_y: f64,
    pub direction: i8,  // +1 = right, -1 = left
    pub tick: u32,
    pub anim_frame: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GamePhase {
    Attract,
    Playing,
    LevelClear,
    GameOver,
}

pub struct GameState {
    pub width: u32,
    pub height: u32,
    pub aliens: Vec<Alien>,
    pub ship: Ship,
    pub bullet: Option<Bullet>,
    pub alien_bullet: Option<AlienBullet>,
    pub grid: GridMotion,
    pub score: u32,
    pub lives: u32,
    /// Zero-based index into LEVELS — increments each time the level is cleared.
    pub level: usize,
    pub phase: GamePhase,
    /// Counts frames spent in the LevelClear phase before advancing.
    pub pause_timer: u32,
}

impl GameState {
    pub fn new(width: u32, height: u32) -> Self {
        GameState {
            width,
            height,
            aliens: Vec::new(),
            ship: Ship { x: width as f64 / 2.0, y: height as f64 - 40.0 },
            bullet: None,
            alien_bullet: None,
            grid: GridMotion { offset_x: 0.0, offset_y: 0.0, direction: 1, tick: 0, anim_frame: false },
            score: 0,
            lives: 3,
            level: 0,
            phase: GamePhase::Attract,
            pause_timer: 0,
        }
    }
}

// ── Grid geometry ─────────────────────────────────────────────────────────────

pub const CELL_W: f64 = 64.0;
pub const CELL_H: f64 = 48.0;
pub const GRID_COLS: u32 = 11;
pub const GRID_ROWS: u32 = 5;
pub const GRID_W: f64 = GRID_COLS as f64 * CELL_W;
pub const GRID_H: f64 = GRID_ROWS as f64 * CELL_H;
/// Extra space beyond the grid on each side — defines the play area and the
/// maximum distance the grid can shift before reversing. Easy to tune.
pub const PLAY_MARGIN: f64 = 48.0;

// ── Grid movement constants ───────────────────────────────────────────────────

/// Pixels the grid jumps per move — larger = more visible, classic feel. Easy to tune.
pub const GRID_STEP_PX: f64 = 4.0;
/// Frames between grid moves at a full formation (slowest) — easy to tune.
pub const GRID_TICK_MAX: u32 = 30;
/// Frames between grid moves when only one alien remains (fastest) — easy to tune.
pub const GRID_TICK_MIN: u32 = 4;

/// Pluggable speed strategy — swap for different difficulty curves.
pub trait SpeedStrategy {
    /// Pixels to move per step.
    fn step_px(&self, alive_count: usize) -> f64;
    /// Frames between steps; lower = faster.
    fn tick_interval(&self, alive_count: usize) -> u32;
}

/// Classic Space Invaders speed: linearly faster as aliens are killed.
pub struct ClassicSpeed {
    pub total_aliens: usize,
}

// ── Ship constants ────────────────────────────────────────────────────────────

/// How many pixels the ship moves per step — easy to tune.
pub const SHIP_STEP: f64 = 4.0;

/// How many pixels the bullet travels upward per frame — easy to tune.
pub const BULLET_STEP: f64 = 8.0;

/// Half the ship sprite width, used for boundary clamping.
/// Ship sprite is 55px wide drawn at natural size; half = 27.5.
pub const SHIP_HALF_W: f64 = 27.5;

/// Half the ship sprite height, used for alien-bullet collision detection — easy to tune.
pub const SHIP_HALF_H: f64 = 10.0;

/// How many pixels the alien bullet travels downward per frame — easy to tune.
pub const ALIEN_BULLET_STEP: f64 = 4.0;

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

impl SpeedStrategy for ClassicSpeed {
    fn step_px(&self, _alive_count: usize) -> f64 {
        GRID_STEP_PX
    }

    fn tick_interval(&self, alive_count: usize) -> u32 {
        if self.total_aliens <= 1 { return GRID_TICK_MIN; }
        // t = 0 at full grid → GRID_TICK_MAX; t = 1 at 1 alien → GRID_TICK_MIN
        let alive = alive_count.min(self.total_aliens);
        let t = 1.0 - alive as f64 / self.total_aliens as f64;
        let range = (GRID_TICK_MAX - GRID_TICK_MIN) as f64;
        let interval = GRID_TICK_MIN as f64 + (1.0 - t) * range;
        (interval.round() as u32).max(GRID_TICK_MIN)
    }
}

/// Advance the alien grid one step in its current direction.
/// If the step would push `offset_x` beyond `±max_offset_x`, the grid instead
/// reverses direction and drops down by one `CELL_H` without moving horizontally.
/// Does nothing if no aliens are alive.
pub fn step_grid(state: &mut GameState, strategy: &dyn SpeedStrategy, max_offset_x: f64) {
    let alive_count = state.aliens.iter().filter(|a| a.alive).count();
    if alive_count == 0 { return; }
    state.grid.tick = state.grid.tick.wrapping_add(1);
    if state.grid.tick % strategy.tick_interval(alive_count) != 0 { return; }
    state.grid.anim_frame = !state.grid.anim_frame;
    let step = strategy.step_px(alive_count);
    let new_offset = state.grid.offset_x + state.grid.direction as f64 * step;
    if new_offset.abs() >= max_offset_x {
        state.grid.direction *= -1;
        state.grid.offset_y += CELL_H;
    } else {
        state.grid.offset_x = new_offset;
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

/// Check whether the player bullet has hit any alive alien.
/// `grid_left` and `grid_top` are the canvas coordinates of the grid's top-left corner.
/// On a hit: the alien is marked dead, the bullet is cleared, and score is incremented.
/// Only the first hit alien is processed per call (one bullet = one kill).
pub fn check_bullet_hit(state: &mut GameState, grid_left: f64, grid_top: f64) {
    let bx = match state.bullet {
        Some(ref b) => b.x,
        None => return,
    };
    let by = match state.bullet {
        Some(ref b) => b.y,
        None => return,
    };

    for alien in state.aliens.iter_mut().filter(|a| a.alive) {
        let left   = grid_left + alien.col as f64 * CELL_W;
        let right  = left + CELL_W;
        let top    = grid_top  + alien.row as f64 * CELL_H;
        let bottom = top + CELL_H;

        if bx >= left && bx < right && by >= top && by < bottom {
            alien.alive = false;
            state.bullet = None;
            state.score += 1;
            return;
        }
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

/// Fire an alien bullet from the lowest alive alien in `col`.
/// `grid_left` / `grid_top` are the canvas coordinates of the grid's top-left corner.
/// Does nothing if a bullet is already in flight or no alive alien occupies that column.
pub fn fire_alien_bullet(state: &mut GameState, col: u32, grid_left: f64, grid_top: f64) {
    if state.alien_bullet.is_some() { return; }
    // Find the highest row number (= lowest on screen) that is alive in this column
    let lowest = state.aliens.iter()
        .filter(|a| a.alive && a.col == col)
        .max_by_key(|a| a.row);
    if let Some(alien) = lowest {
        let x = grid_left + alien.col as f64 * CELL_W + CELL_W / 2.0;
        let y = grid_top  + alien.row as f64 * CELL_H + CELL_H;
        state.alien_bullet = Some(AlienBullet { x, y });
    }
}

/// Advance the alien bullet downward by ALIEN_BULLET_STEP.
/// Clears it if it has moved past `canvas_h`.
pub fn step_alien_bullet(state: &mut GameState, canvas_h: f64) {
    if let Some(ref mut ab) = state.alien_bullet {
        ab.y += ALIEN_BULLET_STEP;
    }
    if state.alien_bullet.as_ref().map_or(false, |ab| ab.y > canvas_h) {
        state.alien_bullet = None;
    }
}

/// Check whether the alien bullet overlaps the ship.
/// On a hit: `lives` is decremented and the alien bullet is cleared.
pub fn check_alien_hit_ship(state: &mut GameState) {
    let (abx, aby) = match state.alien_bullet {
        Some(ref ab) => (ab.x, ab.y),
        None => return,
    };
    let sx = state.ship.x;
    let sy = state.ship.y;
    if abx >= sx - SHIP_HALF_W && abx <= sx + SHIP_HALF_W
        && aby >= sy - SHIP_HALF_H && aby <= sy + SHIP_HALF_H
    {
        state.lives = state.lives.saturating_sub(1);
        state.alien_bullet = None;
        if state.lives == 0 {
            state.phase = GamePhase::GameOver;
        }
    }
}

/// Check whether the alien grid has descended to the ship's level (invasion).
/// `grid_top` is the canvas y of the grid's top-left corner at offset_y = 0.
/// If any alive alien's bottom edge reaches or passes the ship, sets phase to GameOver.
/// Does nothing if no aliens are alive.
pub fn check_invasion(state: &mut GameState, grid_top: f64) {
    if !state.aliens.iter().any(|a| a.alive) { return; }
    let grid_bottom = grid_top + state.grid.offset_y + GRID_H;
    if grid_bottom >= state.ship.y {
        state.phase = GamePhase::GameOver;
    }
}

/// Frames the "LEVEL CLEAR" screen is shown before loading the next level — easy to tune.
pub const LEVEL_CLEAR_PAUSE: u32 = 120; // ~2 s at 60 fps

/// If all aliens are dead and the game is still Playing, transition to LevelClear
/// and reset the pause timer. Call every frame during the Playing phase.
pub fn check_level_clear(state: &mut GameState) {
    if state.phase != GamePhase::Playing { return; }
    if all_aliens_dead(state) {
        state.phase = GamePhase::LevelClear;
        state.pause_timer = 0;
    }
}

/// Advance the pause timer while in the LevelClear phase.
/// When the timer reaches LEVEL_CLEAR_PAUSE, loads the next level and
/// returns to Playing. Does nothing in any other phase.
pub fn tick_level_clear(state: &mut GameState) {
    if state.phase != GamePhase::LevelClear { return; }
    state.pause_timer += 1;
    if state.pause_timer >= LEVEL_CLEAR_PAUSE {
        advance_level(state);
        state.phase = GamePhase::Playing;
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

    // ── Grid movement tests ───────────────────────────────────────────────────

    fn classic_55() -> ClassicSpeed { ClassicSpeed { total_aliens: 55 } }

    // Helper: set tick so the very next step_grid call triggers a move (with 55 alive).
    fn prime_tick(state: &mut GameState) {
        state.grid.tick = GRID_TICK_MAX - 1;
    }

    #[test]
    fn step_grid_does_not_move_before_interval() {
        // Default tick = 0; first call increments to 1, which is not a multiple of GRID_TICK_MAX.
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        step_grid(&mut state, &classic_55(), 100.0);
        assert_eq!(state.grid.offset_x, 0.0);
    }

    #[test]
    fn step_grid_moves_right_by_step() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        prime_tick(&mut state);
        step_grid(&mut state, &classic_55(), 100.0);
        assert_eq!(state.grid.offset_x, GRID_STEP_PX);
        assert_eq!(state.grid.offset_y, 0.0);
    }

    #[test]
    fn step_grid_reverses_and_drops_at_right_wall() {
        let mut state = GameState::new(800, 600);
        state.grid.offset_x = 99.0;
        state.aliens = build_alien_grid(LEVEL_1);
        prime_tick(&mut state);
        step_grid(&mut state, &classic_55(), 100.0);
        assert_eq!(state.grid.direction, -1);
        assert_eq!(state.grid.offset_y, CELL_H);
        assert_eq!(state.grid.offset_x, 99.0); // x unchanged on reversal frame
    }

    #[test]
    fn step_grid_reverses_and_drops_at_left_wall() {
        let mut state = GameState::new(800, 600);
        state.grid.offset_x = -99.0;
        state.grid.direction = -1;
        state.aliens = build_alien_grid(LEVEL_1);
        prime_tick(&mut state);
        step_grid(&mut state, &classic_55(), 100.0);
        assert_eq!(state.grid.direction, 1);
        assert_eq!(state.grid.offset_y, CELL_H);
        assert_eq!(state.grid.offset_x, -99.0);
    }

    #[test]
    fn step_grid_noop_when_no_alive_aliens() {
        let mut state = GameState::new(800, 600);
        step_grid(&mut state, &classic_55(), 100.0);
        let mut state2 = GameState::new(800, 600);
        state2.aliens = build_alien_grid(LEVEL_1);
        for a in &mut state2.aliens { a.alive = false; }
        let before = state2.grid.offset_x;
        step_grid(&mut state2, &classic_55(), 100.0);
        assert_eq!(state2.grid.offset_x, before);
    }

    #[test]
    fn step_grid_toggles_anim_frame_on_move() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        prime_tick(&mut state);
        assert!(!state.grid.anim_frame);
        step_grid(&mut state, &classic_55(), 100.0);
        assert!(state.grid.anim_frame);
    }

    #[test]
    fn step_grid_does_not_toggle_anim_frame_when_gated() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        // tick=0 → first call won't fire a move
        step_grid(&mut state, &classic_55(), 100.0);
        assert!(!state.grid.anim_frame);
    }

    #[test]
    fn step_grid_anim_frame_alternates_each_move() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        prime_tick(&mut state);
        step_grid(&mut state, &classic_55(), 100.0); // frame → true
        assert!(state.grid.anim_frame);
        // prime for a second move
        state.grid.tick = GRID_TICK_MAX - 1;
        step_grid(&mut state, &classic_55(), 100.0); // frame → false
        assert!(!state.grid.anim_frame);
    }

    #[test]
    fn classic_speed_step_px_is_fixed() {
        let s = ClassicSpeed { total_aliens: 55 };
        assert_eq!(s.step_px(55), GRID_STEP_PX);
        assert_eq!(s.step_px(1),  GRID_STEP_PX);
    }

    #[test]
    fn classic_speed_tick_interval_at_full_grid_is_max() {
        let s = ClassicSpeed { total_aliens: 55 };
        assert_eq!(s.tick_interval(55), GRID_TICK_MAX);
    }

    #[test]
    fn classic_speed_tick_interval_at_one_alien_is_min() {
        let s = ClassicSpeed { total_aliens: 55 };
        assert_eq!(s.tick_interval(1), GRID_TICK_MIN);
    }

    #[test]
    fn classic_speed_tick_interval_decreases_as_aliens_die() {
        let s = ClassicSpeed { total_aliens: 55 };
        assert!(s.tick_interval(30) < s.tick_interval(55));
        assert!(s.tick_interval(1)  < s.tick_interval(30));
    }

    // ── Collision tests ───────────────────────────────────────────────────────

    // Helper: state with a full LEVEL_1 grid and a bullet placed at a known alien cell.
    // grid_left=0, grid_top=0 makes the maths trivial: alien(col,row) occupies
    // [col*CELL_W .. (col+1)*CELL_W] × [row*CELL_H .. (row+1)*CELL_H].
    fn state_with_bullet_at_alien(col: u32, row: u32) -> GameState {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        // Centre of that alien's cell when grid_left=0, grid_top=0
        state.bullet = Some(Bullet {
            x: col as f64 * CELL_W + CELL_W / 2.0,
            y: row as f64 * CELL_H + CELL_H / 2.0,
        });
        state
    }

    #[test]
    fn score_starts_at_zero() {
        assert_eq!(GameState::new(800, 600).score, 0);
    }

    #[test]
    fn lives_start_at_three() {
        assert_eq!(GameState::new(800, 600).lives, 3);
    }

    #[test]
    fn bullet_hit_marks_alien_dead() {
        let mut state = state_with_bullet_at_alien(0, 0);
        check_bullet_hit(&mut state, 0.0, 0.0);
        let hit = state.aliens.iter().find(|a| a.col == 0 && a.row == 0).unwrap();
        assert!(!hit.alive);
    }

    #[test]
    fn bullet_hit_clears_bullet() {
        let mut state = state_with_bullet_at_alien(0, 0);
        check_bullet_hit(&mut state, 0.0, 0.0);
        assert!(state.bullet.is_none());
    }

    #[test]
    fn bullet_hit_increments_score() {
        let mut state = state_with_bullet_at_alien(0, 0);
        check_bullet_hit(&mut state, 0.0, 0.0);
        assert_eq!(state.score, 1);
    }

    #[test]
    fn bullet_miss_leaves_all_aliens_alive() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        // Bullet far to the left of the grid
        state.bullet = Some(Bullet { x: -100.0, y: -100.0 });
        check_bullet_hit(&mut state, 0.0, 0.0);
        assert!(state.aliens.iter().all(|a| a.alive));
        assert_eq!(state.score, 0);
    }

    #[test]
    fn bullet_only_hits_one_alien_per_shot() {
        // Two aliens share the same column — only the front (highest row) should die
        let mut state = state_with_bullet_at_alien(5, 4); // bottom row
        check_bullet_hit(&mut state, 0.0, 0.0);
        let dead: Vec<_> = state.aliens.iter().filter(|a| !a.alive).collect();
        assert_eq!(dead.len(), 1);
    }

    // ── Alien shooting tests ──────────────────────────────────────────────────

    #[test]
    fn fire_alien_bullet_spawns_from_lowest_alien_in_col() {
        // Full grid, grid_left=0 grid_top=0. Bottom row (row 4) in col 0
        // should be the source of the bullet.
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        fire_alien_bullet(&mut state, 0, 0.0, 0.0);
        let ab = state.alien_bullet.as_ref().expect("alien bullet should exist");
        // Expected x: centre of col 0 cell
        assert_eq!(ab.x, CELL_W / 2.0);
        // Expected y: bottom of row 4 cell (spawn at bottom edge)
        assert_eq!(ab.y, 4.0 * CELL_H + CELL_H);
    }

    #[test]
    fn fire_alien_bullet_does_nothing_if_already_in_flight() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        fire_alien_bullet(&mut state, 0, 0.0, 0.0);
        let first_x = state.alien_bullet.as_ref().unwrap().x;
        fire_alien_bullet(&mut state, 5, 0.0, 0.0); // different col
        assert_eq!(state.alien_bullet.as_ref().unwrap().x, first_x);
    }

    #[test]
    fn fire_alien_bullet_skips_dead_aliens_in_col() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        // Kill bottom two rows in col 3
        for a in state.aliens.iter_mut() {
            if a.col == 3 && (a.row == 4 || a.row == 3) {
                a.alive = false;
            }
        }
        fire_alien_bullet(&mut state, 3, 0.0, 0.0);
        let ab = state.alien_bullet.as_ref().expect("should fire from row 2");
        // Row 2 is now the lowest alive in col 3
        assert_eq!(ab.y, 2.0 * CELL_H + CELL_H);
    }

    #[test]
    fn fire_alien_bullet_does_nothing_if_col_empty() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        for a in state.aliens.iter_mut() {
            if a.col == 2 { a.alive = false; }
        }
        fire_alien_bullet(&mut state, 2, 0.0, 0.0);
        assert!(state.alien_bullet.is_none());
    }

    #[test]
    fn step_alien_bullet_moves_downward() {
        let mut state = GameState::new(800, 600);
        state.alien_bullet = Some(AlienBullet { x: 100.0, y: 200.0 });
        step_alien_bullet(&mut state, 600.0);
        assert_eq!(state.alien_bullet.as_ref().unwrap().y, 200.0 + ALIEN_BULLET_STEP);
    }

    #[test]
    fn step_alien_bullet_clears_when_below_canvas() {
        let mut state = GameState::new(800, 600);
        state.alien_bullet = Some(AlienBullet { x: 100.0, y: 598.0 });
        step_alien_bullet(&mut state, 600.0);
        assert!(state.alien_bullet.is_none());
    }

    #[test]
    fn check_alien_hit_ship_decrements_lives_on_overlap() {
        let mut state = GameState::new(800, 600);
        // Place alien bullet exactly on ship centre
        state.alien_bullet = Some(AlienBullet { x: state.ship.x, y: state.ship.y });
        check_alien_hit_ship(&mut state);
        assert_eq!(state.lives, 2);
    }

    #[test]
    fn check_alien_hit_ship_clears_alien_bullet_on_hit() {
        let mut state = GameState::new(800, 600);
        state.alien_bullet = Some(AlienBullet { x: state.ship.x, y: state.ship.y });
        check_alien_hit_ship(&mut state);
        assert!(state.alien_bullet.is_none());
    }

    #[test]
    fn check_alien_hit_ship_does_nothing_when_bullet_misses() {
        let mut state = GameState::new(800, 600);
        // Place bullet far from ship
        state.alien_bullet = Some(AlienBullet { x: 0.0, y: 0.0 });
        check_alien_hit_ship(&mut state);
        assert_eq!(state.lives, 3);
        assert!(state.alien_bullet.is_some());
    }

    #[test]
    fn check_alien_hit_ship_no_bullet_does_nothing() {
        let mut state = GameState::new(800, 600);
        check_alien_hit_ship(&mut state);
        assert_eq!(state.lives, 3);
    }

    #[test]
    fn check_alien_hit_ship_sets_game_over_when_last_life_lost() {
        let mut state = GameState::new(800, 600);
        state.lives = 1;
        state.alien_bullet = Some(AlienBullet { x: state.ship.x, y: state.ship.y });
        check_alien_hit_ship(&mut state);
        assert_eq!(state.lives, 0);
        assert_eq!(state.phase, GamePhase::GameOver);
    }

    #[test]
    fn check_alien_hit_ship_stays_playing_while_lives_remain() {
        let mut state = GameState::new(800, 600);
        state.lives = 2;
        state.alien_bullet = Some(AlienBullet { x: state.ship.x, y: state.ship.y });
        check_alien_hit_ship(&mut state);
        assert_eq!(state.phase, GamePhase::Playing);
    }

    // ── Game over / invasion tests ────────────────────────────────────────────

    #[test]
    fn phase_starts_as_attract() {
        assert_eq!(GameState::new(800, 600).phase, GamePhase::Attract);
    }

    #[test]
    fn check_invasion_sets_game_over_when_grid_reaches_ship() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        // grid_top = 0.0 in this test; drop grid until its bottom touches ship.y
        state.grid.offset_y = state.ship.y - GRID_H;
        check_invasion(&mut state, 0.0);
        assert_eq!(state.phase, GamePhase::GameOver);
    }

    #[test]
    fn check_invasion_does_nothing_while_grid_above_ship() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        state.grid.offset_y = 0.0;
        check_invasion(&mut state, 0.0);
        assert_eq!(state.phase, GamePhase::Playing);
    }

    #[test]
    fn check_invasion_does_nothing_when_no_aliens() {
        let mut state = GameState::new(800, 600);
        // no aliens — grid is empty, should not trigger game over
        check_invasion(&mut state, 0.0);
        assert_eq!(state.phase, GamePhase::Playing);
    }

    // ── Level tests ───────────────────────────────────────────────────────────

    #[test]
    fn level_starts_at_zero() {
        assert_eq!(GameState::new(800, 600).level, 0);
    }

    #[test]
    fn all_aliens_dead_true_when_all_dead() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        for a in &mut state.aliens { a.alive = false; }
        assert!(all_aliens_dead(&state));
    }

    #[test]
    fn all_aliens_dead_false_when_any_alive() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        assert!(!all_aliens_dead(&state));
    }

    #[test]
    fn all_aliens_dead_true_when_grid_empty() {
        let state = GameState::new(800, 600);
        assert!(all_aliens_dead(&state));
    }

    #[test]
    fn advance_level_increments_level_index() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        advance_level(&mut state);
        assert_eq!(state.level, 1);
    }

    #[test]
    fn advance_level_loads_new_alien_grid() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        for a in &mut state.aliens { a.alive = false; }
        advance_level(&mut state);
        assert!(state.aliens.iter().any(|a| a.alive));
    }

    #[test]
    fn advance_level_resets_grid_motion() {
        let mut state = GameState::new(800, 600);
        state.grid.offset_x = 40.0;
        state.grid.offset_y = 96.0;
        state.grid.direction = -1;
        advance_level(&mut state);
        assert_eq!(state.grid.offset_x, 0.0);
        assert_eq!(state.grid.offset_y, 0.0);
        assert_eq!(state.grid.direction, 1);
    }

    #[test]
    fn advance_level_clears_bullets() {
        let mut state = GameState::new(800, 600);
        state.bullet = Some(Bullet { x: 100.0, y: 100.0 });
        state.alien_bullet = Some(AlienBullet { x: 200.0, y: 200.0 });
        advance_level(&mut state);
        assert!(state.bullet.is_none());
        assert!(state.alien_bullet.is_none());
    }

    #[test]
    fn advance_level_wraps_to_level_zero_after_last() {
        let mut state = GameState::new(800, 600);
        state.level = LEVELS.len() - 1;
        advance_level(&mut state);
        assert_eq!(state.level, 0);
    }

    #[test]
    fn levels_has_at_least_two_entries() {
        assert!(LEVELS.len() >= 2);
    }

    #[test]
    fn level_2_pattern_has_five_rows() {
        assert_eq!(LEVEL_2.len(), 5);
    }

    // ── Level-clear pause tests ───────────────────────────────────────────────

    #[test]
    fn check_level_clear_transitions_when_all_aliens_dead() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        for a in &mut state.aliens { a.alive = false; }
        check_level_clear(&mut state);
        assert_eq!(state.phase, GamePhase::LevelClear);
    }

    #[test]
    fn check_level_clear_resets_pause_timer() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        for a in &mut state.aliens { a.alive = false; }
        state.pause_timer = 99;
        check_level_clear(&mut state);
        assert_eq!(state.pause_timer, 0);
    }

    #[test]
    fn check_level_clear_does_nothing_while_aliens_remain() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        check_level_clear(&mut state);
        assert_eq!(state.phase, GamePhase::Playing);
    }

    #[test]
    fn tick_level_clear_increments_pause_timer() {
        let mut state = GameState::new(800, 600);
        state.phase = GamePhase::LevelClear;
        tick_level_clear(&mut state);
        assert_eq!(state.pause_timer, 1);
    }

    #[test]
    fn tick_level_clear_does_nothing_when_not_level_clear() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        tick_level_clear(&mut state);
        assert_eq!(state.pause_timer, 0);
        assert_eq!(state.phase, GamePhase::Playing);
    }

    #[test]
    fn tick_level_clear_advances_level_after_pause() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        state.phase = GamePhase::LevelClear;
        state.pause_timer = LEVEL_CLEAR_PAUSE - 1;
        tick_level_clear(&mut state);
        assert_eq!(state.phase, GamePhase::Playing);
        // Level should have advanced and a new grid loaded
        assert!(state.aliens.iter().any(|a| a.alive));
    }

    #[test]
    fn tick_level_clear_does_not_advance_before_pause_expires() {
        let mut state = GameState::new(800, 600);
        state.aliens = build_alien_grid(LEVEL_1);
        for a in &mut state.aliens { a.alive = false; }
        state.phase = GamePhase::LevelClear;
        state.pause_timer = LEVEL_CLEAR_PAUSE - 2;
        tick_level_clear(&mut state);
        assert_eq!(state.phase, GamePhase::LevelClear);
        // Grid should still be empty — advance_level not yet called
        assert!(!state.aliens.iter().any(|a| a.alive));
    }

    // ── reset_game tests ──────────────────────────────────────────────────────

    #[test]
    fn reset_game_sets_phase_to_playing() {
        let mut state = GameState::new(800, 600);
        state.phase = GamePhase::GameOver;
        reset_game(&mut state);
        assert_eq!(state.phase, GamePhase::Playing);
    }

    #[test]
    fn reset_game_restores_lives() {
        let mut state = GameState::new(800, 600);
        state.lives = 0;
        reset_game(&mut state);
        assert_eq!(state.lives, 3);
    }

    #[test]
    fn reset_game_clears_score() {
        let mut state = GameState::new(800, 600);
        state.score = 500;
        reset_game(&mut state);
        assert_eq!(state.score, 0);
    }

    #[test]
    fn reset_game_resets_to_level_zero() {
        let mut state = GameState::new(800, 600);
        state.level = 2;
        reset_game(&mut state);
        assert_eq!(state.level, 0);
    }

    #[test]
    fn reset_game_loads_fresh_alien_grid() {
        let mut state = GameState::new(800, 600);
        reset_game(&mut state);
        assert_eq!(state.aliens.iter().filter(|a| a.alive).count(), 55);
    }

    #[test]
    fn reset_game_clears_bullets() {
        let mut state = GameState::new(800, 600);
        state.bullet = Some(Bullet { x: 100.0, y: 100.0 });
        state.alien_bullet = Some(AlienBullet { x: 200.0, y: 200.0 });
        reset_game(&mut state);
        assert!(state.bullet.is_none());
        assert!(state.alien_bullet.is_none());
    }

    #[test]
    fn reset_game_resets_ship_to_centre() {
        let mut state = GameState::new(800, 600);
        state.ship.x = 100.0;
        reset_game(&mut state);
        assert_eq!(state.ship.x, 400.0);
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

/// Level 2: squids fill the top two rows, crabs the middle, octopuses at the bottom.
/// Denser squid presence makes it harder (squids score higher in classic SI).
pub const LEVEL_2: LevelPattern = &[
    "SSSSSSSSSSS",
    "SSSSSSSSSSS",
    "CCCCCCCCCCC",
    "OOOOOOOOOOO",
    "OOOOOOOOOOO",
];

/// Level 3: all squids — maximum difficulty.
pub const LEVEL_3: LevelPattern = &[
    "SSSSSSSSSSS",
    "SSSSSSSSSSS",
    "SSSSSSSSSSS",
    "CCCCCCCCCCC",
    "OOOOOOOOOOO",
];

/// All levels in order. `advance_level` cycles through these and wraps back to 0.
pub const LEVELS: &[LevelPattern] = &[LEVEL_1, LEVEL_2, LEVEL_3];

/// Returns `true` if every alien in the grid is dead (or the grid is empty).
pub fn all_aliens_dead(state: &GameState) -> bool {
    state.aliens.iter().all(|a| !a.alive)
}

/// Advance to the next level: increment `state.level` (wrapping), load the
/// corresponding alien grid, reset grid motion, and clear any in-flight bullets.
pub fn advance_level(state: &mut GameState) {
    state.level = (state.level + 1) % LEVELS.len();
    state.aliens = build_alien_grid(LEVELS[state.level]);
    state.grid = GridMotion { offset_x: 0.0, offset_y: 0.0, direction: 1, tick: 0, anim_frame: false };
    state.bullet = None;
    state.alien_bullet = None;
}

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
