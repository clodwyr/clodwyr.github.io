pub struct GameState {
    pub width: u32,
    pub height: u32,
}

impl GameState {
    pub fn new(width: u32, height: u32) -> Self {
        GameState { width, height }
    }
}

pub fn quarter_size(width: f64, height: f64) -> (f64, f64) {
    (width / 4.0, height / 4.0)
}
