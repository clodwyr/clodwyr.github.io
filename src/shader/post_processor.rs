use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::{
    HtmlCanvasElement, WebGlBuffer, WebGlProgram, WebGlRenderingContext as GL,
    WebGlShader, WebGlTexture, WebGlUniformLocation,
};

use super::glitch::GlitchTimer;

// ── GLSL source ───────────────────────────────────────────────────────────────

const VERT_SRC: &str = include_str!("glsl/quad.vert");
const FRAG_SRC: &str = include_str!("glsl/crt.frag");

// ── PostProcessor ─────────────────────────────────────────────────────────────

pub struct PostProcessor {
    gl:             GL,
    program:        WebGlProgram,
    quad_buf:       WebGlBuffer,
    texture:        WebGlTexture,
    glitch:         GlitchTimer,
    frame:          u32,
    u_texture:      WebGlUniformLocation,
    u_time:         WebGlUniformLocation,
    u_resolution_y: WebGlUniformLocation,
    u_glitch_on:    WebGlUniformLocation,
    u_glitch_phase: WebGlUniformLocation,
    u_exp_pos:      WebGlUniformLocation,
    u_exp_t:        WebGlUniformLocation,
    u_dist_pos:     WebGlUniformLocation,
    height:         f32,
}

impl PostProcessor {
    pub fn new(source: &HtmlCanvasElement) -> Result<Self, JsValue> {
        let document = web_sys::window()
            .ok_or("no window")?
            .document()
            .ok_or("no document")?;

        let overlay: HtmlCanvasElement = document
            .create_element("canvas")?
            .dyn_into()?;
        overlay.set_id("crt");

        let w = source.width();
        let h = source.height();
        overlay.set_width(w);
        overlay.set_height(h);

        let style = overlay.style();
        style.set_property("position", "absolute")?;
        style.set_property("top", "0")?;
        style.set_property("left", "0")?;
        style.set_property("pointer-events", "none")?;
        style.set_property("z-index", "10")?;

        let body = document.body().ok_or("no body")?;
        body.append_child(&overlay)?;

        source.style().set_property("visibility", "hidden")?;

        let gl: GL = overlay
            .get_context("webgl")?
            .ok_or("webgl unavailable")?
            .dyn_into()?;

        let program = build_program(&gl, VERT_SRC, FRAG_SRC)?;

        let quad_buf = gl.create_buffer().ok_or("create_buffer failed")?;
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&quad_buf));
        let verts: [f32; 8] = [-1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0];
        unsafe {
            let view = js_sys::Float32Array::view(&verts);
            gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &view, GL::STATIC_DRAW);
        }

        let texture = gl.create_texture().ok_or("create_texture failed")?;
        gl.bind_texture(GL::TEXTURE_2D, Some(&texture));
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MIN_FILTER, GL::NEAREST as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MAG_FILTER, GL::NEAREST as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_S, GL::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_T, GL::CLAMP_TO_EDGE as i32);

        let u_texture = gl.get_uniform_location(&program, "u_texture")
            .ok_or("missing uniform u_texture")?;
        let u_time = gl.get_uniform_location(&program, "u_time")
            .ok_or("missing uniform u_time")?;
        let u_resolution_y = gl.get_uniform_location(&program, "u_resolution_y")
            .ok_or("missing uniform u_resolution_y")?;
        let u_glitch_on = gl.get_uniform_location(&program, "u_glitch_on")
            .ok_or("missing uniform u_glitch_on")?;
        let u_glitch_phase = gl.get_uniform_location(&program, "u_glitch_phase")
            .ok_or("missing uniform u_glitch_phase")?;
        // Array uniforms: locate via first element
        let u_exp_pos = gl.get_uniform_location(&program, "u_exp_pos[0]")
            .ok_or("missing uniform u_exp_pos")?;
        let u_exp_t = gl.get_uniform_location(&program, "u_exp_t[0]")
            .ok_or("missing uniform u_exp_t")?;
        let u_dist_pos = gl.get_uniform_location(&program, "u_dist_pos[0]")
            .ok_or("missing uniform u_dist_pos")?;

        gl.viewport(0, 0, w as i32, h as i32);

        Ok(PostProcessor {
            gl, program, quad_buf, texture,
            glitch: GlitchTimer::new(),
            frame: 0,
            u_texture, u_time,
            u_resolution_y,
            u_glitch_on, u_glitch_phase,
            u_exp_pos, u_exp_t, u_dist_pos,
            height: h as f32,
        })
    }

    /// Apply the CRT + explosion effect.
    ///
    /// `explosions` is a slice of `[u, v, t]` triples — UV-space centre and
    /// progress (0.0 = just started, 1.0 = finished) for each active explosion.
    /// At most 8 entries are used; extras are ignored.
    pub fn process(
        &mut self,
        source: &HtmlCanvasElement,
        rand_cooldown: u32,
        rand_burst: u32,
        rand_intensity: u32,
        explosions: &[[f32; 3]],
        dist: &[[f32; 2]],
    ) {
        self.glitch.tick(rand_cooldown, rand_burst, rand_intensity);

        let gl = &self.gl;

        gl.active_texture(GL::TEXTURE0);
        gl.bind_texture(GL::TEXTURE_2D, Some(&self.texture));
        gl.tex_image_2d_with_u32_and_u32_and_canvas(
            GL::TEXTURE_2D, 0, GL::RGBA as i32, GL::RGBA, GL::UNSIGNED_BYTE, source,
        ).expect("texImage2D failed");

        gl.use_program(Some(&self.program));
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&self.quad_buf));

        let pos_loc = gl.get_attrib_location(&self.program, "a_position") as u32;
        gl.enable_vertex_attrib_array(pos_loc);
        gl.vertex_attrib_pointer_with_i32(pos_loc, 2, GL::FLOAT, false, 0, 0);

        // Standard CRT uniforms
        gl.uniform1i(Some(&self.u_texture), 0);
        gl.uniform1f(Some(&self.u_time), self.frame as f32);
        gl.uniform1f(Some(&self.u_resolution_y), self.height);
        gl.uniform1i(Some(&self.u_glitch_on), i32::from(self.glitch.is_glitching()));
        gl.uniform1f(Some(&self.u_glitch_phase), self.glitch.effective_phase());

        // Explosion uniforms — pack into flat arrays; inactive slots use -1.0
        let mut pos_buf = [-1.0f32; 16]; // 8 × vec2
        let mut t_buf   = [-1.0f32;  8];
        for (i, exp) in explosions.iter().take(8).enumerate() {
            pos_buf[i * 2]     = exp[0]; // u
            pos_buf[i * 2 + 1] = exp[1]; // v
            t_buf[i]           = exp[2]; // t
        }
        gl.uniform2fv_with_f32_array(Some(&self.u_exp_pos), &pos_buf);
        gl.uniform1fv_with_f32_array(Some(&self.u_exp_t),   &t_buf);

        // Distortion uniforms — 4 slots; inactive slots use x=-1.0
        let mut dist_buf = [-1.0f32; 8]; // 4 × vec2
        for (i, d) in dist.iter().take(4).enumerate() {
            dist_buf[i * 2]     = d[0];
            dist_buf[i * 2 + 1] = d[1];
        }
        gl.uniform2fv_with_f32_array(Some(&self.u_dist_pos), &dist_buf);

        gl.draw_arrays(GL::TRIANGLE_STRIP, 0, 4);

        self.frame += 1;
    }
}

// ── Shader compilation helpers ────────────────────────────────────────────────

fn compile_shader(gl: &GL, kind: u32, src: &str) -> Result<WebGlShader, JsValue> {
    let shader = gl.create_shader(kind).ok_or("create_shader failed")?;
    gl.shader_source(&shader, src);
    gl.compile_shader(&shader);
    if gl.get_shader_parameter(&shader, GL::COMPILE_STATUS).as_bool().unwrap_or(false) {
        Ok(shader)
    } else {
        let log = gl.get_shader_info_log(&shader).unwrap_or_default();
        gl.delete_shader(Some(&shader));
        Err(JsValue::from_str(&format!("shader compile error: {log}")))
    }
}

fn build_program(gl: &GL, vert_src: &str, frag_src: &str) -> Result<WebGlProgram, JsValue> {
    let vert = compile_shader(gl, GL::VERTEX_SHADER, vert_src)?;
    let frag = compile_shader(gl, GL::FRAGMENT_SHADER, frag_src)?;

    let program = gl.create_program().ok_or("create_program failed")?;
    gl.attach_shader(&program, &vert);
    gl.attach_shader(&program, &frag);
    gl.link_program(&program);

    gl.delete_shader(Some(&vert));
    gl.delete_shader(Some(&frag));

    if gl.get_program_parameter(&program, GL::LINK_STATUS).as_bool().unwrap_or(false) {
        Ok(program)
    } else {
        let log = gl.get_program_info_log(&program).unwrap_or_default();
        gl.delete_program(Some(&program));
        Err(JsValue::from_str(&format!("program link error: {log}")))
    }
}
