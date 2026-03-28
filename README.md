# Clodwyr in Space

Space Invaders built in Rust, compiled to WebAssembly, deployed to GitHub Pages.

## Stack

- **Rust** + **wasm-bindgen** + **web-sys** — game logic and DOM/canvas access
- **Trunk** — builds and bundles the WASM (no JS tooling)
- **WebGL** — CRT post-process shader layer (barrel distortion, scanlines, glitch)
- **GitHub Actions** — builds and deploys to GitHub Pages on every push to `main`

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

Push to `main` — GitHub Actions runs `trunk build --release` and deploys the
output automatically. There is no need to build or commit the `docs/` folder
locally; it is produced entirely by CI.

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

## Controls

| Key        | Action                                      |
|------------|---------------------------------------------|
| Space      | Start game / confirm on game-over screen    |
| ← →        | Move ship                                   |
| Space      | Fire                                        |
| P          | Pause / resume                              |
| Q          | Quit to attract screen                      |
| S          | Toggle sound on/off                         |
| H          | View high score table (from attract screen) |
| Enter      | Submit name on high score entry             |
| Escape     | Skip name entry                             |
| Backspace  | Delete last character during name entry     |

## Project structure

```
├── src/
│   ├── lib.rs              # WASM entry point (#[wasm_bindgen(start)])
│   ├── game.rs             # Pure game logic (unit tested)
│   ├── sound.rs            # Web Audio sound engine
│   ├── shader/
│   │   ├── mod.rs
│   │   ├── glitch.rs       # GlitchTimer state machine (unit tested)
│   │   ├── post_processor.rs  # WebGL CRT overlay canvas
│   │   └── glsl/
│   │       ├── quad.vert   # Fullscreen quad vertex shader
│   │       └── crt.frag    # CRT effect fragment shader
│   └── bin/
│       └── gen_sprites.rs  # Sprite generator (cargo run --bin gen-sprites)
├── assets/                 # Source sprites (PNG)
├── docs/                   # Built output — produced by CI, do not commit manually
├── index.html              # Trunk entry point
├── Trunk.toml              # dist = "docs"
├── diagrams.md             # Architecture diagrams and level reference table
└── AGENTS.md               # AI agent instructions (TDD, branching, pre-merge checklist)
```

## Workflow

All work happens on a feature branch. The TDD cycle is:

1. **Red** — write a failing test, commit
2. **Green** — implement the minimum to pass, commit
3. **Refactor** — clean up without breaking tests, commit

Then do a visual check in the browser and squash merge into `main`.

## Research

[Space Invaders Design](https://robbiegrier.github.io/assets/research/SpaceInvadersDesignDocFinal.pdf)
