# Corrections Log

A running record of things that needed to be corrected during development.
Reviewed regularly to improve how we work together.

Each entry covers: what went wrong, what was corrected, and the lesson.

---

## 2026-03

### Non-ASCII characters in GLSL source strings

**What went wrong:** Unicode box-drawing and arrow characters (e.g. `──`, `→`) were included inside Rust raw string literals that were passed to the WebGL shader compiler. The GLSL compiler rejected them, causing a runtime panic in the browser.

**Corrected by:** Replacing all non-ASCII characters in the GLSL source with plain ASCII equivalents.

**Lesson:** GLSL ES 1.00 source must be pure ASCII. Any decorative characters in comments need to be stripped before the string reaches the compiler. A Python check script was used to detect them; consider adding this as a pre-build sanity check.

---

### Uniform optimised away by GLSL compiler

**What went wrong:** `u_resolution_x` was declared as a uniform in the CRT fragment shader but never used in any expression. The GLSL compiler optimised it out. `get_uniform_location` returned `None`, which propagated through `.ok_or().?` and caused a runtime panic.

**Corrected by:** Removing the unused uniform from both the GLSL source and the Rust struct.

**Lesson:** Only declare uniforms that are actually referenced in the shader body. If a uniform is declared "for future use", the Rust side must treat `None` from `get_uniform_location` as acceptable rather than an error.

---

### CRT overlay canvas misaligned with game canvas

**What went wrong:** The game canvas was a flex item centred by `display:flex` on the body. The CRT overlay was `position:absolute; top:0; left:0`. When the browser devtools were open, `window.innerHeight` was less than `100vh`, so the flex layout placed the game canvas at a Y offset while the overlay stayed at the top — they were visibly misaligned.

**Corrected by:** Setting both canvases to `position:absolute; top:0; left:0` in Rust, and hiding the game canvas (`visibility:hidden`) so only the CRT overlay is visible. The game canvas is still written to and uploaded as a WebGL texture each frame.

**Lesson:** When layering canvases, flex/block layout and absolute positioning cannot be mixed. Both elements must use the same positioning model.

---

### S / P / Q keys firing game actions during name entry

**What went wrong:** The keydown handler pushed typed characters to both `typed_chars` (name input buffer) and `keys` (action key map). Pressing S while entering a high score name toggled sound; P paused the game; Q quit to the attract screen.

**Corrected by:** Adding an `accepting_text` guard — when the phase is `NameEntry`, the S/P/Q action handlers are skipped entirely.

**Lesson:** Any key that is also a valid name character must be excluded from action detection during text-input phases. When adding new action keys, check whether they conflict with printable ASCII.

---

### Wrong web-sys method name for canvas texture upload

**What went wrong:** The code called `tex_image_2d_with_u32_and_u32_and_html_canvas_element`, which does not exist in web-sys. This caused a compile error.

**Corrected by:** Using the correct method name: `tex_image_2d_with_u32_and_u32_and_canvas`.

**Lesson:** web-sys method names are generated from the WebIDL spec and are not always intuitive. When in doubt, grep the web-sys source or generated bindings rather than guessing.

---

### README incorrectly described deployment as manual

**What went wrong:** README said "No CI pipeline — build locally and commit the output" and showed `git add docs/` instructions. A full GitHub Actions pipeline (`deploy.yml`) had existed from the start and builds/deploys automatically on push to `main`.

**Corrected by:** Updating README to describe the actual CI workflow and removing the manual `docs/` commit instructions.

**Lesson:** README deployment instructions drifted from reality early and were never corrected. The pre-merge documentation review step in AGENTS.md is intended to prevent this class of drift.

---

### Level difficulty table in diagrams.md drifted from code

**What went wrong:** The level difficulty table in `diagrams.md` still reflected the original design-time values. The user had manually adjusted `speed_scale` and `grid_y_offset` across all ten levels (to improve game play balance), but the documentation was not updated. All ten `speed_scale` values and most `grid_y_offset` values were wrong. The `ufo_repeat_shots` column was missing entirely.

**Corrected by:** Reading the actual `LEVELS` array from `game.rs` and regenerating the table from code. Also flagged a likely unintended inversion: level 8 `speed_scale` (0.48) is higher than level 7 (0.45), making level 8 slightly slower than level 7.

**Lesson:** Game balance parameters change frequently and documentation falls behind quickly. The pre-merge checklist now explicitly requires reviewing `diagrams.md` against the code before merging.

---

### WebGL explosion invisible on black background (alpha=0 canvas)

**What went wrong:** The 2D game canvas is cleared with `clearRect`, which leaves background pixels as transparent black (`rgba(0,0,0,0)`). When the game canvas is uploaded as a WebGL texture, background fragments have alpha=0. Adding colour to those fragments in the shader had no visible effect because the pixel remained transparent — the HTML body showed through instead.

**Corrected by:** Forcing `gl_FragColor = vec4(color.rgb, 1.0)` in the shader so all output pixels are opaque regardless of texture alpha.

**Lesson:** Additive shader effects on a transparent-background texture produce no visible result on the transparent areas. When debugging a shader effect that isn't appearing, check the alpha channel of the source texture before investigating the colour maths. Ask for a screenshot or browser observation earlier rather than iterating on coordinate maths in the dark.

### Human and LLM/Agent interaction when resolving WebGL explosions

**What went wrong:** The Agent couldn't see the problem and relied on the humans interpretation. This was a complex case, and watching the agent thought process revealed a lot of time was taken exploring different avenues.

**Corrected by:** Asking the Agent to really simplify the steps to take to resolve the issue. Remove the distracting alternative pathways and get human feedback in quick steps.

**Lesson:** When the output is visual and only the human can see it, reasoning without data is guessing. The trigger to stop and ask is: "I cannot verify this myself." The fix is to reduce the problem space — strip back to the smallest verifiable change, get a human observation, then build from there. Temporary functionality loss is acceptable and often faster than preserving it while debugging. When the human gives meta-feedback mid-task ("I'd approach this in smaller steps"), stop the current approach immediately, reset, and propose a minimal next step.
