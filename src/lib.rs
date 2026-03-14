pub mod game;

use game::{build_alien_grid, AlienKind, GameState, LEVEL_1};
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, HtmlImageElement, CanvasRenderingContext2d};

// Cell size in pixels — large enough for the widest/tallest sprite plus padding
const CELL_W: f64 = 64.0;
const CELL_H: f64 = 48.0;
const COLS: u32 = 11;
const ROWS: u32 = 5;

fn grid_pixel_width() -> f64  { COLS as f64 * CELL_W }
#[allow(dead_code)]
fn grid_pixel_height() -> f64 { ROWS as f64 * CELL_H }

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

    let state = Rc::new(GameState::new(viewport_w as u32, viewport_h as u32));

    // Load the three alien sprites and the ship sprite.
    // We track how many have loaded; once all 4 are ready we draw.
    let sprites: Rc<RefCell<HashMap<&'static str, HtmlImageElement>>> =
        Rc::new(RefCell::new(HashMap::new()));
    let loaded = Rc::new(RefCell::new(0u32));
    const TOTAL: u32 = 4;

    for name in ["crab", "squid", "octopus", "ship"] {
        let img = HtmlImageElement::new().expect("failed to create image");
        img.set_src(&format!("assets/{name}.png"));

        let context_c  = context.clone();
        let state_c    = state.clone();
        let sprites_c  = sprites.clone();
        let loaded_c   = loaded.clone();

        let onload = Closure::wrap(Box::new(move || {
            *loaded_c.borrow_mut() += 1;
            if *loaded_c.borrow() == TOTAL {
                draw_scene(&context_c, &state_c, &sprites_c.borrow(), viewport_w, viewport_h);
            }
        }) as Box<dyn FnMut()>);

        img.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget();

        sprites.borrow_mut().insert(name, img);
    }
}

fn draw_scene(
    ctx: &CanvasRenderingContext2d,
    state: &GameState,
    sprites: &HashMap<&'static str, HtmlImageElement>,
    viewport_w: f64,
    viewport_h: f64,
) {
    // Grid top-left: centred horizontally, starting 15% down from top
    let grid_left = (viewport_w - grid_pixel_width()) / 2.0;
    let grid_top  = viewport_h * 0.15;

    let aliens = build_alien_grid(LEVEL_1);

    for alien in &aliens {
        let sprite_name = match alien.sprite {
            AlienKind::Crab    => "crab",
            AlienKind::Squid   => "squid",
            AlienKind::Octopus => "octopus",
        };
        if let Some(img) = sprites.get(sprite_name) {
            let cell_x = grid_left + alien.col as f64 * CELL_W;
            let cell_y = grid_top  + alien.row as f64 * CELL_H;
            // Centre the sprite inside its cell, drawn 4px smaller for spacing
            let draw_w = img.natural_width()  as f64 - 8.0;
            let draw_h = img.natural_height() as f64 - 8.0;
            let x = cell_x + (CELL_W - draw_w) / 2.0;
            let y = cell_y + (CELL_H - draw_h) / 2.0;
            ctx.draw_image_with_html_image_element_and_dw_and_dh(img, x, y, draw_w, draw_h)
                .expect("failed to draw alien");
        }
    }

    // Ship — centred horizontally, near bottom
    if let Some(ship_img) = sprites.get("ship") {
        let draw_w = ship_img.natural_width()  as f64;
        let draw_h = ship_img.natural_height() as f64;
        let x = state.ship.x - draw_w / 2.0;
        let y = state.ship.y - draw_h / 2.0;
        ctx.draw_image_with_html_image_element_and_dw_and_dh(ship_img, x, y, draw_w, draw_h)
            .expect("failed to draw ship");
    }
}

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
