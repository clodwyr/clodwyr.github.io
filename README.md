# Clodwyr in Space

Space Invaders built in Rust, compiled to WebAssembly, deployed to GitHub Pages.

## Stack

- **Rust** + **wasm-bindgen** + **web-sys** — game logic and DOM/canvas access
- **Trunk** — builds and bundles the WASM (no JS tooling)
- **GitHub Pages** — served from the `/docs` folder on `main`

A static `<img>` fallback is shown for browsers without Canvas/WebAssembly support.

## Prerequisites

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk
```

## Development

```sh
trunk serve        # live-reload dev server at http://localhost:8080
cargo test         # run unit tests (native, no browser needed)
```

## Building for deployment

```sh
trunk build        # outputs to /docs
git add docs/
git commit -m "..."
git push
```

GitHub Pages serves from the `/docs` folder on `main`. No CI pipeline — build locally and commit the output.

## Regenerating sprites

Sprites live in `assets/` as PNG files. They are generated from ASCII pixel art defined in `src/bin/gen_sprites.rs`. To regenerate after editing the patterns:

```sh
cargo run --bin gen-sprites
```

Each sprite is defined as rows of `#` (green pixel) and `.` (transparent). The scale factor and output dimensions are documented inline:

| Sprite  | Grid   | Scale | Output   |
|---------|--------|-------|----------|
| crab    | 20×12  | 3     | 60×36px  |
| squid   | 12×8   | 5     | 60×40px  |
| octopus | 12×8   | 5     | 60×40px  |
| ufo     | 16×6   | 5     | 80×30px  |
| ship    | 11×4   | 5     | 55×20px  |

## Project structure

```
├── src/
│   ├── lib.rs          # WASM entry point (#[wasm_bindgen(start)])
│   ├── game.rs         # Pure game logic (unit tested)
│   └── bin/
│       └── gen_sprites.rs  # Sprite generator (cargo run --bin gen-sprites)
├── assets/             # Source sprites (PNG)
├── docs/               # Built output — committed to main, served by Pages
├── index.html          # Trunk entry point
├── Trunk.toml          # dist = "docs"
└── AGENTS.md           # AI agent instructions (TDD, branching)
```

## Workflow

All work happens on a feature branch. The TDD cycle is:

1. **Red** — write a failing test, commit
2. **Green** — implement the minimum to pass, commit
3. **Refactor** — clean up without breaking tests, commit

Then do a visual check in the browser and squash merge into `main`.
