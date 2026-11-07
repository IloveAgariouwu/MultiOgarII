struct View {
    view: vec4f,
    px: f32,
}

@group(0) @binding(0) var<uniform> u: View;

struct In {
    @builtin(vertex_index) vertex: u32,
    @location(0) mark: vec3f,
    @location(1) color: vec4f,
    @location(2) cover0: vec4f,
    @location(3) cover1: vec4f,
    @location(4) cover2: vec4f,
    @location(5) cover3: vec4f,
    @location(6) cover4: vec4f,
    @location(7) cover5: vec4f,
}

struct Out {
    @builtin(position) position: vec4f,
    @location(0) local: vec2f,
    @location(1) @interpolate(flat) mark: vec2f,
    @location(2) @interpolate(flat) color: vec4f,
    @location(3) @interpolate(flat) cover0: vec4f,
    @location(4) @interpolate(flat) cover1: vec4f,
    @location(5) @interpolate(flat) cover2: vec4f,
    @location(6) @interpolate(flat) cover3: vec4f,
    @location(7) @interpolate(flat) cover4: vec4f,
    @location(8) @interpolate(flat) cover5: vec4f,
}

@vertex
fn vs(i: In) -> Out {
    let corner = vec2f(f32(i.vertex & 1u), f32(i.vertex >> 1u)) * 2.0 - 1.0;
    let r = i.mark.z;
    let local = corner * r;
    var o: Out;
    o.position = vec4f((i.mark.xy + local - u.view.xy) * u.view.zw * vec2f(1.0, -1.0), 0.0, 1.0);
    o.local = local;
    o.mark = vec2f(r, 1.0 / max(r * u.px, 1e-6));
    o.color = i.color;
    o.cover0 = i.cover0;
    o.cover1 = i.cover1;
    o.cover2 = i.cover2;
    o.cover3 = i.cover3;
    o.cover4 = i.cover4;
    o.cover5 = i.cover5;
    return o;
}

fn covered(p: vec2f, c: vec2f, r: f32) -> bool {
    let q = p - c;
    return dot(q, q) < r * r;
}

@fragment
fn fs(i: Out) -> @location(0) vec4f {
    let p = i.local;
    let d = length(p) / i.mark.x;
    let e = i.mark.y;
    if (d > 1.0) {
        discard;
    }
    if (i.cover0.z >= 0.0) {
        let inside = covered(p, i.cover0.xy, i.cover0.z) || covered(p, vec2f(i.cover0.w, i.cover1.x), i.cover1.y)
            || covered(p, i.cover1.zw, i.cover2.x) || covered(p, i.cover2.yz, i.cover2.w)
            || covered(p, i.cover3.xy, i.cover3.z) || covered(p, vec2f(i.cover3.w, i.cover4.x), i.cover4.y)
            || covered(p, i.cover4.zw, i.cover5.x) || covered(p, i.cover5.yz, i.cover5.w);
        if (!inside) {
            discard;
        }
    }
    let inward = 1.0 - d;
    let rim = smoothstep(OVERLAP_RIM - e, OVERLAP_RIM + e, d);
    let a = max(0.5 * exp(-(inward * inward) / OVERLAP_GLOW_SPREAD), rim);
    let cover = i.color.a * a * (1.0 - smoothstep(1.0 - e, 1.0, d));
    return vec4f(i.color.rgb * cover, cover);
}
