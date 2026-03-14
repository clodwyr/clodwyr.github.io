pub mod game;

use game::{
    build_alien_grid, check_alien_hit_ship, check_bullet_hit, check_invasion,
    check_level_clear, fire, fire_alien_bullet, move_ship, step_alien_bullet, step_bullet,
    step_grid, tick_level_clear, AlienKind, ClassicSpeed, CrispMovement, Direction,
    GamePhase, GameState, CELL_H, CELL_W, GRID_COLS, GRID_W, LEVEL_1, PLAY_MARGIN,
    SHIP_HALF_H, SHIP_STEP,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
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
    state.borrow_mut().aliens = build_alien_grid(LEVEL_1);

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
    const TOTAL: u32 = 7;

    for name in ["crab", "crab_f2", "squid", "squid_f2", "octopus", "octopus_f2", "ship"] {
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
    let speed        = ClassicSpeed { total_aliens: 55 };

    // Play area: grid centred with PLAY_MARGIN of breathing room on each side.
    // The grid shifts ±PLAY_MARGIN from centre; ship is bounded to the same area.
    let max_offset_x   = PLAY_MARGIN;
    let base_grid_left = (viewport_w - GRID_W) / 2.0;
    let play_left      = base_grid_left - PLAY_MARGIN;
    let grid_top       = viewport_h * 0.15;
    let ship_left      = play_left + game::SHIP_HALF_W;
    let ship_right     = play_left + GRID_W + 2.0 * PLAY_MARGIN - game::SHIP_HALF_W;

    // Frame counter — used as a cheap pseudo-random column selector.
    // Aliens fire every ALIEN_FIRE_INTERVAL frames from a rotating column.
    const ALIEN_FIRE_INTERVAL: u32 = 90; // ~1.5 s at 60 fps — easy to tune
    let frame: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));

    *raf_cb_init.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        // ── Update ────────────────────────────────────────────────────────────
        {
            let mut s = state.borrow_mut();

            if s.phase == GamePhase::Playing {
                let held = keys.borrow();
                if held.contains_key("ArrowLeft") {
                    move_ship(&mut s.ship, Direction::Left, &movement, ship_left, ship_right);
                }
                if held.contains_key("ArrowRight") {
                    move_ship(&mut s.ship, Direction::Right, &movement, ship_left, ship_right);
                }
                if held.contains_key(" ") {
                    fire(&mut s);
                }
                step_bullet(&mut s, grid_top);
                // Collision: compute current grid canvas origin from live offsets
                let cur_grid_left = base_grid_left + s.grid.offset_x;
                let cur_grid_top  = grid_top + s.grid.offset_y;
                check_bullet_hit(&mut s, cur_grid_left, cur_grid_top);
                step_grid(&mut s, &speed, max_offset_x);

                // Alien shooting — fire from a cycling column every interval
                let f = {
                    let mut fc = frame.borrow_mut();
                    *fc += 1;
                    *fc
                };
                // Check hit against current bullet position BEFORE stepping,
                // so the bullet can't be cleared past the floor before the
                // collision is tested.
                check_alien_hit_ship(&mut s);
                // Bullet clears when it passes below the ship, not the canvas bottom
                let bullet_floor = s.ship.y + SHIP_HALF_H;
                if f % ALIEN_FIRE_INTERVAL == 0 {
                    let col = (f / ALIEN_FIRE_INTERVAL) % GRID_COLS;
                    step_alien_bullet(&mut s, bullet_floor);
                    fire_alien_bullet(&mut s, col, cur_grid_left, cur_grid_top);
                } else {
                    step_alien_bullet(&mut s, bullet_floor);
                }
                check_invasion(&mut s, grid_top);
                check_level_clear(&mut s);
            }
            // tick_level_clear runs outside the Playing guard — it owns the LevelClear phase
            tick_level_clear(&mut s);
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

    for alien in state.aliens.iter().filter(|a| a.alive) {
        let sprite_name = match alien.sprite {
            AlienKind::Crab    => if state.grid.anim_frame { "crab_f2"    } else { "crab"    },
            AlienKind::Squid   => if state.grid.anim_frame { "squid_f2"   } else { "squid"   },
            AlienKind::Octopus => if state.grid.anim_frame { "octopus_f2" } else { "octopus" },
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

    // Alien bullet — 3×12px red rect
    if let Some(ref ab) = state.alien_bullet {
        ctx.set_fill_style_str("#ff4444");
        ctx.fill_rect(ab.x - 1.5, ab.y, 3.0, 12.0);
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
        ctx.set_font("bold 64px monospace");
        ctx.set_text_align("center");
        ctx.fill_text("GAME OVER", viewport_w / 2.0, viewport_h / 2.0)
            .expect("fill_text failed");
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
