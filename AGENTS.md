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
Deployed to GitHub Pages via GitHub Actions on push to `main`.

## Pre-merge checklist

Before declaring a feature ready to merge, always run:

```
cargo test
cargo build --target wasm32-unknown-unknown
```

Both commands must complete with **zero errors and zero warnings**. In particular:

- `unused import` — remove the import from the use list
- `unused variable` — prefix with `_` or remove it
- `dead_code` — remove the item or add `#[allow(dead_code)]` only if deliberately kept

Do not suppress warnings with `#[allow(...)]` to pass the check — fix the underlying issue.

## Stack

- **Rust** + **wasm-bindgen** + **web-sys**
- **Trunk** for building (no JS tooling)
- Output goes to `/docs` locally (gitignored — never commit build artifacts)
- CI builds and deploys automatically via `.github/workflows/deploy.yml`
- Static fallback (`<img>`) for environments without Wasm support
