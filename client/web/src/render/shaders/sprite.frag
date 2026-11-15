#version 300 es
precision highp float;
precision highp sampler2DArray;
uniform sampler2DArray u_atlas;
uniform sampler2DArray u_skins;
uniform sampler2D u_backdrop;
uniform highp sampler2D u_jelly;
in vec2 v_local;
in vec2 v_uv;
in vec4 v_color0;
in vec4 v_color1;
in float v_scalar;
in float v_aa;
flat in uint v_kind;
flat in uint v_flags;
flat in uint v_layer;
flat in vec4 v_aux;
flat in vec2 v_size;
flat in float v_jelly;
flat in float v_glow;
out vec4 o_color;
float starDistance(vec2 p) {
    float a = mod(atan(p.y, p.x), STAR_SEGMENT);
    a = min(a, STAR_SEGMENT - a);
    vec2 q = length(p) * vec2(cos(a), sin(a));
    vec2 outer = vec2(STAR_OUTER, 0.0);
    vec2 inner = STAR_INNER * vec2(cos(STAR_SEGMENT * 0.5), sin(STAR_SEGMENT * 0.5));
    vec2 e = inner - outer;
    return dot(q - outer, normalize(vec2(e.y, -e.x)));
}
float erfc(float x) {
    float t = 1.0 / (1.0 + 0.3275911 * x);
    float poly = t * (0.254829592 + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
    return poly * exp(-x * x);
}
float frameAlpha(vec2 offset, vec2 halfSize, float band, float px, bool glow) {
    vec2 q = abs(offset);
    float d = min(halfSize.x - q.x, halfSize.y - q.y);
    float solid = clamp((d + band) / px + 0.5, 0.0, 1.0) * clamp(-d / px + 0.5, 0.0, 1.0);
    float sigma = FRAME_GLOW_SIGMA * band;
    float halo = glow ? 0.5 * erfc(max(d, 0.0) / (sigma * 1.41421356)) * clamp(d / px + 0.5, 0.0, 1.0) : 0.0;
    return max(solid, halo);
}
float logistic(float x, float sigma) {
    return 1.0 / (1.0 + exp(-1.702 * x / sigma));
}
float rainbowFrameAlpha(vec2 offset, vec2 halfSize, float band, float px, vec2 glow) {
    vec2 q = abs(offset);
    float v = min(halfSize.x - q.x, halfSize.y - q.y) + band;
    float sigma = max(glow.x, 1e-6);
    float core = 1.0 - smoothstep(band - px, band + px, v);
    float halo = logistic(v, sigma) - logistic(v - band, sigma);
    return max(core, halo) * clamp(v / px + 0.5, 0.0, 1.0) * step(v, band + glow.y);
}
vec2 jellyWarp(vec2 p, float row) {
    if (row < 0.0) return p;
    float rho = length(p);
    if (rho <= 0.0) return p;
    float u = fract(atan(p.y, p.x) / TAU) * float(JELLY_SAMPLES);
    int i0 = int(floor(u)) % JELLY_SAMPLES;
    int i1 = (i0 + 1) % JELLY_SAMPLES;
    float e = mix(texelFetch(u_jelly, ivec2(i0, int(row)), 0).r, texelFetch(u_jelly, ivec2(i1, int(row)), 0).r, fract(u));
    float cd = JELLY_CORE * min(1.0, e);
    float s = rho <= cd ? rho * JELLY_CORE / cd : rho <= e ? JELLY_CORE + (rho - cd) * (1.0 - JELLY_CORE) / (e - cd) : rho - e + 1.0;
    return p * (s / rho);
}
float boxCover(float p, float extent, float aa) {
    float lo = max(p - 0.5 * aa, -extent);
    float hi = min(p + 0.5 * aa, extent);
    return clamp((hi - lo) / aa, 0.0, 1.0);
}
vec4 over(vec4 top, vec4 below) {
    return top + below * (1.0 - top.a);
}
vec4 decor() {
    if ((v_flags & CARD_TINT) != 0u) return v_color1;
    if ((v_flags & CARD_ENEMY) == 0u) return vec4(0.0);
    vec2 p = v_local;
    float band = ENEMY_LINE * 0.5;
    float ring = clamp((band - abs(length(p) - 1.0 + ENEMY_RING_INSET)) / v_aa + 0.5, 0.0, 1.0);
    float across = abs(p.x + p.y) * 0.70710678;
    float along = abs(p.x - p.y) * 0.70710678;
    float slash = clamp((band - across) / v_aa + 0.5, 0.0, 1.0) * clamp((SLASH_HALF - along) / v_aa + 0.5, 0.0, 1.0);
    vec4 team = vec4(v_color1.rgb, 1.0);
    vec4 dim = vec4(0.0, 0.0, 0.0, ENEMY_DIM);
    return over(team * (v_color1.a * slash), over(team * (v_color1.a * ring), dim));
}
vec4 card() {
    vec3 base = v_color0.rgb;
    if ((v_flags & CARD_SKINNED) != 0u) {
        vec4 s = texture(u_skins, vec3(v_local * 0.5 + 0.5, float(v_layer)));
        float k = v_aux.x * clamp((1.0 - length(v_local)) / v_aa + 0.5, 0.0, 1.0);
        base = base * (1.0 - s.a * k) + s.rgb * k;
    }
    float alpha = v_aux.y;
    vec4 d = decor();
    float under = alpha * (1.0 - d.a);
    return vec4(alpha * d.rgb + under * base, alpha * d.a + under);
}
void main() {
    if (v_kind == KIND_IMAGE) {
        o_color = texture(u_backdrop, v_uv) * v_color0;
        return;
    }
    if (v_kind == KIND_ARROW) {
        float span = ARROW_TIP - ARROW_BASE;
        float side = ARROW_HALF_WIDTH * (ARROW_TIP - v_local.y) / span - abs(v_local.x);
        float inside = min(min(v_local.y - ARROW_BASE, ARROW_TIP - v_local.y), side);
        o_color = v_color0 * clamp(inside / v_aa + 0.5, 0.0, 1.0);
        return;
    }
    if (v_kind == KIND_FRAME && v_flags == FRAME_RAINBOW) {
        vec2 outer = v_size + v_scalar;
        float t = clamp(0.5 + (v_local.x / outer.x - v_local.y / outer.y) * 0.25, 0.0, 1.0);
        o_color = mix(v_color0, v_color1, t) * rainbowFrameAlpha(v_local, v_size, v_scalar, v_aa, v_aux.xy * min(v_size.x, v_size.y));
        return;
    }
    if (v_kind == KIND_FRAME) {
        o_color = v_color0 * frameAlpha(v_local, v_size, v_scalar, v_aa, v_flags == FRAME_GLOW);
        return;
    }
    if (v_kind == KIND_SHADOW) {
        float d = length(jellyWarp(v_local, v_jelly));
        float a = clamp((d - 1.0 + SHADOW_FEATHER) / SHADOW_FEATHER, 0.0, 1.0) * clamp((SHADOW_REACH - d) / (SHADOW_REACH - 1.0), 0.0, 1.0);
        if (a <= 0.0) discard;
        o_color = v_color0 * a;
        return;
    }
    if (v_kind == KIND_GRADIENT) {
        vec2 local = jellyWarp(v_local, v_jelly);
        float d = length(local);
        float cover = clamp((1.0 - d) / v_aa + 0.5, 0.0, 1.0) * clamp((d - 1.0 + v_scalar) / v_aa + 0.5, 0.0, 1.0);
        if (cover <= 0.0) discard;
        float t = clamp(0.5 + (local.x - local.y) * v_aux.x, 0.0, 1.0);
        o_color = mix(v_color0, v_color1, t) * cover;
        return;
    }
    if (v_kind == KIND_GLYPH) {
        o_color = texture(u_atlas, vec3(v_uv, float(v_layer))) * v_color0;
        return;
    }
    if (v_kind == KIND_SDF) {
        float dist = (texture(u_atlas, vec3(v_uv, float(v_layer))).a - 0.5) * 2.0 * SDF_SPREAD;
        o_color = v_color0 * clamp(dist / v_aa + 0.5, 0.0, 1.0);
        return;
    }
    if (v_kind == KIND_RECT || v_kind == KIND_SEGMENT) {
        o_color = v_color0 * (boxCover(v_local.x, v_size.x, v_aa) * boxCover(v_local.y, v_size.y, v_aa));
        return;
    }
    if (v_kind == KIND_STAR) {
        float aa = v_aa * STAR_EXTENT;
        float sd = starDistance(v_local * STAR_EXTENT);
        vec4 fill = v_color0 * clamp(-sd / aa + 0.5, 0.0, 1.0);
        vec4 stroke = v_color1 * clamp((STAR_HALF_LINE - abs(sd)) / aa + 0.5, 0.0, 1.0);
        vec4 c = over(stroke, fill);
        if (c.a <= 0.0) discard;
        o_color = c;
        return;
    }
    vec2 local = v_kind == KIND_HALO ? v_local : jellyWarp(v_local, v_jelly);
    float d = length(local);
    float edge = clamp((1.0 - d) / v_aa + 0.5, 0.0, 1.0);
    float glow = v_glow > 0.0 ? v_aux.y * 0.5 * erfc(max(d - 1.0, 0.0) / (v_glow * 1.41421356)) * inversesqrt(max(d, 1.0)) : 0.0;
    if (edge <= 0.0 && glow < GLOW_CUTOFF) discard;
    if (v_kind == KIND_HALO) {
        o_color = v_color0 * (edge * clamp((d - HALO_INNER) / (1.0 - HALO_INNER), 0.0, 1.0));
        return;
    }
    if (v_kind == KIND_CARD) {
        o_color = card() * edge + vec4(v_color0.rgb, 1.0) * (glow * (1.0 - edge));
        return;
    }
    vec4 c = v_color0;
    if (v_scalar > 0.0) {
        c = mix(c, v_color1, clamp((d - 1.0 + v_scalar) / v_aa + 0.5, 0.0, 1.0));
    }
    o_color = c * edge;
}
