pub mod game;
pub mod sound;

use sound::SoundEngine;

use game::{
    check_alien_hit_ship, check_bullet_hit, check_invasion, check_level_clear, check_ufo_hit,
    fire, fire_alien_bullet, move_ship, pause_game, quit_game, reset_game, step_alien_bullets,
    step_bullet, step_grid, tick_explosions, tick_game_over, tick_ground_explosions,
    tick_level_clear, tick_ufo, try_spawn_ufo, AlienKind, ClassicSpeed, CrispMovement,
    Direction, GamePhase, GameState, SpeedStrategy,
    CELL_H, CELL_W, GAME_OVER_PAUSE, GRID_COLS, GRID_W, PLAY_MARGIN, SHIP_HALF_H, SHIP_STEP,
    UFO_H, UFO_SCORES, UFO_W,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use js_sys::Math;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlImageElement, KeyboardEvent};

// ── Entry point ───────────────────────────────────────────────────────────────

#[wasm_bindgen(start)]
pub fn start() {
    let window = web_sys::window().expect("no global window");
    let document = window.document().expect("no document on window");

    let canvas = document
        .get_element_by_id("canvas")
        .expect("no #canvas element")
        .dyn_into::<HtmlCanvasElement>()
        .expect("#canvas is not a canvas element");

    let viewport_w = window.inner_width().unwrap().as_f64().unwrap();
    let viewport_h = window.inner_height().unwrap().as_f64().unwrap();
    canvas.set_width(viewport_w as u32);
    canvas.set_height(viewport_h as u32);

    let context = Rc::new(
        canvas
            .get_context("2d").unwrap().unwrap()
            .dyn_into::<CanvasRenderingContext2d>()
            .expect("failed to get 2d context"),
    );

    // Shared game state — populate the first level's alien grid immediately
    let state = Rc::new(RefCell::new(
        GameState::new(viewport_w as u32, viewport_h as u32)
    ));
    // Key state: which keys are currently held
    let keys: Rc<RefCell<HashMap<String, bool>>> = Rc::new(RefCell::new(HashMap::new()));

    // ── Keyboard listeners ────────────────────────────────────────────────────

    {
        let keys_down = keys.clone();
        let on_keydown = Closure::<dyn FnMut(_)>::new(move |e: KeyboardEvent| {
            keys_down.borrow_mut().insert(e.key(), true);
        });
        document
            .add_event_listener_with_callback("keydown", on_keydown.as_ref().unchecked_ref())
            .unwrap();
        on_keydown.forget();
    }

    {
        let keys_up = keys.clone();
        let on_keyup = Closure::<dyn FnMut(_)>::new(move |e: KeyboardEvent| {
            keys_up.borrow_mut().remove(&e.key());
        });
        document
            .add_event_listener_with_callback("keyup", on_keyup.as_ref().unchecked_ref())
            .unwrap();
        on_keyup.forget();
    }

    // ── Load sprites then start loop ──────────────────────────────────────────

    let sprites: Rc<RefCell<HashMap<&'static str, HtmlImageElement>>> =
        Rc::new(RefCell::new(HashMap::new()));
    let loaded = Rc::new(RefCell::new(0u32));
    const TOTAL: u32 = 11;

    for name in ["crab", "crab_f2", "squid", "squid_f2", "octopus", "octopus_f2", "ship",
                 "crab_exp", "squid_exp", "octopus_exp", "ufo"] {
        let img = HtmlImageElement::new().expect("failed to create image");
        img.set_src(&format!("assets/{name}.png"));

        let context_c = context.clone();
        let state_c   = state.clone();
        let sprites_c = sprites.clone();
        let keys_c    = keys.clone();
        let loaded_c  = loaded.clone();

        let onload = Closure::wrap(Box::new(move || {
            *loaded_c.borrow_mut() += 1;
            if *loaded_c.borrow() == TOTAL {
                start_loop(
                    context_c.clone(),
                    state_c.clone(),
                    sprites_c.clone(),
                    keys_c.clone(),
                    viewport_w,
                    viewport_h,
                );
            }
        }) as Box<dyn FnMut()>);

        img.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget();

        sprites.borrow_mut().insert(name, img);
    }
}

// ── Game loop ─────────────────────────────────────────────────────────────────

fn start_loop(
    context: Rc<CanvasRenderingContext2d>,
    state: Rc<RefCell<GameState>>,
    sprites: Rc<RefCell<HashMap<&'static str, HtmlImageElement>>>,
    keys: Rc<RefCell<HashMap<String, bool>>>,
    viewport_w: f64,
    viewport_h: f64,
) {
    // Wrap the rAF callback in Rc<RefCell<Option<Closure>>> so it can schedule itself.
    let raf_cb: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let raf_cb_init = raf_cb.clone();

    let movement     = CrispMovement { step_px: SHIP_STEP };

    // Play area: grid centred with PLAY_MARGIN of breathing room on each side.
    // The grid shifts ±PLAY_MARGIN from centre; ship is bounded to the same area.
    let max_offset_x   = PLAY_MARGIN;
    let base_grid_left = (viewport_w - GRID_W) / 2.0;
    let play_left      = base_grid_left - PLAY_MARGIN;
    let grid_top       = viewport_h * 0.15;
    let ship_left      = play_left + game::SHIP_HALF_W;
    let ship_right     = play_left + GRID_W + 2.0 * PLAY_MARGIN - game::SHIP_HALF_W;

    // Frame counter — used as a cheap pseudo-random column selector.
    let frame: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));

    // Track prior-frame key state for rising-edge (fresh-press) detection.
    let mut space_was_held = false;
    let mut p_was_held = false;
    let mut q_was_held = false;
    let mut s_was_held = false;

    // Sound engine — created here, resumed on first Space press (autoplay policy).
    let mut sound: Option<SoundEngine> = SoundEngine::new().ok();
    let mut sound_initialized = false;
    // UFO sound state — tracks whether the continuous tone is running.
    let mut ufo_sound_active = false;

    *raf_cb_init.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        // ── Update ────────────────────────────────────────────────────────────
        {
            let mut s = state.borrow_mut();

            // Detect fresh key presses (rising edge — not held keys).
            let space_now = keys.borrow().contains_key(" ");
            let p_now     = keys.borrow().contains_key("p") || keys.borrow().contains_key("P");
            let q_now     = keys.borrow().contains_key("q") || keys.borrow().contains_key("Q");
            let s_now     = keys.borrow().contains_key("s") || keys.borrow().contains_key("S");
            let space_just_pressed = space_now && !space_was_held;
            let p_just_pressed     = p_now && !p_was_held;
            let q_just_pressed     = q_now && !q_was_held;
            let s_just_pressed     = s_now && !s_was_held;
            space_was_held = space_now;
            p_was_held = p_now;
            q_was_held = q_now;
            s_was_held = s_now;

            if space_just_pressed {
                // Resume AudioContext on first user gesture (browser autoplay policy).
                if !sound_initialized {
                    if let Some(ref snd) = sound { snd.resume(); }
                    sound_initialized = true;
                }
                match s.phase {
                    GamePhase::Attract => reset_game(&mut s),
                    // Only allow restart once the game-over message has been visible long enough
                    GamePhase::GameOver if s.game_over_timer >= GAME_OVER_PAUSE => {
                        reset_game(&mut s);
                    }
                    _ => {}
                }
            }
            if p_just_pressed {
                pause_game(&mut s);
            }
            if q_just_pressed {
                quit_game(&mut s);
            }
            if s_just_pressed {
                if let Some(ref mut snd) = sound {
                    let now_muted = SoundEngine::toggle(&mut snd.muted);
                    // Stop UFO tone immediately on mute; restart if UFO is still flying.
                    if now_muted {
                        snd.stop_ufo_sound();
                        ufo_sound_active = false;
                    } else if ufo_sound_active {
                        snd.start_ufo_sound();
                    }
                }
            }

            if s.phase == GamePhase::Playing {
                let held = keys.borrow();
                if held.contains_key("ArrowLeft") {
                    move_ship(&mut s.ship, Direction::Left, &movement, ship_left, ship_right);
                }
                if held.contains_key("ArrowRight") {
                    move_ship(&mut s.ship, Direction::Right, &movement, ship_left, ship_right);
                }
                if held.contains_key(" ") {
                    let had_bullet = s.bullet.is_some();
                    fire(&mut s);
                    if !had_bullet && s.bullet.is_some() {
                        if let Some(ref snd) = sound { snd.play_player_fire(); }
                    }
                    // Try to spawn UFO with a randomly chosen direction
                    let direction = if Math::random() < 0.5 { 1i8 } else { -1i8 };
                    try_spawn_ufo(&mut s, direction, viewport_w, grid_top);
                }
                step_bullet(&mut s, grid_top);
                // Check UFO hit — detect transition to explosion state for sound
                let ufo_was_alive = s.ufo.as_ref().map(|u| u.explosion_timer == 0).unwrap_or(false);
                let ufo_score = UFO_SCORES[(Math::random() * UFO_SCORES.len() as f64) as usize];
                check_ufo_hit(&mut s, ufo_score);
                let ufo_just_hit = ufo_was_alive
                    && s.ufo.as_ref().map(|u| u.explosion_timer > 0).unwrap_or(false);
                if ufo_just_hit {
                    if let Some(ref mut snd) = sound {
                        snd.stop_ufo_sound();
                        snd.play_ufo_hit();
                    }
                    ufo_sound_active = false;
                }
                // Collision: compute current grid canvas origin from live offsets
                let cur_grid_left = base_grid_left + s.grid.offset_x;
                let cur_grid_top  = grid_top + s.grid.offset_y;
                let alive_before = s.aliens.iter().filter(|a| a.alive).count();
                check_bullet_hit(&mut s, cur_grid_left, cur_grid_top);
                if s.aliens.iter().filter(|a| a.alive).count() < alive_before {
                    if let Some(ref snd) = sound { snd.play_alien_explosion(); }
                }
                let speed = ClassicSpeed { total_aliens: 55, speed_scale: s.speed_scale };
                // Advance march engine — tempo locked to grid tick interval
                let alive_count = s.aliens.iter().filter(|a| a.alive).count();
                let tick_interval = speed.tick_interval(alive_count);
                if let Some(ref mut snd) = sound {
                    if let Some(note) = snd.march.tick(tick_interval) {
                        snd.play_march_note(note);
                    }
                }
                step_grid(&mut s, &speed, max_offset_x);

                // Alien shooting — fire from a cycling column every interval (from level spec)
                let f = {
                    let mut fc = frame.borrow_mut();
                    *fc += 1;
                    *fc
                };
                // Detect ship hit for explosion sound
                let lives_before = s.lives;
                check_alien_hit_ship(&mut s);
                if s.lives < lives_before {
                    if let Some(ref snd) = sound { snd.play_ship_explosion(); }
                }
                // Bullet clears when it passes below the ship, not the canvas bottom
                let bullet_floor = s.ship.y + SHIP_HALF_H;
                step_alien_bullets(&mut s, bullet_floor);
                let fire_interval = s.alien_fire_interval;
                if f % fire_interval == 0 {
                    let col = (f / fire_interval) % GRID_COLS;
                    fire_alien_bullet(&mut s, col, cur_grid_left, cur_grid_top);
                }
                check_invasion(&mut s, grid_top);
                check_level_clear(&mut s);
            }
            // UFO flyby sound — start/stop continuous tone as UFO appears/disappears.
            // Also stop immediately on game over so no sound plays during the game-over screen.
            {
                let ufo_now_active = s.ufo.as_ref().map(|u| u.explosion_timer == 0).unwrap_or(false)
                    && s.phase != GamePhase::GameOver;
                if ufo_now_active && !ufo_sound_active {
                    if let Some(ref mut snd) = sound { snd.start_ufo_sound(); }
                    ufo_sound_active = true;
                } else if !ufo_now_active && ufo_sound_active {
                    if let Some(ref mut snd) = sound { snd.stop_ufo_sound(); }
                    ufo_sound_active = false;
                }
            }
            // These run outside the Playing guard — each owns its respective phase
            tick_level_clear(&mut s);
            tick_game_over(&mut s);
            tick_explosions(&mut s);
            tick_ground_explosions(&mut s);
            tick_ufo(&mut s, viewport_w);
        }

        // ── Draw ──────────────────────────────────────────────────────────────
        context.clear_rect(0.0, 0.0, viewport_w, viewport_h);
        draw_scene(&context, &state.borrow(), &sprites.borrow(), viewport_w, viewport_h);

        // Schedule next frame
        web_sys::window()
            .unwrap()
            .request_animation_frame(
                raf_cb.borrow().as_ref().unwrap().as_ref().unchecked_ref(),
            )
            .unwrap();
    }) as Box<dyn FnMut()>));

    // Kick off the first frame
    web_sys::window()
        .unwrap()
        .request_animation_frame(
            raf_cb_init.borrow().as_ref().unwrap().as_ref().unchecked_ref(),
        )
        .unwrap();
}

// ── Scene renderer ────────────────────────────────────────────────────────────

fn draw_scene(
    ctx: &CanvasRenderingContext2d,
    state: &GameState,
    sprites: &HashMap<&'static str, HtmlImageElement>,
    viewport_w: f64,
    viewport_h: f64,
) {
    let grid_left = (viewport_w - GRID_W) / 2.0 + state.grid.offset_x;
    let grid_top  = viewport_h * 0.15 + state.grid.offset_y;

    for alien in state.aliens.iter().filter(|a| a.alive || a.explosion_timer > 0) {
        let sprite_name = if !alien.alive {
            match alien.sprite {
                AlienKind::Crab    => "crab_exp",
                AlienKind::Squid   => "squid_exp",
                AlienKind::Octopus => "octopus_exp",
            }
        } else {
            match alien.sprite {
                AlienKind::Crab    => if state.grid.anim_frame { "crab_f2"    } else { "crab"    },
                AlienKind::Squid   => if state.grid.anim_frame { "squid_f2"   } else { "squid"   },
                AlienKind::Octopus => if state.grid.anim_frame { "octopus_f2" } else { "octopus" },
            }
        };
        if let Some(img) = sprites.get(sprite_name) {
            let cell_x = grid_left + alien.col as f64 * CELL_W;
            let cell_y = grid_top  + alien.row as f64 * CELL_H;
            let draw_w = img.natural_width()  as f64 - 8.0;
            let draw_h = img.natural_height() as f64 - 8.0;
            let x = cell_x + (CELL_W - draw_w) / 2.0;
            let y = cell_y + (CELL_H - draw_h) / 2.0;
            ctx.draw_image_with_html_image_element_and_dw_and_dh(img, x, y, draw_w, draw_h)
                .expect("failed to draw alien");
        }
    }

    if let Some(ship_img) = sprites.get("ship") {
        let draw_w = ship_img.natural_width()  as f64;
        let draw_h = ship_img.natural_height() as f64;
        let x = state.ship.x - draw_w / 2.0;
        let y = state.ship.y - draw_h / 2.0;
        ctx.draw_image_with_html_image_element_and_dw_and_dh(ship_img, x, y, draw_w, draw_h)
            .expect("failed to draw ship");
    }

    // Player bullet — 3×12px green rect
    if let Some(ref b) = state.bullet {
        ctx.set_fill_style_str("#68fb35");
        ctx.fill_rect(b.x - 1.5, b.y - 12.0, 3.0, 12.0);
    }

    // Alien bullets — per-kind colour, 3×12px rects
    for ab in &state.alien_bullets {
        ctx.set_fill_style_str(ab.kind.bullet_profile().color);
        ctx.fill_rect(ab.x - 1.5, ab.y, 3.0, 12.0);
    }

    // Ground explosions — small burst for squid bullets hitting the floor
    for ge in &state.ground_explosions {
        let alpha = ge.timer as f64 / game::GROUND_EXPLOSION_FRAMES as f64;
        ctx.set_global_alpha(alpha);
        ctx.set_fill_style_str(AlienKind::Squid.bullet_profile().color);
        ctx.fill_rect(ge.x - 8.0, ge.y - 6.0, 16.0, 6.0);
        ctx.fill_rect(ge.x - 4.0, ge.y - 10.0, 8.0, 4.0);
        ctx.set_global_alpha(1.0);
    }

    // UFO — sprite while alive; score text while exploding
    if let Some(ref ufo) = state.ufo {
        if ufo.explosion_timer > 0 {
            // Flash the score value at the hit position
            ctx.set_fill_style_str("#ff4444");
            ctx.set_text_align("center");
            ctx.set_font("bold 20px monospace");
            ctx.fill_text(
                &ufo.score.to_string(),
                ufo.x + UFO_W / 2.0,
                ufo.y + UFO_H / 2.0 + 7.0,
            ).expect("fill_text failed");
        } else if let Some(img) = sprites.get("ufo") {
            ctx.draw_image_with_html_image_element_and_dw_and_dh(
                img, ufo.x, ufo.y, UFO_W, UFO_H,
            ).expect("failed to draw ufo");
        }
    }

    // Attract screen
    if state.phase == GamePhase::Attract {
        ctx.set_fill_style_str("rgba(0,0,0,0.75)");
        ctx.fill_rect(0.0, 0.0, viewport_w, viewport_h);
        ctx.set_fill_style_str("#68fb35");
        ctx.set_text_align("center");
        ctx.set_font("bold 64px monospace");
        ctx.fill_text("SPACE INVADERS", viewport_w / 2.0, viewport_h / 2.0 - 40.0)
            .expect("fill_text failed");
        ctx.set_font("bold 24px monospace");
        ctx.fill_text("PRESS SPACE TO START", viewport_w / 2.0, viewport_h / 2.0 + 30.0)
            .expect("fill_text failed");
    }

    // Paused overlay
    if state.phase == GamePhase::Paused {
        ctx.set_fill_style_str("rgba(0,0,0,0.55)");
        ctx.fill_rect(0.0, 0.0, viewport_w, viewport_h);
        ctx.set_fill_style_str("#68fb35");
        ctx.set_text_align("center");
        ctx.set_font("bold 64px monospace");
        ctx.fill_text("PAUSED", viewport_w / 2.0, viewport_h / 2.0 - 20.0)
            .expect("fill_text failed");
        ctx.set_font("bold 20px monospace");
        ctx.fill_text("P — RESUME   Q — QUIT", viewport_w / 2.0, viewport_h / 2.0 + 40.0)
            .expect("fill_text failed");
    }

    // Level clear overlay
    if state.phase == GamePhase::LevelClear {
        ctx.set_fill_style_str("rgba(0,0,0,0.45)");
        ctx.fill_rect(0.0, 0.0, viewport_w, viewport_h);
        ctx.set_fill_style_str("#68fb35");
        ctx.set_font("bold 64px monospace");
        ctx.set_text_align("center");
        ctx.fill_text("LEVEL CLEAR", viewport_w / 2.0, viewport_h / 2.0)
            .expect("fill_text failed");
    }

    // Game over overlay
    if state.phase == GamePhase::GameOver {
        ctx.set_fill_style_str("rgba(0,0,0,0.55)");
        ctx.fill_rect(0.0, 0.0, viewport_w, viewport_h);
        ctx.set_fill_style_str("#68fb35");
        ctx.set_text_align("center");
        let mid_y = if state.game_over_timer >= GAME_OVER_PAUSE {
            // Shift "GAME OVER" up to make room for the prompt below
            viewport_h / 2.0 - 40.0
        } else {
            viewport_h / 2.0
        };
        ctx.set_font("bold 64px monospace");
        ctx.fill_text("GAME OVER", viewport_w / 2.0, mid_y)
            .expect("fill_text failed");
        if state.game_over_timer >= GAME_OVER_PAUSE {
            ctx.set_font("bold 24px monospace");
            ctx.fill_text("PRESS SPACE TO PLAY AGAIN", viewport_w / 2.0, mid_y + 70.0)
                .expect("fill_text failed");
        }
    }

    draw_hud(ctx, state, sprites, viewport_w);
}

// ── HUD ───────────────────────────────────────────────────────────────────────

const HUD_MARGIN: f64 = 24.0;
const HUD_BASELINE: f64 = 36.0; // y baseline for text / icons

fn draw_hud(
    ctx: &CanvasRenderingContext2d,
    state: &GameState,
    sprites: &HashMap<&'static str, HtmlImageElement>,
    viewport_w: f64,
) {
    ctx.set_fill_style_str("#68fb35");
    ctx.set_font("bold 24px monospace");

    // Score — left-aligned
    ctx.set_text_align("left");
    ctx.fill_text(&format!("SCORE  {:>6}", state.score), HUD_MARGIN, HUD_BASELINE)
        .expect("fill_text failed");

    // Level — centred
    ctx.set_text_align("center");
    ctx.fill_text(&format!("LEVEL  {}", state.level + 1), viewport_w / 2.0, HUD_BASELINE)
        .expect("fill_text failed");

    // Lives — ship icons, right-aligned
    if let Some(ship_img) = sprites.get("ship") {
        // Draw ship icons from right edge inward; scale to ~half sprite size
        let icon_w = ship_img.natural_width()  as f64 * 0.6;
        let icon_h = ship_img.natural_height() as f64 * 0.6;
        let gap    = icon_w + 8.0;
        for i in 0..state.lives {
            let x = viewport_w - HUD_MARGIN - icon_w - i as f64 * gap;
            let y = HUD_BASELINE - icon_h;
            ctx.draw_image_with_html_image_element_and_dw_and_dh(ship_img, x, y, icon_w, icon_h)
                .expect("failed to draw life icon");
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::game::{centered_position, quarter_size, GameState};

    #[test]
    fn game_state_initialises_with_correct_dimensions() {
        let state = GameState::new(800, 600);
        assert_eq!(state.width, 800);
        assert_eq!(state.height, 600);
    }

    #[test]
    fn quarter_size_reduces_dimensions_by_four() {
        let (w, h) = quarter_size(800.0, 600.0);
        assert_eq!(w, 200.0);
        assert_eq!(h, 150.0);
    }

    #[test]
    fn centered_position_places_image_in_middle_of_canvas() {
        let (x, y) = centered_position(800.0, 600.0, 200.0, 150.0);
        assert_eq!(x, 300.0);
        assert_eq!(y, 225.0);
    }
}
