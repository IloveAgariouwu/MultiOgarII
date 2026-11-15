#version 300 es
precision highp float;
layout(location = 0) in vec4 a_geom;
layout(location = 1) in vec4 a_aux;
layout(location = 2) in vec4 a_color0;
layout(location = 3) in vec4 a_color1;
layout(location = 4) in float a_scalar;
layout(location = 5) in uint a_head;
uniform vec4 u_view;
uniform float u_px;
uniform highp sampler2D u_jelly;
out vec2 v_local;
out vec2 v_uv;
out vec4 v_color0;
out vec4 v_color1;
out float v_scalar;
out float v_aa;
flat out uint v_kind;
flat out uint v_flags;
flat out uint v_layer;
flat out vec4 v_aux;
flat out vec2 v_size;
flat out float v_jelly;
flat out float v_glow;
void main() {
    vec2 corner = vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1)) * 2.0 - 1.0;
    uint kind = a_head & KIND_MASK;
    float radiusPx = max(a_geom.z * u_px, MIN_RADIUS_PX);
    float glow = float((a_head >> GLOW_SHIFT) & GLOW_MASK) / GLOW_UNITS;
    bool disc = kind == KIND_DISC || kind == KIND_CARD || kind == KIND_HALO || kind == KIND_GRADIENT || kind == KIND_SHADOW;
    float spread = kind == KIND_SHADOW ? SHADOW_REACH - 1.0 : GLOW_REACH * glow;
    uint jellyIndex = uint(round(a_aux.w * 65535.0));
    bool jelly = disc && jellyIndex != NO_JELLY;
    float jellyRow = jelly ? float(jellyIndex) : -1.0;
    float reach = jelly ? texelFetch(u_jelly, ivec2(JELLY_SAMPLES, int(jellyIndex)), 0).r : 1.0;
    vec2 local = disc ? corner * (reach + spread + AA_MARGIN_PX / radiusPx) : corner;
    vec2 world = a_geom.xy + local * a_geom.zw;
    if (kind == KIND_FRAME) {
        vec2 extent = a_geom.zw + a_scalar;
        world = a_geom.xy + corner * extent;
        local = corner * extent;
    }
    vec2 size = a_geom.zw;
    if (kind == KIND_RECT) {
        local = corner * (size + AA_MARGIN_PX / u_px);
        world = a_geom.xy + local;
    }
    if (kind == KIND_SEGMENT) {
        vec2 dir = a_geom.zw - a_geom.xy;
        float len = max(length(dir), 1e-6);
        vec2 along = dir / len;
        vec2 normal = vec2(-along.y, along.x);
        size = vec2(len * 0.5, a_scalar);
        local = corner * (size + AA_MARGIN_PX / u_px);
        world = (a_geom.xy + a_geom.zw) * 0.5 + along * local.x + normal * local.y;
    }
    gl_Position = vec4((world - u_view.xy) * u_view.zw * vec2(1.0, -1.0), 0.0, 1.0);
    v_local = local;
    v_uv = mix(a_aux.xy, a_aux.zw, corner * 0.5 + 0.5);
    v_color0 = a_color0;
    v_color1 = a_color1;
    v_scalar = a_scalar;
    v_aa = kind == KIND_SDF ? 1.0 / (a_scalar * u_px) : kind == KIND_FRAME || kind == KIND_RECT || kind == KIND_SEGMENT ? 1.0 / u_px : 1.0 / radiusPx;
    v_kind = kind;
    v_flags = (a_head >> FLAG_SHIFT) & FLAG_MASK;
    v_layer = a_head >> LAYER_SHIFT;
    v_aux = a_aux;
    v_size = size;
    v_jelly = jellyRow;
    v_glow = glow;
}
