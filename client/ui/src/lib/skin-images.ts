import type { Action } from "svelte/action";

const MAX_SIDE = 2048;
const BITMAP_SIDE = 256;
const IDLE_BITMAPS = 256;
const MAX_BACKING = 8192;
const RAINBOW_MS = 33;
const MOTION_STARTS = ["transitionrun", "animationstart"];
const SKIN_NAME = /^[A-Za-z0-9_-][A-Za-z0-9_.-]*$/;
const SKIN_PATH = /^(?:\.?\/)?skins\/([^/]+)$/;
const WEB_SKIN = /^https?:\/\/\S+$/;

export function skinUrl(value: string): string {
    const text = value.trim();
    if (WEB_SKIN.test(text)) return text;
    if (SKIN_NAME.test(text)) return `skins/${text}.png`;
    const file = SKIN_PATH.exec(text)?.[1];
    return file && SKIN_NAME.test(file) ? `skins/${file}` : "";
}

async function loadImage(url: string): Promise<ImageBitmap | null> {
    try {
        const response = await fetch(url, { credentials: "omit", referrerPolicy: "no-referrer" });
        if (!response.ok) return null;
        const image = await createImageBitmap(await response.blob());
        if (image.width <= MAX_SIDE && image.height <= MAX_SIDE) return image;
        image.close();
        return null;
    } catch {
        return null;
    }
}

async function resolveSkin(clean: string): Promise<ImageBitmap | null> {
    const image = await loadImage(clean);
    if (!image) return null;
    const scale = Math.min(1, BITMAP_SIDE / Math.max(image.width, image.height));
    const small = await createImageBitmap(image, {
        resizeWidth: Math.max(1, Math.round(image.width * scale)),
        resizeHeight: Math.max(1, Math.round(image.height * scale)),
        resizeQuality: "high",
    }).catch(() => null);
    image.close();
    return small;
}

interface Bitmap {
    ready: Promise<ImageBitmap | null>;
    image: ImageBitmap | null;
    users: number;
}

const bitmaps = new Map<string, Bitmap>();

function takeBitmap(clean: string): Bitmap {
    let bitmap = bitmaps.get(clean);
    if (bitmap) bitmaps.delete(clean);
    else {
        const fresh: Bitmap = { ready: resolveSkin(clean), image: null, users: 0 };
        void fresh.ready.then((image) => (fresh.image = image));
        bitmap = fresh;
    }
    bitmap.users++;
    bitmaps.set(clean, bitmap);
    return bitmap;
}

function dropBitmap(clean: string): void {
    const bitmap = bitmaps.get(clean);
    if (!bitmap) return;
    bitmap.users--;
    const idle = [...bitmaps].filter(([, b]) => b.users === 0);
    for (const [key, b] of idle.slice(0, Math.max(0, idle.length - IDLE_BITMAPS))) {
        bitmaps.delete(key);
        void b.ready.then((image) => image?.close());
    }
}

const RGB_STOPS: [number, number, number, number][] = [
    [0, 255, 66, 66],
    [8, 255, 167, 79],
    [16, 255, 255, 103],
    [25, 163, 255, 72],
    [33, 70, 255, 70],
    [41, 80, 253, 166],
    [50, 85, 252, 252],
    [58, 74, 163, 252],
    [66, 66, 66, 255],
    [75, 165, 77, 253],
    [83, 255, 77, 255],
    [91, 255, 75, 165],
    [100, 255, 66, 66],
];
const RGB_CYCLE_MS = 20000;

function rgbNow(): string {
    const p = ((performance.now() % RGB_CYCLE_MS) / RGB_CYCLE_MS) * 100;
    let i = 0;
    while (i < RGB_STOPS.length - 1 && RGB_STOPS[i + 1][0] <= p) i++;
    const a = RGB_STOPS[i],
        b = RGB_STOPS[i + 1],
        t = (p - a[0]) / (b[0] - a[0]);
    const ch = (k: number) => Math.round(a[k] + (b[k] - a[k]) * t);
    return `rgb(${ch(1)},${ch(2)},${ch(3)})`;
}

interface Slot {
    el: HTMLElement;
    clean: string | null;
    bitmap: Bitmap | null;
    fill: string | undefined;
    alpha: number;
    x: number;
    y: number;
    side: number;
}

function layerCanvas(): HTMLCanvasElement {
    const canvas = document.createElement("canvas");
    canvas.className = "avatar-layer";
    canvas.setAttribute("aria-hidden", "true");
    return canvas;
}

function scrolls(el: HTMLElement): boolean {
    const style = getComputedStyle(el);
    return /(auto|scroll)/.test(style.overflowY + style.overflowX);
}

const scratch = document.createElement("canvas");

function compositeSkin(fill: string, image: ImageBitmap | null | undefined, side: number): HTMLCanvasElement {
    const size = Math.max(1, Math.ceil(side));
    if (scratch.width !== size || scratch.height !== size) scratch.width = scratch.height = size;
    const ctx = scratch.getContext("2d");
    if (!ctx) return scratch;
    ctx.clearRect(0, 0, size, size);
    ctx.fillStyle = fill;
    ctx.fillRect(0, 0, size, size);
    if (image) ctx.drawImage(image, 0, 0, size, size);
    return scratch;
}

class AvatarHost {
    readonly slots = new Set<Slot>();
    private readonly rainbow = layerCanvas();
    private readonly skins = layerCanvas();
    private readonly resize = new ResizeObserver(() => this.schedule());
    private readonly mutations = new MutationObserver((records) => {
        if (records.some((r) => r.target !== this.rainbow && r.target !== this.skins)) this.schedule();
    });
    private frame = 0;
    private ratio = 1;
    private madePositioned = false;

    constructor(readonly el: HTMLElement) {
        if (getComputedStyle(el).position === "static") {
            el.style.position = "relative";
            this.madePositioned = true;
        }
        el.append(this.rainbow, this.skins);
        this.resize.observe(el);
        this.mutations.observe(el, {
            childList: true,
            subtree: true,
            characterData: true,
            attributeFilter: ["class", "style"],
        });
        for (const type of MOTION_STARTS) el.addEventListener(type, this.started);
    }

    private readonly started = () => this.schedule();

    private animating(): boolean {
        return this.el
            .getAnimations({ subtree: true })
            .some((a) => a.playState === "running" && a.effect?.getTiming().iterations !== Infinity);
    }

    get hasRainbow(): boolean {
        for (const slot of this.slots) if (!slot.fill) return true;
        return false;
    }

    schedule(): void {
        this.frame ||= requestAnimationFrame(() => {
            this.frame = 0;
            this.layout();
            if (this.animating()) this.schedule();
        });
    }

    private layout(): void {
        const host = this.el.getBoundingClientRect();
        const scale = this.el.offsetWidth ? host.width / this.el.offsetWidth : 1;
        if (!scale) return;
        for (const canvas of [this.rainbow, this.skins]) canvas.style.width = canvas.style.height = "0px";
        const width = this.el.scrollWidth;
        const height = this.el.scrollHeight;
        this.ratio = Math.min(devicePixelRatio, 2, MAX_BACKING / Math.max(width, height, 1));
        for (const canvas of [this.rainbow, this.skins]) {
            canvas.style.width = `${width}px`;
            canvas.style.height = `${height}px`;
            const w = Math.max(1, Math.round(width * this.ratio));
            const h = Math.max(1, Math.round(height * this.ratio));
            if (canvas.width !== w || canvas.height !== h) {
                canvas.width = w;
                canvas.height = h;
            }
        }
        for (const slot of this.slots) {
            const r = slot.el.getBoundingClientRect();
            slot.x = (r.left - host.left) / scale + this.el.scrollLeft - this.el.clientLeft;
            slot.y = (r.top - host.top) / scale + this.el.scrollTop - this.el.clientTop;
            slot.side = r.width / scale;
            slot.alpha = this.opacity(slot.el);
        }
        this.redraw();
    }

    private opacity(el: HTMLElement): number {
        let alpha = 1;
        for (let at: HTMLElement | null = el; at && at !== this.el; at = at.parentElement) {
            const style = getComputedStyle(at);
            if (style.visibility === "hidden" || style.display === "none") return 0;
            alpha *= Number(style.opacity);
        }
        return alpha;
    }

    private circle(ctx: CanvasRenderingContext2D, slot: Slot): void {
        const k = this.ratio;
        ctx.beginPath();
        ctx.arc((slot.x + slot.side / 2) * k, (slot.y + slot.side / 2) * k, (slot.side / 2) * k, 0, Math.PI * 2);
    }

    private drawMask(): void {
        const ctx = this.rainbow.getContext("2d");
        if (!ctx) return;
        ctx.globalCompositeOperation = "source-over";
        ctx.globalAlpha = 1;
        ctx.clearRect(0, 0, this.rainbow.width, this.rainbow.height);
        ctx.fillStyle = rgbNow();
        const k = this.ratio;
        for (const slot of this.slots) {
            if (slot.fill || !slot.side || !slot.alpha) continue;
            ctx.save();
            this.circle(ctx, slot);
            ctx.clip();
            ctx.globalCompositeOperation = "source-over";
            ctx.globalAlpha = slot.alpha;
            ctx.fill();
            const image = slot.bitmap?.image;
            if (image && slot.alpha < 1) {
                ctx.globalCompositeOperation = "destination-out";
                ctx.globalAlpha = 1;
                ctx.drawImage(image, slot.x * k, slot.y * k, slot.side * k, slot.side * k);
            }
            ctx.restore();
        }
    }

    redraw(): void {
        this.drawMask();
        this.drawSkins();
    }

    private drawSkins(): void {
        const ctx = this.skins.getContext("2d");
        if (!ctx) return;
        ctx.clearRect(0, 0, this.skins.width, this.skins.height);
        const k = this.ratio;
        for (const slot of this.slots) {
            if (!slot.side || !slot.alpha) continue;
            const image = slot.bitmap?.image;
            if (!slot.fill && !image) continue;
            const x = slot.x * k,
                y = slot.y * k,
                side = slot.side * k;
            ctx.save();
            this.circle(ctx, slot);
            ctx.clip();
            ctx.globalAlpha = slot.alpha;
            if (slot.fill && slot.alpha < 1) ctx.drawImage(compositeSkin(slot.fill, image, side), x, y, side, side);
            else {
                if (slot.fill) {
                    ctx.fillStyle = slot.fill;
                    ctx.fill();
                }
                if (image) ctx.drawImage(image, x, y, side, side);
            }
            ctx.restore();
        }
    }

    tint(color: string): void {
        const ctx = this.rainbow.getContext("2d");
        if (!ctx) return;
        ctx.globalCompositeOperation = "source-in";
        ctx.globalAlpha = 1;
        ctx.fillStyle = color;
        ctx.fillRect(0, 0, this.rainbow.width, this.rainbow.height);
    }

    destroy(): void {
        cancelAnimationFrame(this.frame);
        this.resize.disconnect();
        this.mutations.disconnect();
        for (const type of MOTION_STARTS) this.el.removeEventListener(type, this.started);
        this.rainbow.remove();
        this.skins.remove();
        if (this.madePositioned) this.el.style.position = "";
    }
}

const hosts = new Map<HTMLElement, AvatarHost>();
let rainbowTimer = 0;

function tintAll(): void {
    if (document.hidden) return;
    const color = rgbNow();
    for (const host of hosts.values()) if (host.hasRainbow) host.tint(color);
}

function syncRainbowTimer(): void {
    const needed = [...hosts.values()].some((h) => h.hasRainbow);
    if (needed && !rainbowTimer) rainbowTimer = window.setInterval(tintAll, RAINBOW_MS);
    else if (!needed && rainbowTimer) {
        clearInterval(rainbowTimer);
        rainbowTimer = 0;
    }
}

function hostFor(el: HTMLElement): AvatarHost {
    let positioned: HTMLElement | null = null;
    let target: HTMLElement | null = null;
    for (let at = el.parentElement; at && at !== document.body; at = at.parentElement) {
        if (scrolls(at)) {
            target = at;
            break;
        }
        if (!positioned && getComputedStyle(at).position !== "static") positioned = at;
    }
    target ??= positioned ?? document.body;
    let host = hosts.get(target);
    if (!host) hosts.set(target, (host = new AvatarHost(target)));
    return host;
}

function releaseSlot(host: AvatarHost, slot: Slot): void {
    host.slots.delete(slot);
    if (slot.clean) dropBitmap(slot.clean);
    if (!host.slots.size) {
        host.destroy();
        hosts.delete(host.el);
    } else host.schedule();
    syncRainbowTimer();
}

export interface SkinParams {
    url: string;
    fill?: string;
}

export const skin: Action<HTMLElement, SkinParams> = (el, params) => {
    const host = hostFor(el);
    const slot: Slot = { el, clean: null, bitmap: null, fill: undefined, alpha: 1, x: 0, y: 0, side: 0 };
    el.style.background = "transparent";
    host.slots.add(slot);
    const show = ({ url, fill }: SkinParams) => {
        if (slot.clean) dropBitmap(slot.clean);
        slot.fill = fill || undefined;
        slot.clean = skinUrl(url) || null;
        const bitmap = slot.clean ? takeBitmap(slot.clean) : null;
        slot.bitmap = bitmap;
        void bitmap?.ready.then(() => {
            if (slot.bitmap === bitmap && host.slots.has(slot)) host.redraw();
        });
        host.schedule();
        syncRainbowTimer();
    };
    let shown = params;
    show(params);
    return {
        update: (next: SkinParams) => {
            if (next.url === shown.url && next.fill === shown.fill) return;
            shown = next;
            show(next);
        },
        destroy: () => releaseSlot(host, slot),
    };
};
