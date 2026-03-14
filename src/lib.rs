pub mod game;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, HtmlImageElement};

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

    let context = canvas
        .get_context("2d")
        .unwrap()
        .unwrap()
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .expect("failed to get 2d context");

    let image = HtmlImageElement::new().expect("failed to create image element");
    image.set_src("assets/crab.png");

    let context = std::rc::Rc::new(context);
    let image = std::rc::Rc::new(image);

    let context_clone = context.clone();
    let image_clone = image.clone();

    let onload = Closure::wrap(Box::new(move || {
        let draw_w = image_clone.natural_width() as f64;
        let draw_h = image_clone.natural_height() as f64;
        let (x, y) = game::centered_position(
            viewport_w,
            viewport_h,
            draw_w,
            draw_h,
        );
        context_clone
            .draw_image_with_html_image_element_and_dw_and_dh(
                &image_clone,
                x,
                y,
                draw_w,
                draw_h,
            )
            .expect("failed to draw image");

    }) as Box<dyn FnMut()>);

    image.set_onload(Some(onload.as_ref().unchecked_ref()));
    onload.forget();
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
