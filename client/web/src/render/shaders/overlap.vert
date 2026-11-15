#version 300 es
precision highp float;
layout(location = 0) in vec3 a_mark;
layout(location = 1) in vec4 a_color;
layout(location = 2) in vec4 a_cover0;
layout(location = 3) in vec4 a_cover1;
layout(location = 4) in vec4 a_cover2;
layout(location = 5) in vec4 a_cover3;
layout(location = 6) in vec4 a_cover4;
layout(location = 7) in vec4 a_cover5;
uniform vec4 u_view;
uniform float u_px;
out vec2 v_local;
flat out vec2 v_mark;
flat out vec4 v_color;
flat out vec4 v_cover0;
flat out vec4 v_cover1;
flat out vec4 v_cover2;
flat out vec4 v_cover3;
flat out vec4 v_cover4;
flat out vec4 v_cover5;
void main() {
    vec2 corner = vec2(float(gl_VertexID & 1), float(gl_VertexID >> 1)) * 2.0 - 1.0;
    float r = a_mark.z;
    vec2 local = corner * r;
    gl_Position = vec4((a_mark.xy + local - u_view.xy) * u_view.zw * vec2(1.0, -1.0), 0.0, 1.0);
    v_local = local;
    v_mark = vec2(r, 1.0 / max(r * u_px, 1e-6));
    v_color = a_color;
    v_cover0 = a_cover0;
    v_cover1 = a_cover1;
    v_cover2 = a_cover2;
    v_cover3 = a_cover3;
    v_cover4 = a_cover4;
    v_cover5 = a_cover5;
}
