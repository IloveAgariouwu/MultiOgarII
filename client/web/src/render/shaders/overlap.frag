#version 300 es
precision highp float;
in vec2 v_local;
flat in vec2 v_mark;
flat in vec4 v_color;
flat in vec4 v_cover0;
flat in vec4 v_cover1;
flat in vec4 v_cover2;
flat in vec4 v_cover3;
flat in vec4 v_cover4;
flat in vec4 v_cover5;
out vec4 o_color;
bool covered(vec2 p, vec2 c, float r) {
    vec2 q = p - c;
    return dot(q, q) < r * r;
}
void main() {
    vec2 p = v_local;
    float d = length(p) / v_mark.x;
    float e = v_mark.y;
    if (d > 1.0) discard;
    if (v_cover0.z >= 0.0) {
        bool inside = covered(p, v_cover0.xy, v_cover0.z) || covered(p, vec2(v_cover0.w, v_cover1.x), v_cover1.y)
            || covered(p, v_cover1.zw, v_cover2.x) || covered(p, v_cover2.yz, v_cover2.w)
            || covered(p, v_cover3.xy, v_cover3.z) || covered(p, vec2(v_cover3.w, v_cover4.x), v_cover4.y)
            || covered(p, v_cover4.zw, v_cover5.x) || covered(p, v_cover5.yz, v_cover5.w);
        if (!inside) discard;
    }
    float inward = 1.0 - d;
    float rim = smoothstep(OVERLAP_RIM - e, OVERLAP_RIM + e, d);
    float a = max(0.5 * exp(-(inward * inward) / OVERLAP_GLOW_SPREAD), rim);
    float cover = v_color.a * a * (1.0 - smoothstep(1.0 - e, 1.0, d));
    o_color = vec4(v_color.rgb * cover, cover);
}
