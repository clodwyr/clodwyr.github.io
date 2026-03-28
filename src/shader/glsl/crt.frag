precision mediump float;

varying vec2 v_uv;

uniform sampler2D u_texture;
uniform float     u_time;
uniform float     u_resolution_y;
uniform int       u_glitch_on;
uniform float     u_glitch_phase;

// ── Explosion sparks ──────────────────────────────────────────────────────────
// Up to 8 simultaneous explosions passed from Rust each frame.
// u_exp_t[i] == -1.0 means slot i is inactive.
uniform vec2  u_exp_pos[8];
uniform float u_exp_t[8];

// ── Per-alien local distortion ────────────────────────────────────────────────
// Up to 4 slots: 0-1 random alive aliens, 2-3 exploding aliens.
// u_dist_pos[i].x < 0.0 means slot i is inactive.
uniform vec2 u_dist_pos[4];

// Cheap hash -- returns 0..1 from a vec2 seed.
float hash(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

// Rotate a 2D direction by precomputed cos/sin.
vec2 rot(vec2 d, float ca, float sa) {
    return vec2(d.x * ca - d.y * sa, d.x * sa + d.y * ca);
}

// Procedural spark burst in UV space.
// pos: UV-space centre. t: 0.0 (just started) -> 1.0 (finished).
// The burst is randomly rotated based on pos so simultaneous hits
// at different positions show distinct patterns.
vec3 spark_at(vec2 uv, vec2 pos, float t) {
    float fade = max(0.0, 1.0 - t * 1.1);

    // Central flash: flat alien green
    vec3 col = vec3(0.0);
    if (length(uv - pos) < 0.014 * max(0.0, 1.0 - t * 1.6)) {
        col += vec3(0.408, 0.984, 0.208) * fade * 2.0;
    }

    // Random rotation seeded by position
    float angle = hash(pos) * 6.2832;
    float ca = cos(angle); float sa = sin(angle);

    float rL    = t * 0.08;
    float rS    = t * 0.055;
    float hsize = 0.003;
    vec3  sc    = vec3(0.408, 0.984, 0.208) * fade;
    vec2 sp; float du; float dv;

    // 8 long spokes (cardinal + diagonal), rotated
    sp = pos + rot(vec2( 1.000,  0.000), ca, sa) * rL; du=abs(uv.x-sp.x); dv=abs(uv.y-sp.y); if(du<hsize&&dv<hsize) col+=sc;
    sp = pos + rot(vec2(-1.000,  0.000), ca, sa) * rL; du=abs(uv.x-sp.x); dv=abs(uv.y-sp.y); if(du<hsize&&dv<hsize) col+=sc;
    sp = pos + rot(vec2( 0.000,  1.000), ca, sa) * rL; du=abs(uv.x-sp.x); dv=abs(uv.y-sp.y); if(du<hsize&&dv<hsize) col+=sc;
    sp = pos + rot(vec2( 0.000, -1.000), ca, sa) * rL; du=abs(uv.x-sp.x); dv=abs(uv.y-sp.y); if(du<hsize&&dv<hsize) col+=sc;
    sp = pos + rot(vec2( 0.707,  0.707), ca, sa) * rL; du=abs(uv.x-sp.x); dv=abs(uv.y-sp.y); if(du<hsize&&dv<hsize) col+=sc;
    sp = pos + rot(vec2(-0.707,  0.707), ca, sa) * rL; du=abs(uv.x-sp.x); dv=abs(uv.y-sp.y); if(du<hsize&&dv<hsize) col+=sc;
    sp = pos + rot(vec2(-0.707, -0.707), ca, sa) * rL; du=abs(uv.x-sp.x); dv=abs(uv.y-sp.y); if(du<hsize&&dv<hsize) col+=sc;
    sp = pos + rot(vec2( 0.707, -0.707), ca, sa) * rL; du=abs(uv.x-sp.x); dv=abs(uv.y-sp.y); if(du<hsize&&dv<hsize) col+=sc;
    // 8 short spokes (22.5-degree offset), rotated
    sp = pos + rot(vec2( 0.924,  0.383), ca, sa) * rS; du=abs(uv.x-sp.x); dv=abs(uv.y-sp.y); if(du<hsize&&dv<hsize) col+=sc;
    sp = pos + rot(vec2( 0.383,  0.924), ca, sa) * rS; du=abs(uv.x-sp.x); dv=abs(uv.y-sp.y); if(du<hsize&&dv<hsize) col+=sc;
    sp = pos + rot(vec2(-0.383,  0.924), ca, sa) * rS; du=abs(uv.x-sp.x); dv=abs(uv.y-sp.y); if(du<hsize&&dv<hsize) col+=sc;
    sp = pos + rot(vec2(-0.924,  0.383), ca, sa) * rS; du=abs(uv.x-sp.x); dv=abs(uv.y-sp.y); if(du<hsize&&dv<hsize) col+=sc;
    sp = pos + rot(vec2(-0.924, -0.383), ca, sa) * rS; du=abs(uv.x-sp.x); dv=abs(uv.y-sp.y); if(du<hsize&&dv<hsize) col+=sc;
    sp = pos + rot(vec2(-0.383, -0.924), ca, sa) * rS; du=abs(uv.x-sp.x); dv=abs(uv.y-sp.y); if(du<hsize&&dv<hsize) col+=sc;
    sp = pos + rot(vec2( 0.383, -0.924), ca, sa) * rS; du=abs(uv.x-sp.x); dv=abs(uv.y-sp.y); if(du<hsize&&dv<hsize) col+=sc;
    sp = pos + rot(vec2( 0.924, -0.383), ca, sa) * rS; du=abs(uv.x-sp.x); dv=abs(uv.y-sp.y); if(du<hsize&&dv<hsize) col+=sc;

    return col;
}

// GLSL ES 1.00: uniform arrays may only be indexed by compile-time constants.
vec3 explosion_layer(vec2 uv) {
    vec3 result = vec3(0.0);
    if (u_exp_t[0] >= 0.0) result += spark_at(uv, u_exp_pos[0], u_exp_t[0]);
    if (u_exp_t[1] >= 0.0) result += spark_at(uv, u_exp_pos[1], u_exp_t[1]);
    if (u_exp_t[2] >= 0.0) result += spark_at(uv, u_exp_pos[2], u_exp_t[2]);
    if (u_exp_t[3] >= 0.0) result += spark_at(uv, u_exp_pos[3], u_exp_t[3]);
    if (u_exp_t[4] >= 0.0) result += spark_at(uv, u_exp_pos[4], u_exp_t[4]);
    if (u_exp_t[5] >= 0.0) result += spark_at(uv, u_exp_pos[5], u_exp_t[5]);
    if (u_exp_t[6] >= 0.0) result += spark_at(uv, u_exp_pos[6], u_exp_t[6]);
    if (u_exp_t[7] >= 0.0) result += spark_at(uv, u_exp_pos[7], u_exp_t[7]);
    return result;
}

// ── Local alien distortion ────────────────────────────────────────────────────

// Horizontal band-tear within a radius of an alien position.
// Works in barrel-distorted UV space; positional error from barrel is negligible.
vec2 local_glitch_at(vec2 uv, vec2 pos) {
    float d = length(uv - pos);
    if (d < 0.05) {
        float falloff  = 1.0 - d / 0.05;
        float band     = floor(uv.y * u_resolution_y / 2.0);
        // Per-band random amplitude: each band gets an independent weight 0..1,
        // re-seeded every ~12 frames so it reshuffles without constant flicker.
        float band_amp = hash(vec2(band, floor(u_time * 0.08)));
        uv.x += sin(band * 29.3 + u_time * 0.9) * 0.010 * falloff * band_amp;
    }
    return uv;
}

// ── CRT effects ───────────────────────────────────────────────────────────────

// Barrel distortion (mild CRT curve)
vec2 barrel(vec2 uv) {
    vec2  cc   = uv - 0.5;
    float dist = dot(cc, cc);
    return uv + cc * dist * 0.10;
}

// Scanlines
float scanline(vec2 uv) {
    // pi/2 gives ~4-px stripes at full canvas height; wide enough to survive
    // HiDPI upscaling while still looking like a CRT grill.
    float line = sin(uv.y * u_resolution_y * 1.5708);
    return 0.5 + 0.5 * line * line;
}

// Vignette
float vignette(vec2 uv) {
    vec2 d = uv - 0.5;
    return 1.0 - dot(d, d) * 2.2;
}

// Chromatic aberration sample
vec4 chroma_sample(vec2 uv, float strength) {
    vec2  off = vec2(strength, 0.0);
    float r   = texture2D(u_texture, uv + off).r;
    float g   = texture2D(u_texture, uv      ).g;
    float b   = texture2D(u_texture, uv - off).b;
    float a   = texture2D(u_texture, uv      ).a;
    return vec4(r, g, b, a);
}

// ── Toggle: comment out one main() and uncomment the other ───────────────────

void main() {
    // ── Full CRT pipeline ─────────────────────────────────────────────────────

    // 1. Barrel distortion
    vec2 uv = barrel(v_uv);

    // Black border outside the distorted area
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        gl_FragColor = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }

    // 2. Per-alien local distortion (applied before texture sample)
    if (u_dist_pos[0].x >= 0.0) uv = local_glitch_at(uv, u_dist_pos[0]);
    if (u_dist_pos[1].x >= 0.0) uv = local_glitch_at(uv, u_dist_pos[1]);
    if (u_dist_pos[2].x >= 0.0) uv = local_glitch_at(uv, u_dist_pos[2]);
    if (u_dist_pos[3].x >= 0.0) uv = local_glitch_at(uv, u_dist_pos[3]);

    // 3. Sample with chromatic aberration / glitch tear
    vec4 color;
    if (u_glitch_on == 1) {
        float band    = floor(uv.y * u_resolution_y / 4.0);
        float tear_x  = sin(band * 17.31 + u_time * 0.3) * 0.008 * u_glitch_phase;
        vec2  torn_uv = vec2(uv.x + tear_x, uv.y);
        color         = chroma_sample(torn_uv, 0.006 * u_glitch_phase);
    } else {
        color = texture2D(u_texture, uv);
    }

    // 4. Explosion sparks (additive, before CRT so effects layer over them)
    color.rgb += explosion_layer(v_uv);
    color.rgb  = clamp(color.rgb, 0.0, 1.0);

    // 5. Scanlines
    color.rgb *= scanline(uv);

    // 6. Vignette
    color.rgb *= clamp(vignette(uv), 0.0, 1.0);

    // 7. Subtle green phosphor tint
    color.rgb *= vec3(0.92, 1.0, 0.88);

    // Force opaque: canvas background is alpha=0 (cleared with clearRect).
    gl_FragColor = vec4(color.rgb, 1.0);
}

/* -- CRT bypass (swap comments with the main() above to disable CRT) --

void main() {
    vec4 color = texture2D(u_texture, v_uv);
    color.rgb += explosion_layer(v_uv);
    color.rgb  = clamp(color.rgb, 0.0, 1.0);
    gl_FragColor = vec4(color.rgb, 1.0);
}

*/
