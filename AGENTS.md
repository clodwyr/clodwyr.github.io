# Agent Instructions

## Development Approach

Follow strict **red → green → refactor** TDD:

1. **Red** — write a failing test first. Commit it to the feature branch. Do not write implementation code until the test exists and fails for the right reason.
2. **Green** — write the minimum implementation to make the test pass. Commit it. No more.
3. **Refactor** — clean up code without changing behaviour. All tests must remain green. Commit it.

Never skip the red phase. Never write implementation before a test.

## Branching & Merging

- All work happens on a **feature branch** (never commit directly to `main`).
- Commit at each TDD phase: red commit, green commit, refactor commit.
- When a feature is complete, **confirm with the user before merging** — they want to run visual checks first.
- Then **squash merge** the branch into `main` to keep history clean.
- One branch per feature/milestone.

## Project

Space Invaders game built in Rust, compiled to WebAssembly via Trunk.
Deployed to GitHub Pages from the `/docs` folder on `main`.

## Stack

- **Rust** + **wasm-bindgen** + **web-sys**
- **Trunk** for building (no JS tooling)
- Output goes to `/docs`, committed to `main`
- Static fallback (`<img>`) for environments without Wasm support
