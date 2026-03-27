precision mediump float;

varying vec2 v_uv;

uniform sampler2D u_texture;
uniform float     u_time;
uniform float     u_resolution_y;
uniform int       u_glitch_on;
uniform float     u_glitch_phase;

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

void main() {
    // 1. Barrel distortion
    vec2 uv = barrel(v_uv);

    // Black border outside the distorted area
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        gl_FragColor = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }

    // 2. Sample: chromatic aberration + horizontal tear during glitch
    vec4 color;
    if (u_glitch_on == 1) {
        float band    = floor(uv.y * u_resolution_y / 4.0);
        float tear_x  = sin(band * 17.31 + u_time * 0.3) * 0.008 * u_glitch_phase;
        vec2  torn_uv = vec2(uv.x + tear_x, uv.y);
        color         = chroma_sample(torn_uv, 0.006 * u_glitch_phase);
    } else {
        color = texture2D(u_texture, uv);
    }

    // 3. Scanlines
    color.rgb *= scanline(uv);

    // 4. Vignette
    color.rgb *= clamp(vignette(uv), 0.0, 1.0);

    // 5. Subtle green phosphor tint
    color.rgb *= vec3(0.92, 1.0, 0.88);

    gl_FragColor = color;
}
