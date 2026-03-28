pub mod game;
pub mod shader;
pub mod sound;

use shader::post_processor::PostProcessor;
use sound::SoundEngine;

use game::{
    begin_name_entry, check_alien_hit_ship, check_bullet_hit, check_invasion, check_level_clear,
    check_ufo_hit, close_scoreboard, fire, fire_alien_bullet, handle_name_backspace,
    handle_name_char, move_ship, open_scoreboard, pause_game, quit_game, reset_game,
    step_alien_bullets, step_bullet, step_grid, submit_name, tick_explosions, tick_game_over,
    tick_ground_explosions, tick_level_clear, tick_ufo, try_spawn_ufo,
    AlienKind, ClassicSpeed, CrispMovement, Direction, GamePhase, GameState, Scoreboard,
    ScoreEntry, SpeedStrategy,
    CELL_H, CELL_W, EXPLOSION_FRAMES, GAME_H, GAME_OVER_PAUSE, GAME_W, GRID_COLS, GRID_W,
    MAX_NAME_LEN, PLAY_MARGIN, SHIP_HALF_H, SHIP_STEP, UFO_EXPLOSION_FRAMES, UFO_H, UFO_SCORES, UFO_W,
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

    let canvas = Rc::new(
        document
            .get_element_by_id("canvas")
            .expect("no #canvas element")
            .dyn_into::<HtmlCanvasElement>()
            .expect("#canvas is not a canvas element"),
    );

    let viewport_w = window.inner_width().unwrap().as_f64().unwrap();
    let viewport_h = window.inner_height().unwrap().as_f64().unwrap();
    canvas.set_width(viewport_w as u32);
    canvas.set_height(viewport_h as u32);

    // Pull the game canvas out of the flex flow so it always sits at (0,0).
    // PostProcessor will hide it once the CRT overlay is in place.
    let cs = canvas.style();
    cs.set_property("position", "absolute").unwrap();
    cs.set_property("top", "0").unwrap();
    cs.set_property("left", "0").unwrap();

    let context = Rc::new(
        canvas
            .get_context("2d").unwrap().unwrap()
            .dyn_into::<CanvasRenderingContext2d>()
            .expect("failed to get 2d context"),
    );

    let post = Rc::new(RefCell::new(
        PostProcessor::new(&canvas).unwrap_or_else(|e| {
            web_sys::console::error_2(&"CRT post-processor failed:".into(), &e);
            panic!("failed to create CRT post-processor");
        }),
    ));

    // Shared game state — always in the fixed canonical game coordinate space.
    let state = Rc::new(RefCell::new(
        GameState::new(GAME_W as u32, GAME_H as u32)
    ));
    // Key state: which keys are currently held
    let keys: Rc<RefCell<HashMap<String, bool>>> = Rc::new(RefCell::new(HashMap::new()));
    // Queue of raw key strings from keydown events — drained each frame for text input.
    let typed_chars: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

    // ── Keyboard listeners ────────────────────────────────────────────────────

    {
        let keys_down   = keys.clone();
        let typed_down  = typed_chars.clone();
        let on_keydown = Closure::<dyn FnMut(_)>::new(move |e: KeyboardEvent| {
            let key = e.key();
            keys_down.borrow_mut().insert(key.clone(), true);
            typed_down.borrow_mut().push(key);
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
    const TOTAL: u32 = 8;

    for name in ["crab", "crab_f2", "squid", "squid_f2", "octopus", "octopus_f2", "ship", "ufo"] {
        let img = HtmlImageElement::new().expect("failed to create image");
        img.set_src(&format!("assets/{name}.png"));

        let context_c = context.clone();
        let canvas_c  = canvas.clone();
        let post_c    = post.clone();
        let state_c   = state.clone();
        let sprites_c = sprites.clone();
        let keys_c    = keys.clone();
        let typed_c   = typed_chars.clone();
        let loaded_c  = loaded.clone();

        let onload = Closure::wrap(Box::new(move || {
            *loaded_c.borrow_mut() += 1;
            if *loaded_c.borrow() == TOTAL {
                start_loop(
                    context_c.clone(),
                    canvas_c.clone(),
                    post_c.clone(),
                    state_c.clone(),
                    sprites_c.clone(),
                    keys_c.clone(),
                    typed_c.clone(),
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
    canvas: Rc<HtmlCanvasElement>,
    post: Rc<RefCell<PostProcessor>>,
    state: Rc<RefCell<GameState>>,
    sprites: Rc<RefCell<HashMap<&'static str, HtmlImageElement>>>,
    keys: Rc<RefCell<HashMap<String, bool>>>,
    typed_chars: Rc<RefCell<Vec<String>>>,
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
    let base_grid_left = (GAME_W - GRID_W) / 2.0;
    let play_left      = base_grid_left - PLAY_MARGIN;
    let grid_top       = GAME_H * 0.15;
    let ship_left      = play_left + game::SHIP_HALF_W;
    let ship_right     = play_left + GRID_W + 2.0 * PLAY_MARGIN - game::SHIP_HALF_W;

    // Frame counter — used as a cheap pseudo-random column selector.
    let frame: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));

    // Persistent high score table — loaded from localStorage on startup.
    let mut scoreboard = load_scoreboard();

    // Track prior-frame key state for rising-edge (fresh-press) detection.
    let mut space_was_held = false;
    let mut p_was_held = false;
    let mut q_was_held = false;
    let mut h_was_held = false;
    let mut s_was_held = false;

    // Sound engine — created here, resumed on first Space press (autoplay policy).
    let mut sound: Option<SoundEngine> = SoundEngine::new().ok();
    let mut sound_initialized = false;
    // UFO sound state — tracks whether the continuous tone is running.
    let mut ufo_sound_active = false;

    // Per-alien distortion: ~0.5 s pop on 1-2 aliens, then quiet for 2-4 s.
    let mut dist_targets: Vec<[f32; 2]> = vec![];
    let mut dist_timer: u32 = 0;   // counts down; 0 = time to transition
    let mut dist_on: bool = false;  // true = currently showing effect

    *raf_cb_init.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        // ── Update ────────────────────────────────────────────────────────────
        {
            let mut s = state.borrow_mut();

            // Detect fresh key presses (rising edge — not held keys).
            let space_now = keys.borrow().contains_key(" ");
            let p_now     = keys.borrow().contains_key("p") || keys.borrow().contains_key("P");
            let q_now     = keys.borrow().contains_key("q") || keys.borrow().contains_key("Q");
            let s_now     = keys.borrow().contains_key("s") || keys.borrow().contains_key("S");
            let h_now     = keys.borrow().contains_key("h") || keys.borrow().contains_key("H");
            let space_just_pressed = space_now && !space_was_held;
            let p_just_pressed     = p_now && !p_was_held;
            let q_just_pressed     = q_now && !q_was_held;
            let s_just_pressed     = s_now && !s_was_held;
            let h_just_pressed     = h_now && !h_was_held;
            space_was_held = space_now;
            p_was_held = p_now;
            q_was_held = q_now;
            s_was_held = s_now;
            h_was_held = h_now;

            if space_just_pressed {
                // Resume AudioContext on first user gesture (browser autoplay policy).
                if !sound_initialized {
                    if let Some(ref snd) = sound { snd.resume(); }
                    sound_initialized = true;
                }
                match s.phase {
                    GamePhase::Attract => reset_game(&mut s),
                    // After the game-over message, move to name entry only if the
                    // score qualifies for the scoreboard; otherwise go straight to
                    // the attract screen.
                    GamePhase::GameOver if s.game_over_timer >= GAME_OVER_PAUSE => {
                        if scoreboard.qualifies(s.score) {
                            begin_name_entry(&mut s);
                        } else {
                            s.phase = GamePhase::Attract;
                        }
                    }
                    _ => {}
                }
            }
            if h_just_pressed {
                match s.phase {
                    GamePhase::Attract    => open_scoreboard(&mut s),
                    GamePhase::Scoreboard => close_scoreboard(&mut s),
                    _ => {}
                }
            }
            // P / Q / S must not fire while the player is typing a name —
            // those letters are valid name characters and the key ends up in
            // both `typed_chars` (name input) and `keys` (action detection).
            let accepting_text = s.phase == GamePhase::NameEntry;
            if p_just_pressed && !accepting_text {
                pause_game(&mut s);
            }
            if q_just_pressed && !accepting_text {
                quit_game(&mut s);
            }
            if s_just_pressed && !accepting_text {
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
                    try_spawn_ufo(&mut s, direction, GAME_W, grid_top);
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
            // ── Name entry ────────────────────────────────────────────────────
            {
                let chars: Vec<String> = typed_chars.borrow_mut().drain(..).collect();
                if s.phase == GamePhase::NameEntry {
                    for key in chars {
                        match key.as_str() {
                            "Enter" => {
                                if let Some(entry) = submit_name(&mut s) {
                                    scoreboard.insert(entry);
                                    save_scoreboard(&scoreboard);
                                }
                                // submit_name transitions to Attract whether name given or not
                            }
                            "Escape" => {
                                // Skip without saving — clear buffer then submit
                                s.name_input.clear();
                                submit_name(&mut s);
                            }
                            "Backspace" => handle_name_backspace(&mut s),
                            k if k.len() == 1 => {
                                if let Some(ch) = k.chars().next() {
                                    handle_name_char(&mut s, ch.to_ascii_uppercase());
                                }
                            }
                            _ => {}
                        }
                    }
                }
                // Outside NameEntry, discard queued chars so they don't accumulate.
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
            tick_ufo(&mut s, GAME_W);
        }

        // ── Draw ──────────────────────────────────────────────────────────────
        context.clear_rect(0.0, 0.0, viewport_w, viewport_h);
        // Centre the fixed game area in the viewport.
        let game_x = ((viewport_w - GAME_W) / 2.0).max(0.0);
        let game_y = ((viewport_h - GAME_H) / 2.0).max(0.0);
        context.save();
        context.translate(game_x, game_y).unwrap();
        draw_scene(&context, &state.borrow(), &sprites.borrow(), &scoreboard, GAME_W, GAME_H);
        context.restore();

        // ── CRT post-process ──────────────────────────────────────────────────
        let rc = (Math::random() * 1024.0) as u32;
        let rb = (Math::random() * 1024.0) as u32;
        let ri = (Math::random() * 1024.0) as u32;

        // Collect all exploding alien data, then split into pre-glitch and spark phases.
        // First 6 frames (t < PRE_T): glitch distortion only, no sparks yet.
        // Remaining frames  (t >= PRE_T): sparks fire, glitch continues alongside.
        const PRE_T: f32 = 6.0 / EXPLOSION_FRAMES as f32;
        let (explosions, pre_glitch_pos): (Vec<[f32; 3]>, Vec<[f32; 2]>) = {
            let s = state.borrow();
            let grid_left = game_x + (GAME_W - GRID_W) / 2.0 + s.grid.offset_x;
            let grid_top  = game_y + GAME_H * 0.15 + s.grid.offset_y;
            let mut exps: Vec<[f32; 3]> = Vec::new();
            let mut pre:  Vec<[f32; 2]> = Vec::new();
            for a in s.aliens.iter().filter(|a| !a.alive && a.explosion_timer > 0) {
                let cx = grid_left + a.col as f64 * CELL_W + CELL_W / 2.0;
                let cy = grid_top  + a.row as f64 * CELL_H + CELL_H / 2.0;
                let u  = cx as f32 / viewport_w as f32;
                let v  = cy as f32 / viewport_h as f32;
                let t  = 1.0 - a.explosion_timer as f32 / EXPLOSION_FRAMES as f32;
                if t < PRE_T {
                    if pre.len() < 2 { pre.push([u, v]); }
                } else if exps.len() < 8 {
                    exps.push([u, v, t]);
                }
            }
            if exps.len() < 8 {
                if let Some(ref ufo) = s.ufo {
                    if ufo.explosion_timer > 0 {
                        let cx = game_x + ufo.x + UFO_W / 2.0;
                        let cy = game_y + ufo.y + UFO_H / 2.0;
                        let t  = 1.0 - ufo.explosion_timer as f32 / UFO_EXPLOSION_FRAMES as f32;
                        exps.push([cx as f32 / viewport_w as f32, cy as f32 / viewport_h as f32, t]);
                    }
                }
            }
            (exps, pre)
        };

        // Per-alien distortion: pop for ~30 frames (0.5 s), quiet for 2-4 s.
        if dist_timer == 0 {
            if dist_on {
                // End of pop -- clear targets, start quiet gap.
                dist_targets.clear();
                dist_on    = false;
                dist_timer = 120 + (Math::random() * 120.0) as u32;
            } else {
                // Start a new pop -- pick 1-2 alive aliens.
                let s = state.borrow();
                let grid_left = game_x + (GAME_W - GRID_W) / 2.0 + s.grid.offset_x;
                let grid_top  = game_y + GAME_H * 0.15 + s.grid.offset_y;
                let alive: Vec<_> = s.aliens.iter().filter(|a| a.alive).collect();
                let n = alive.len();
                if n > 0 {
                    // Random offset so distortion is not always centred on the sprite
                    let jitter = || (Math::random() as f32 - 0.5) * 0.04;
                    let to_uv = |a: &&_| {
                        let a: &game::Alien = a;
                        let cx = grid_left + a.col as f64 * CELL_W + CELL_W / 2.0;
                        let cy = grid_top  + a.row as f64 * CELL_H + CELL_H / 2.0;
                        [cx as f32 / viewport_w as f32 + jitter(),
                         cy as f32 / viewport_h as f32 + jitter()]
                    };
                    let i0 = (Math::random() * n as f64) as usize;
                    let i1 = (Math::random() * n as f64) as usize;
                    dist_targets = if i0 == i1 { vec![to_uv(&alive[i0])] }
                                   else        { vec![to_uv(&alive[i0]), to_uv(&alive[i1])] };
                }
                dist_on    = true;
                dist_timer = 30; // ~0.5 s at 60 fps
            }
        } else {
            dist_timer -= 1;
        }

        // Build per-frame distortion list (up to 4 slots):
        //   0-1: random alive-alien pops (held between refreshes)
        //   2-3: pre-glitch phase aliens, then spark-phase aliens alongside their burst
        let mut dist_frame = dist_targets.clone();
        for pos in &pre_glitch_pos {
            if dist_frame.len() >= 4 { break; }
            dist_frame.push(*pos);
        }
        for exp in &explosions {
            if dist_frame.len() >= 4 { break; }
            dist_frame.push([exp[0], exp[1]]);
        }

        post.borrow_mut().process(&canvas, rc, rb, ri, &explosions, &dist_frame);

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

// ── Scoreboard persistence ────────────────────────────────────────────────────

fn get_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn save_scoreboard(board: &Scoreboard) {
    let Some(storage) = get_storage() else { return };
    let data: String = board
        .entries()
        .iter()
        .map(|e| format!("{}\t{}\t{}", e.name, e.score, e.level))
        .collect::<Vec<_>>()
        .join("\n");
    let _ = storage.set_item("si_scores", &data);
}

fn load_scoreboard() -> Scoreboard {
    let mut board = Scoreboard::new();
    let Some(storage) = get_storage() else { return board };
    let Ok(Some(data)) = storage.get_item("si_scores") else { return board };
    for line in data.lines() {
        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        if parts.len() == 3 {
            if let (Ok(score), Ok(level)) = (parts[1].parse::<u32>(), parts[2].parse::<u32>()) {
                board.insert(ScoreEntry { name: parts[0].to_string(), score, level });
            }
        }
    }
    board
}

// ── Scene renderer ────────────────────────────────────────────────────────────

fn draw_scene(
    ctx: &CanvasRenderingContext2d,
    state: &GameState,
    sprites: &HashMap<&'static str, HtmlImageElement>,
    scoreboard: &Scoreboard,
    game_w: f64,
    game_h: f64,
) {
    let grid_left = (game_w - GRID_W) / 2.0 + state.grid.offset_x;
    let grid_top  = game_h * 0.15 + state.grid.offset_y;

    for alien in state.aliens.iter().filter(|a| a.alive) {
        let sprite_name = {
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
        ctx.fill_rect(0.0, 0.0, game_w, game_h);
        ctx.set_fill_style_str("#68fb35");
        ctx.set_text_align("center");
        ctx.set_font("bold 64px monospace");
        ctx.fill_text("SPACE INVADERS", game_w / 2.0, game_h / 2.0 - 40.0)
            .expect("fill_text failed");
        ctx.set_font("bold 24px monospace");
        ctx.fill_text("SPACE — START", game_w / 2.0, game_h / 2.0 + 30.0)
            .expect("fill_text failed");
        ctx.fill_text("H — HIGH SCORES", game_w / 2.0, game_h / 2.0 + 66.0)
            .expect("fill_text failed");
    }

    // Paused overlay
    if state.phase == GamePhase::Paused {
        ctx.set_fill_style_str("rgba(0,0,0,0.55)");
        ctx.fill_rect(0.0, 0.0, game_w, game_h);
        ctx.set_fill_style_str("#68fb35");
        ctx.set_text_align("center");
        ctx.set_font("bold 64px monospace");
        ctx.fill_text("PAUSED", game_w / 2.0, game_h / 2.0 - 20.0)
            .expect("fill_text failed");
        ctx.set_font("bold 20px monospace");
        ctx.fill_text("P — RESUME   Q — QUIT", game_w / 2.0, game_h / 2.0 + 40.0)
            .expect("fill_text failed");
    }

    // Level clear overlay
    if state.phase == GamePhase::LevelClear {
        ctx.set_fill_style_str("rgba(0,0,0,0.45)");
        ctx.fill_rect(0.0, 0.0, game_w, game_h);
        ctx.set_fill_style_str("#68fb35");
        ctx.set_font("bold 64px monospace");
        ctx.set_text_align("center");
        ctx.fill_text("LEVEL CLEAR", game_w / 2.0, game_h / 2.0)
            .expect("fill_text failed");
    }

    // Game over overlay
    if state.phase == GamePhase::GameOver {
        ctx.set_fill_style_str("rgba(0,0,0,0.55)");
        ctx.fill_rect(0.0, 0.0, game_w, game_h);
        ctx.set_fill_style_str("#68fb35");
        ctx.set_text_align("center");
        let mid_y = if state.game_over_timer >= GAME_OVER_PAUSE {
            game_h / 2.0 - 40.0
        } else {
            game_h / 2.0
        };
        ctx.set_font("bold 64px monospace");
        ctx.fill_text("GAME OVER", game_w / 2.0, mid_y)
            .expect("fill_text failed");
        if state.game_over_timer >= GAME_OVER_PAUSE {
            ctx.set_font("bold 24px monospace");
            ctx.fill_text("PRESS SPACE TO CONTINUE", game_w / 2.0, mid_y + 70.0)
                .expect("fill_text failed");
        }
    }

    // Name entry overlay
    if state.phase == GamePhase::NameEntry {
        ctx.set_fill_style_str("rgba(0,0,0,0.80)");
        ctx.fill_rect(0.0, 0.0, game_w, game_h);
        ctx.set_fill_style_str("#68fb35");
        ctx.set_text_align("center");
        let cy = game_h / 2.0;
        ctx.set_font("bold 32px monospace");
        ctx.fill_text(
            &format!("SCORE: {}   LEVEL: {}", state.score, state.level + 1),
            game_w / 2.0, cy - 70.0,
        ).expect("fill_text failed");
        ctx.set_font("bold 24px monospace");
        ctx.fill_text("ENTER YOUR NAME", game_w / 2.0, cy - 20.0)
            .expect("fill_text failed");
        // Input box
        let cursor = if state.name_input.len() < MAX_NAME_LEN { "_" } else { "" };
        ctx.set_font("bold 40px monospace");
        ctx.fill_text(
            &format!("{}{}", state.name_input, cursor),
            game_w / 2.0, cy + 40.0,
        ).expect("fill_text failed");
        ctx.set_font("bold 18px monospace");
        ctx.set_fill_style_str("#aaffaa");
        ctx.fill_text(
            "ENTER — SAVE   ESC — SKIP",
            game_w / 2.0, cy + 100.0,
        ).expect("fill_text failed");
    }

    // Scoreboard screen
    if state.phase == GamePhase::Scoreboard {
        ctx.set_fill_style_str("rgba(0,0,0,0.92)");
        ctx.fill_rect(0.0, 0.0, game_w, game_h);
        ctx.set_fill_style_str("#68fb35");
        ctx.set_text_align("center");
        ctx.set_font("bold 48px monospace");
        ctx.fill_text("HIGH SCORES", game_w / 2.0, 80.0)
            .expect("fill_text failed");

        let cx   = game_w / 2.0;
        let top  = 140.0_f64;
        let row  = 52.0_f64;

        // Header
        ctx.set_font("bold 20px monospace");
        ctx.set_fill_style_str("#aaffaa");
        ctx.fill_text(
            &format!("{:<4}  {:<6}  {:<5}  {}", "RANK", "SCORE", "LEVEL", "NAME"),
            cx, top,
        ).expect("fill_text failed");

        ctx.set_font("bold 26px monospace");
        ctx.set_fill_style_str("#68fb35");
        if scoreboard.entries().is_empty() {
            ctx.fill_text("— NO SCORES YET —", cx, top + row * 1.5)
                .expect("fill_text failed");
        } else {
            for (i, entry) in scoreboard.entries().iter().enumerate() {
                ctx.fill_text(
                    &format!(
                        "{:<4}  {:>6}  {:>5}  {}",
                        i + 1, entry.score, entry.level, entry.name
                    ),
                    cx, top + row * (i as f64 + 1.0),
                ).expect("fill_text failed");
            }
        }

        ctx.set_font("bold 20px monospace");
        ctx.set_fill_style_str("#aaffaa");
        ctx.fill_text("H — BACK", cx, game_h - 40.0)
            .expect("fill_text failed");
    }

    draw_hud(ctx, state, sprites, game_w);
}

// ── HUD ───────────────────────────────────────────────────────────────────────

const HUD_MARGIN: f64 = 24.0;
const HUD_BASELINE: f64 = 36.0; // y baseline for text / icons

fn draw_hud(
    ctx: &CanvasRenderingContext2d,
    state: &GameState,
    sprites: &HashMap<&'static str, HtmlImageElement>,
    game_w: f64,
) {
    ctx.set_fill_style_str("#68fb35");
    ctx.set_font("bold 24px monospace");

    // Score — left-aligned
    ctx.set_text_align("left");
    ctx.fill_text(&format!("SCORE  {:>6}", state.score), HUD_MARGIN, HUD_BASELINE)
        .expect("fill_text failed");

    // Level — centred
    ctx.set_text_align("center");
    ctx.fill_text(&format!("LEVEL  {}", state.level + 1), game_w / 2.0, HUD_BASELINE)
        .expect("fill_text failed");

    // Lives — ship icons, right-aligned
    if let Some(ship_img) = sprites.get("ship") {
        // Draw ship icons from right edge inward; scale to ~half sprite size
        let icon_w = ship_img.natural_width()  as f64 * 0.6;
        let icon_h = ship_img.natural_height() as f64 * 0.6;
        let gap    = icon_w + 8.0;
        for i in 0..state.lives {
            let x = game_w - HUD_MARGIN - icon_w - i as f64 * gap;
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
