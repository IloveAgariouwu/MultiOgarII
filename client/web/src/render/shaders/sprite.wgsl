struct View {
    view: vec4f,
    px: f32,
}

@group(0) @binding(0) var<uniform> u: View;
@group(0) @binding(1) var atlasTex: texture_2d_array<f32>;
@group(0) @binding(2) var atlasSampler: sampler;
@group(0) @binding(3) var skinTex: texture_2d_array<f32>;
@group(0) @binding(4) var skinSampler: sampler;
@group(0) @binding(5) var backdropTex: texture_2d<f32>;
@group(0) @binding(6) var jellyTex: texture_2d<f32>;

struct In {
    @builtin(vertex_index) vertex: u32,
    @location(0) geom: vec4f,
    @location(1) aux: vec4f,
    @location(2) color0: vec4f,
    @location(3) color1: vec4f,
    @location(4) scalar: f32,
    @location(5) head: u32,
}

struct Out {
    @builtin(position) position: vec4f,
    @location(0) local: vec2f,
    @location(1) uv: vec2f,
    @location(2) color0: vec4f,
    @location(3) color1: vec4f,
    @location(4) scalar: f32,
    @location(5) aa: f32,
    @location(6) @interpolate(flat) kind: u32,
    @location(7) @interpolate(flat) flags: u32,
    @location(8) @interpolate(flat) layer: u32,
    @location(9) @interpolate(flat) aux: vec4f,
    @location(10) @interpolate(flat) size: vec2f,
    @location(11) @interpolate(flat) jelly: f32,
    @location(12) @interpolate(flat) glow: f32,
}

fn starDistance(p: vec2f) -> f32 {
    var a = atan2(p.y, p.x);
    a = a - floor(a / STAR_SEGMENT) * STAR_SEGMENT;
    a = min(a, STAR_SEGMENT - a);
    let q = length(p) * vec2f(cos(a), sin(a));
    let outer = vec2f(STAR_OUTER, 0.0);
    let inner = STAR_INNER * vec2f(cos(STAR_SEGMENT * 0.5), sin(STAR_SEGMENT * 0.5));
    let e = inner - outer;
    return dot(q - outer, normalize(vec2f(e.y, -e.x)));
}

fn isDisc(kind: u32) -> bool {
    return kind == KIND_DISC || kind == KIND_CARD || kind == KIND_HALO || kind == KIND_GRADIENT || kind == KIND_SHADOW;
}

@vertex
fn vs(i: In) -> Out {
    let corner = vec2f(f32(i.vertex & 1u), f32(i.vertex >> 1u)) * 2.0 - 1.0;
    let kind = i.head & KIND_MASK;
    let radiusPx = max(i.geom.z * u.px, MIN_RADIUS_PX);
    let disc = isDisc(kind);
    let glow = f32((i.head >> GLOW_SHIFT) & GLOW_MASK) / GLOW_UNITS;
    let jellyIndex = u32(round(i.aux.w * 65535.0));
    var jellyRow = -1.0;
    var reach = 1.0;
    if (disc && jellyIndex != NO_JELLY) {
        jellyRow = f32(jellyIndex);
        reach = textureLoad(jellyTex, vec2i(JELLY_SAMPLES, i32(jellyIndex)), 0).r;
    }
    var local = corner;
    if (disc) {
        let spread = select(GLOW_REACH * glow, SHADOW_REACH - 1.0, kind == KIND_SHADOW);
        local = corner * (reach + spread + AA_MARGIN_PX / radiusPx);
    }
    var world = i.geom.xy + local * i.geom.zw;
    if (kind == KIND_FRAME) {
        let extent = i.geom.zw + i.scalar;
        world = i.geom.xy + corner * extent;
        local = corner * extent;
    }
    var size = i.geom.zw;
    if (kind == KIND_RECT) {
        local = corner * (size + AA_MARGIN_PX / u.px);
        world = i.geom.xy + local;
    }
    if (kind == KIND_SEGMENT) {
        let dir = i.geom.zw - i.geom.xy;
        let len = max(length(dir), 1e-6);
        let along = dir / len;
        let normal = vec2f(-along.y, along.x);
        size = vec2f(len * 0.5, i.scalar);
        local = corner * (size + AA_MARGIN_PX / u.px);
        world = (i.geom.xy + i.geom.zw) * 0.5 + along * local.x + normal * local.y;
    }
    var o: Out;
    o.position = vec4f((world - u.view.xy) * u.view.zw * vec2f(1.0, -1.0), 0.0, 1.0);
    o.local = local;
    o.uv = mix(i.aux.xy, i.aux.zw, corner * 0.5 + 0.5);
    o.color0 = i.color0;
    o.color1 = i.color1;
    o.scalar = i.scalar;
    o.aa = select(1.0 / radiusPx, 1.0 / u.px, kind == KIND_FRAME || kind == KIND_RECT || kind == KIND_SEGMENT);
    if (kind == KIND_SDF) {
        o.aa = 1.0 / (i.scalar * u.px);
    }
    o.kind = kind;
    o.flags = (i.head >> FLAG_SHIFT) & FLAG_MASK;
    o.layer = i.head >> LAYER_SHIFT;
    o.aux = i.aux;
    o.size = size;
    o.jelly = jellyRow;
    o.glow = glow;
    return o;
}

fn erfcApprox(x: f32) -> f32 {
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let poly = t * (0.254829592 + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
    return poly * exp(-x * x);
}

fn frameAlpha(offset: vec2f, halfSize: vec2f, band: f32, px: f32, glow: bool) -> f32 {
    let q = abs(offset);
    let d = min(halfSize.x - q.x, halfSize.y - q.y);
    let solid = clamp((d + band) / px + 0.5, 0.0, 1.0) * clamp(-d / px + 0.5, 0.0, 1.0);
    let sigma = FRAME_GLOW_SIGMA * band;
    var halo = 0.0;
    if (glow) {
        halo = 0.5 * erfcApprox(max(d, 0.0) / (sigma * 1.41421356)) * clamp(d / px + 0.5, 0.0, 1.0);
    }
    return max(solid, halo);
}

fn logistic(x: f32, sigma: f32) -> f32 {
    return 1.0 / (1.0 + exp(-1.702 * x / sigma));
}

fn rainbowFrameAlpha(offset: vec2f, halfSize: vec2f, band: f32, px: f32, glow: vec2f) -> f32 {
    let q = abs(offset);
    let v = min(halfSize.x - q.x, halfSize.y - q.y) + band;
    let sigma = max(glow.x, 1e-6);
    let core = 1.0 - smoothstep(band - px, band + px, v);
    let halo = logistic(v, sigma) - logistic(v - band, sigma);
    return max(core, halo) * clamp(v / px + 0.5, 0.0, 1.0) * step(v, band + glow.y);
}

fn jellyWarp(p: vec2f, row: f32) -> vec2f {
    let rho = length(p);
    if (row < 0.0 || rho <= 0.0) {
        return p;
    }
    let u = fract(atan2(p.y, p.x) / TAU) * f32(JELLY_SAMPLES);
    let i0 = i32(floor(u)) % JELLY_SAMPLES;
    let i1 = (i0 + 1) % JELLY_SAMPLES;
    let r = i32(row);
    let e = mix(textureLoad(jellyTex, vec2i(i0, r), 0).r, textureLoad(jellyTex, vec2i(i1, r), 0).r, fract(u));
    let cd = JELLY_CORE * min(1.0, e);
    var s = rho - e + 1.0;
    if (rho <= cd) {
        s = rho * JELLY_CORE / cd;
    } else if (rho <= e) {
        s = JELLY_CORE + (rho - cd) * (1.0 - JELLY_CORE) / (e - cd);
    }
    return p * (s / rho);
}

fn boxCover(p: f32, extent: f32, aa: f32) -> f32 {
    let lo = max(p - 0.5 * aa, -extent);
    let hi = min(p + 0.5 * aa, extent);
    return clamp((hi - lo) / aa, 0.0, 1.0);
}

fn over(top: vec4f, below: vec4f) -> vec4f {
    return top + below * (1.0 - top.a);
}

fn decor(i: Out) -> vec4f {
    if ((i.flags & CARD_TINT) != 0u) {
        return i.color1;
    }
    if ((i.flags & CARD_ENEMY) == 0u) {
        return vec4f(0.0);
    }
    let p = i.local;
    let band = ENEMY_LINE * 0.5;
    let ring = clamp((band - abs(length(p) - 1.0 + ENEMY_RING_INSET)) / i.aa + 0.5, 0.0, 1.0);
    let across = abs(p.x + p.y) * 0.70710678;
    let along = abs(p.x - p.y) * 0.70710678;
    let slash = clamp((band - across) / i.aa + 0.5, 0.0, 1.0) * clamp((SLASH_HALF - along) / i.aa + 0.5, 0.0, 1.0);
    let team = vec4f(i.color1.rgb, 1.0);
    let dim = vec4f(0.0, 0.0, 0.0, ENEMY_DIM);
    return over(team * (i.color1.a * slash), over(team * (i.color1.a * ring), dim));
}

fn card(i: Out, skin: vec4f) -> vec4f {
    var base = i.color0.rgb;
    if ((i.flags & CARD_SKINNED) != 0u) {
        let k = i.aux.x * clamp((1.0 - length(i.local)) / i.aa + 0.5, 0.0, 1.0);
        base = base * (1.0 - skin.a * k) + skin.rgb * k;
    }
    let alpha = i.aux.y;
    let d = decor(i);
    let under = alpha * (1.0 - d.a);
    return vec4f(alpha * d.rgb + under * base, alpha * d.a + under);
}

@fragment
fn fs(i: Out) -> @location(0) vec4f {
    let page = select(0u, i.layer, i.kind == KIND_GLYPH || i.kind == KIND_SDF);
    let text = textureSample(atlasTex, atlasSampler, i.uv, page);
    let back = textureSample(backdropTex, atlasSampler, i.uv);
    let skinLayer = min(i.layer, textureNumLayers(skinTex) - 1u);
    var local = i.local;
    if (i.kind == KIND_DISC || i.kind == KIND_CARD || i.kind == KIND_GRADIENT || i.kind == KIND_SHADOW) {
        local = jellyWarp(i.local, i.jelly);
    }
    let skin = textureSample(skinTex, skinSampler, i.local * 0.5 + 0.5, skinLayer);
    if (i.kind == KIND_IMAGE) {
        return back * i.color0;
    }
    if (i.kind == KIND_ARROW) {
        let span = ARROW_TIP - ARROW_BASE;
        let side = ARROW_HALF_WIDTH * (ARROW_TIP - i.local.y) / span - abs(i.local.x);
        let inside = min(min(i.local.y - ARROW_BASE, ARROW_TIP - i.local.y), side);
        return i.color0 * clamp(inside / i.aa + 0.5, 0.0, 1.0);
    }
    if (i.kind == KIND_FRAME && i.flags == FRAME_RAINBOW) {
        let outer = i.size + i.scalar;
        let t = clamp(0.5 + (i.local.x / outer.x - i.local.y / outer.y) * 0.25, 0.0, 1.0);
        return mix(i.color0, i.color1, t) * rainbowFrameAlpha(i.local, i.size, i.scalar, i.aa, i.aux.xy * min(i.size.x, i.size.y));
    }
    if (i.kind == KIND_FRAME) {
        return i.color0 * frameAlpha(i.local, i.size, i.scalar, i.aa, i.flags == FRAME_GLOW);
    }
    if (i.kind == KIND_SHADOW) {
        let d = length(local);
        let a = clamp((d - 1.0 + SHADOW_FEATHER) / SHADOW_FEATHER, 0.0, 1.0) * clamp((SHADOW_REACH - d) / (SHADOW_REACH - 1.0), 0.0, 1.0);
        if (a <= 0.0) {
            discard;
        }
        return i.color0 * a;
    }
    if (i.kind == KIND_GRADIENT) {
        let d = length(local);
        let cover = clamp((1.0 - d) / i.aa + 0.5, 0.0, 1.0) * clamp((d - 1.0 + i.scalar) / i.aa + 0.5, 0.0, 1.0);
        if (cover <= 0.0) {
            discard;
        }
        let t = clamp(0.5 + (local.x - local.y) * i.aux.x, 0.0, 1.0);
        return mix(i.color0, i.color1, t) * cover;
    }
    if (i.kind == KIND_GLYPH) {
        return text * i.color0;
    }
    if (i.kind == KIND_SDF) {
        let dist = (text.a - 0.5) * 2.0 * SDF_SPREAD;
        return i.color0 * clamp(dist / i.aa + 0.5, 0.0, 1.0);
    }
    if (i.kind == KIND_RECT || i.kind == KIND_SEGMENT) {
        return i.color0 * (boxCover(i.local.x, i.size.x, i.aa) * boxCover(i.local.y, i.size.y, i.aa));
    }
    if (i.kind == KIND_STAR) {
        let aa = i.aa * STAR_EXTENT;
        let sd = starDistance(i.local * STAR_EXTENT);
        let fill = i.color0 * clamp(-sd / aa + 0.5, 0.0, 1.0);
        let stroke = i.color1 * clamp((STAR_HALF_LINE - abs(sd)) / aa + 0.5, 0.0, 1.0);
        let c = over(stroke, fill);
        if (c.a <= 0.0) {
            discard;
        }
        return c;
    }
    let d = length(local);
    let edge = clamp((1.0 - d) / i.aa + 0.5, 0.0, 1.0);
    var glow = 0.0;
    if (i.glow > 0.0) {
        glow = i.aux.y * 0.5 * erfcApprox(max(d - 1.0, 0.0) / (i.glow * 1.41421356)) * inverseSqrt(max(d, 1.0));
    }
    if (edge <= 0.0 && glow < GLOW_CUTOFF) {
        discard;
    }
    if (i.kind == KIND_HALO) {
        return i.color0 * (edge * clamp((d - HALO_INNER) / (1.0 - HALO_INNER), 0.0, 1.0));
    }
    if (i.kind == KIND_CARD) {
        return card(i, skin) * edge + vec4f(i.color0.rgb, 1.0) * (glow * (1.0 - edge));
    }
    var c = i.color0;
    if (i.scalar > 0.0) {
        c = mix(c, i.color1, clamp((d - 1.0 + i.scalar) / i.aa + 0.5, 0.0, 1.0));
    }
    return c * edge;
}
