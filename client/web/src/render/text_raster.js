export function render(req) {
    const canvas = new OffscreenCanvas(1, 1);
    const style = (ctx, font) => {
        ctx.font = font;
        ctx.lineJoin = "round";
        ctx.fillStyle = "#fff";
        ctx.strokeStyle = "#000";
    };
    const line = (text, outlined) => {
        let ctx = canvas.getContext("2d", { willReadFrequently: true });
        style(ctx, req.font);
        const width = ctx.measureText(text).width;
        const pad = Math.ceil(req.stroke / 2) + 1;
        const w = Math.ceil(width + pad * 2) + 1;
        const h = Math.ceil(req.px * req.lineHeight + pad * 2);
        canvas.width = w + req.gutter * 2;
        canvas.height = h + req.gutter * 2;
        ctx = canvas.getContext("2d", { willReadFrequently: true });
        style(ctx, req.font);
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.lineWidth = req.stroke;
        const cx = req.gutter + w / 2;
        const cy = req.gutter + h / 2;
        if (outlined) ctx.strokeText(text, cx, cy);
        ctx.fillText(text, cx, cy);
        return { bitmap: canvas.transferToImageBitmap(), w, h };
    };
    const strip = (text, outlined) => {
        let ctx = canvas.getContext("2d", { willReadFrequently: true });
        style(ctx, req.font);
        const pad = Math.ceil(req.stroke / 2) + 1;
        const h = Math.ceil(req.px * req.lineHeight + pad * 2);
        const glyphs = [];
        let x = 0;
        for (const ch of text) {
            const advance = ctx.measureText(ch).width;
            const w = Math.ceil(advance + pad * 2) + 1;
            glyphs.push({ ch, x, w, advance });
            x += w + req.gutter * 2;
        }
        const w = x - req.gutter * 2;
        canvas.width = w + req.gutter * 2;
        canvas.height = h + req.gutter * 2;
        ctx = canvas.getContext("2d", { willReadFrequently: true });
        style(ctx, req.font);
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.lineWidth = req.stroke;
        const cy = req.gutter + h / 2;
        for (const g of glyphs) {
            const cx = req.gutter + g.x + g.w / 2;
            if (outlined) ctx.strokeText(g.ch, cx, cy);
            ctx.fillText(g.ch, cx, cy);
        }
        return { bitmap: canvas.transferToImageBitmap(), w, h, glyphs: glyphs.map(({ x, w, advance }) => ({ x, w, advance })) };
    };
    const circular = () => {
        const side = req.side;
        canvas.width = side + req.gutter * 2;
        canvas.height = side + req.gutter * 2;
        const ctx = canvas.getContext("2d", { willReadFrequently: true });
        style(ctx, req.font);
        ctx.textAlign = "left";
        ctx.textBaseline = "alphabetic";
        ctx.lineWidth = req.stroke;
        ctx.translate(req.gutter + side / 2, req.gutter + side / 2);
        const radius = side / 2 * req.radius;
        let angle = req.start;
        for (const ch of req.text) {
            const advance = ctx.measureText(ch).width;
            ctx.save();
            ctx.rotate(angle);
            ctx.strokeText(ch, -advance / 2, radius);
            ctx.fillText(ch, -advance / 2, radius);
            ctx.restore();
            const half = 0.4 * advance + 0.2 * req.px;
            angle -= 2 * Math.atan2(half, Math.sqrt(Math.max(radius * radius - half * half, 0)));
        }
        return { bitmap: canvas.transferToImageBitmap(), w: side, h: side };
    };
    const sdf = () => {
        const ss = req.supersample;
        const spread = req.spread;
        const probe = new OffscreenCanvas(1, 1).getContext("2d");
        style(probe, req.font);
        const advance = probe.measureText(req.text).width / ss;
        const kerns = [...req.followers].map((c) => (probe.measureText(req.text + c).width - probe.measureText(req.text).width - probe.measureText(c).width) / ss);
        const w = Math.ceil(advance + spread * 2 + 3);
        const h = Math.ceil(req.px * req.lineHeight + spread * 2 + 2);
        const W = w * ss;
        const H = h * ss;
        const big = new OffscreenCanvas(W, H);
        const bigCtx = big.getContext("2d", { willReadFrequently: true });
        style(bigCtx, req.font);
        bigCtx.textAlign = "center";
        bigCtx.textBaseline = "middle";
        bigCtx.fillText(req.text, W / 2, H / 2);
        const data = bigCtx.getImageData(0, 0, W, H).data;
        big.width = big.height = 0;
        const far = 1e12;
        const outside = new Float32Array(W * H);
        const inside = new Float32Array(W * H);
        for (let i = 0; i < W * H; i++) {
            const a = data[i * 4 + 3] / 255;
            const edge = 0.5 - a;
            outside[i] = a >= 1 ? 0 : a <= 0 ? far : Math.max(edge, 0) ** 2;
            inside[i] = a <= 0 ? 0 : a >= 1 ? far : Math.max(-edge, 0) ** 2;
        }
        const longest = Math.max(W, H);
        const f = new Float32Array(longest);
        const d = new Float32Array(longest);
        const v = new Int32Array(longest);
        const z = new Float64Array(longest + 1);
        const line = (n) => {
            let k = 0;
            v[0] = 0;
            z[0] = -1e20;
            z[1] = 1e20;
            for (let q = 1; q < n; q++) {
                let s;
                do {
                    const r = v[k];
                    s = (f[q] - f[r] + q * q - r * r) / (2 * q - 2 * r);
                } while (s <= z[k] && --k > -1);
                k++;
                v[k] = q;
                z[k] = s;
                z[k + 1] = 1e20;
            }
            k = 0;
            for (let q = 0; q < n; q++) {
                while (z[k + 1] < q) k++;
                const r = v[k];
                d[q] = f[r] + (q - r) * (q - r);
            }
        };
        const transform = (grid) => {
            for (let x = 0; x < W; x++) {
                for (let y = 0; y < H; y++) f[y] = grid[y * W + x];
                line(H);
                for (let y = 0; y < H; y++) grid[y * W + x] = d[y];
            }
            for (let y = 0; y < H; y++) {
                for (let x = 0; x < W; x++) f[x] = grid[y * W + x];
                line(W);
                for (let x = 0; x < W; x++) grid[y * W + x] = d[x];
            }
        };
        transform(outside);
        transform(inside);
        const tw = w + req.gutter * 2;
        const th = h + req.gutter * 2;
        const out = new Uint8ClampedArray(tw * th * 4);
        for (let i = 3; i < out.length; i += 4) {
            out[i - 3] = out[i - 2] = out[i - 1] = 255;
        }
        const scale = 0.5 / (spread * ss);
        for (let y = 0; y < h; y++) {
            for (let x = 0; x < w; x++) {
                let sum = 0;
                for (let sy = 0; sy < ss; sy++) {
                    for (let sx = 0; sx < ss; sx++) {
                        const i = (y * ss + sy) * W + x * ss + sx;
                        sum += Math.sqrt(outside[i]) - Math.sqrt(inside[i]);
                    }
                }
                out[((y + req.gutter) * tw + x + req.gutter) * 4 + 3] = Math.round((0.5 - (sum / (ss * ss)) * scale) * 255);
            }
        }
        canvas.width = tw;
        canvas.height = th;
        canvas.getContext("2d", { willReadFrequently: true }).putImageData(new ImageData(out, tw, th), 0, 0);
        return { bitmap: canvas.transferToImageBitmap(), w, h, advance, kerns, glyphs: [{ x: 0, w, advance }] };
    };
    switch (req.kind) {
        case "circular":
            return circular();
        case "glyphs":
            return strip(req.text, req.outlined);
        case "sdf":
            return sdf();
        default:
            return line(req.text, req.outlined);
    }
}

function fontFaces(families) {
    const faces = [];
    for (const sheet of document.styleSheets) {
        let rules;
        try {
            rules = sheet.cssRules;
        } catch {
            continue;
        }
        const base = sheet.href || document.baseURI;
        for (const rule of rules) {
            if (!(rule instanceof CSSFontFaceRule)) continue;
            const s = rule.style;
            const src = s.getPropertyValue("src").replace(/url\((['"]?)([^'")]+)\1\)/g, (_, q, u) => `url("${new URL(u, base).href}")`);
            const family = s.getPropertyValue("font-family").replace(/^['"]|['"]$/g, "");
            if (!families.includes(family)) continue;
            faces.push({
                family,
                src,
                descriptors: {
                    style: s.getPropertyValue("font-style") || "normal",
                    weight: s.getPropertyValue("font-weight") || "normal",
                    unicodeRange: s.getPropertyValue("unicode-range") || "U+0-10FFFF",
                },
            });
        }
    }
    const unreadable = families.filter(f => !faces.some(x => x.family === f) && [...document.fonts].some(d => d.family.replace(/^['"]|['"]$/g, "") === f));
    return { faces, unreadable };
}

function workerMain(render) {
    let queue = [];
    const added = new Set();
    const custom = new Set();
    let generation = 0;
    const unquote = family => family.replace(/^['"]|['"]$/g, "");
    const run = jobs => {
        for (const req of jobs) {
            const draw = () => {
                const item = render(custom.size ? { ...req, font: `${req.font}, "loaded-${generation}"` } : req);
                self.postMessage({ id: req.id, item }, [item.bitmap]);
            };
            if (custom.size) self.fonts.load(req.font, req.text).then(draw, draw);
            else draw();
        }
    };
    self.fonts.addEventListener("loadingdone", e => {
        if (e.fontfaces.some(f => custom.has(unquote(f.family)))) {
            generation += 1;
            self.postMessage({ fontsLoaded: true });
        }
    });
    self.fonts.addEventListener("loadingerror", e => {
        const failed = e.fontfaces.filter(f => custom.has(unquote(f.family)));
        if (failed.length) self.postMessage({ fontErrors: failed.map(f => `${f.family} ${f.weight} ${f.unicodeRange}`) });
    });
    self.onmessage = e => {
        const msg = e.data;
        if (msg.extra) {
            for (const f of msg.extra) {
                const key = `${f.family}|${f.src}|${f.descriptors.style}|${f.descriptors.weight}|${f.descriptors.unicodeRange}`;
                if (added.has(key)) continue;
                added.add(key);
                custom.add(f.family);
                self.fonts.add(new FontFace(f.family, f.src, f.descriptors));
            }
            return;
        }
        if (msg.faces) {
            Promise.all(msg.faces.map(f => {
                const face = new FontFace(f.family, f.src, f.descriptors);
                self.fonts.add(face);
                return face.load().then(() => null, err => ({ family: f.family, src: f.src, error: String(err) }));
            })).then(results => {
                const failures = results.filter(Boolean);
                if (failures.length) self.postMessage({ fontFailures: failures });
                const jobs = queue;
                queue = null;
                setTimeout(() => run(jobs));
            });
            return;
        }
        if (queue) queue.push(...msg.jobs);
        else run(msg.jobs);
    };
}

export function addFaces(worker, family) {
    const { faces } = fontFaces([family]);
    if (faces.length) worker.postMessage({ extra: faces });
}

export function startWorker(onResult, onFailure, onFonts, families) {
    const { faces, unreadable } = fontFaces(families);
    if (unreadable.length) throw new Error(`@font-face rules unreadable for ${unreadable.join(", ")}`);
    const source = `(${workerMain.toString()})(${render.toString()});`;
    const url = URL.createObjectURL(new Blob([source], { type: "text/javascript" }));
    let worker;
    try {
        worker = new Worker(url);
    } finally {
        URL.revokeObjectURL(url);
    }
    worker.onmessage = e => {
        if (e.data.fontsLoaded) {
            onFonts();
            return;
        }
        if (e.data.fontErrors) {
            console.error("text worker: custom font failed to load", e.data.fontErrors);
            return;
        }
        if (e.data.fontFailures) {
            console.error(`text worker: built-in font failed to load: ${e.data.fontFailures.map(f => `${f.family}: ${f.error}`).join("; ")}`);
            return;
        }
        onResult(e.data.id, e.data.item);
    };
    worker.onerror = e => {
        const reason = `${e.message} (${e.filename}:${e.lineno}:${e.colno})`;
        console.error(`text worker error: ${reason}`, e.error);
        onFailure(reason);
    };
    worker.onmessageerror = e => {
        console.error("text worker: message could not be deserialized", e);
        onFailure("message could not be deserialized");
    };
    worker.postMessage({ faces });
    return worker;
}
