use image::{ImageBuffer, Rgba};
use std::path::Path;

// Colours
const GREEN: Rgba<u8> = Rgba([104, 251, 53, 255]);
const WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);
const TRANSPARENT: Rgba<u8> = Rgba([0, 0, 0, 0]);

// ── Sprite pixel art ────────────────────────────────────────────────────────
// Each row must be the same width within a sprite. '#' = filled, '.' = empty.

// 16×8 sprite — scale=3 → 60×36px
const CRAB: &[&str] = &[
    "...#........#...",
    "....#......#....",
    "...##########...",
    "..###.####.###..",
    ".##############.",
    ".#.##########.#.",
    ".#.#........#.#.",
    "....###..###....",
];

const CRAB_FRAME_2: &[&str] = &[
  "...#........#...",
  ".#..#......#..#.",
  ".#.##########.#.",
  ".####.####.####.",
  ".##############.",
  "..############..",
  "...#........#...",
  "..#..........#..",
];

// 12×8 sprite — scale=5 → 60×40px
const SQUID: &[&str] = &[
  "....##....",
  "...####...",
  "..######..",
  ".##.##.##.",
  ".########.",
  "..#.##.#..",
  ".#......#.",
  "..#....#..",
];

const SQUID_FRAME_2: &[&str] = &[
    "....##....",
    "...####...",
    "..######..",
    ".##.##.##.",
    ".########.",
    "...#..#...",
    "..#.##.#..",
    ".#.#..#.#.",
];

// 12×8 sprite — scale=5 → 60×40px
const OCTOPUS_FRAME_2: &[&str] = &[
    "..########..",
    ".##########.",
    "############",
    "###..##..###",
    "############",
    "..###..###..",
    ".##..##..##.",
    "..##....##..",
];

const OCTOPUS: &[&str] = &[
    "..########..",
    ".##########.",
    "############",
    "###..##..###",
    "############",
    "..##....##..",
    ".##.####.##.",
    "##........##",
];

// ── Explosions ───────────────────────────────────────────────────────────────
// Same dimensions / scale as their parent alien so they fit in the same cell.

// 16×8 sprite — scale=4 (matches crab)
const CRAB_EXP: &[&str] = &[
    ".....#....#.....",
    "#...........#...",
    "..#..#..#..#....",
    "....#.....#.....",
    "....#.....#.....",
    "..#..#..#..#....",
    "#...........#...",
    ".....#....#.....",
];

// 10×8 sprite — scale=5 (matches squid)
const SQUID_EXP: &[&str] = &[
    "..#...#...",
    "#.......#.",
    "...#.#....",
    "...###....",
    "...###....",
    "...#.#....",
    "#.......#.",
    "..#...#...",
];

// 12×8 sprite — scale=5 (matches octopus)
const OCTOPUS_EXP: &[&str] = &[
    "..#.....#...",
    "#...#.#...#.",
    "....#.#.....",
    "...#...#....",
    "...#...#....",
    "....#.#.....",
    "#...#.#...#.",
    "..#.....#...",
];

// 16×6 sprite — scale=5 → 80×30px
const UFO: &[&str] = &[
    "....########....",
    "..############..",
    ".####.####.####.",
    "################",
    ".####.####.####.",
    "..##.######.##..",
];

// 11×4 sprite — scale=5 → 55×20px
const SHIP: &[&str] = &[
    ".....#.....",
    "...#####...",
    ".#########.",
    "###########",
];

// ── Renderer ─────────────────────────────────────────────────────────────────

/// Render a pixel-art sprite and save it as a PNG.
/// Each `#` in `pixels` becomes a `scale×scale` block in `colour`; `.` is transparent.
/// All rows in `pixels` must be the same length.
/// Output size: (cols * scale) × (rows * scale) pixels.
fn save_sprite(name: &str, scale: u32, colour: Rgba<u8>, pixels: &[&str]) {
    let rows = pixels.len() as u32;
    let cols = pixels[0].len() as u32;
    assert!(
        pixels.iter().all(|r| r.len() == cols as usize),
        "sprite '{name}': all rows must be the same width"
    );
    let mut img = ImageBuffer::from_pixel(cols * scale, rows * scale, TRANSPARENT);
    for (row, line) in pixels.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            if ch == '#' {
                for dy in 0..scale {
                    for dx in 0..scale {
                        img.put_pixel(col as u32 * scale + dx, row as u32 * scale + dy, colour);
                    }
                }
            }
        }
    }
    let path = format!("assets/{name}.png");
    img.save(Path::new(&path)).unwrap();
    println!("wrote {path}  ({}×{} sprite, scale={scale} → {}×{}px)",
        cols, rows, cols * scale, rows * scale);
}

// ── Generate ─────────────────────────────────────────────────────────────────

fn main() {
    save_sprite("crab",        4, GREEN, CRAB);
    save_sprite("crab_f2",     4, GREEN, CRAB_FRAME_2);
    save_sprite("crab_white",  4, WHITE, CRAB);
    save_sprite("squid",       5, GREEN, SQUID);
    save_sprite("squid_f2",       5, GREEN, SQUID_FRAME_2);
    save_sprite("octopus",     5, GREEN, OCTOPUS);
    save_sprite("octopus_f2",  5, GREEN, OCTOPUS_FRAME_2);
    save_sprite("ufo",         5, GREEN, UFO);
    save_sprite("ship",        5, GREEN, SHIP);
    save_sprite("crab_exp",    4, WHITE, CRAB_EXP);
    save_sprite("squid_exp",   5, WHITE, SQUID_EXP);
    save_sprite("octopus_exp", 5, WHITE, OCTOPUS_EXP);
}
