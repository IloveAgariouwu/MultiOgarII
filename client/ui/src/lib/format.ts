import type { ServerPopulation } from "./game.svelte";
import type { Population } from "./types";

export function population(p: Population | undefined): string {
    return p ? (p.max ? `${p.pop} / ${p.max}` : String(p.pop)) : "-/-";
}

export function livePopulation(p: ServerPopulation): Population | undefined {
    return p.max ? { pop: p.players, max: p.max } : undefined;
}

export function mass(m: number): string {
    return m >= 1000 ? `${(m / 1000).toFixed(1)}k` : String(Math.round(m));
}

export function bytes(n: number): string {
    if (n < 1048576) return `${Math.ceil(n / 1024)}KB`;
    if (n < 1073741824) return `${(n / 1048576).toFixed(n < 10485760 ? 1 : 0)}MB`;
    return `${(n / 1073741824).toFixed(1)}GB`;
}

export function argbCss(v: number): string {
    return `rgba(${(v >>> 16) & 255},${(v >>> 8) & 255},${v & 255},${+(((v >>> 24) & 255) / 255).toFixed(3)})`;
}

export function argbHex(v: number): string {
    const h = (n: number) => n.toString(16).padStart(2, "0");
    return `#${h((v >>> 16) & 255)}${h((v >>> 8) & 255)}${h(v & 255)}${h((v >>> 24) & 255)}`;
}

export function hexArgb(text: string): number | null {
    const m = /^#?([0-9a-f]{6})([0-9a-f]{2})?$/i.exec(text.trim());
    return m ? ((parseInt(m[2] ?? "ff", 16) << 24) | parseInt(m[1], 16)) >>> 0 : null;
}

const rgbOf = new Map<string, number>();
let colorProbe: OffscreenCanvasRenderingContext2D | null = null;

export function cssRgb(color: string): number {
    const known = rgbOf.get(color);
    if (known !== undefined) return known;
    colorProbe ??= new OffscreenCanvas(1, 1).getContext("2d");
    if (!colorProbe) throw new Error("2d context unavailable for color parsing");
    colorProbe.fillStyle = "#000000";
    colorProbe.fillStyle = color;
    const css = String(colorProbe.fillStyle);
    const channels = css.startsWith("#") ? null : (css.match(/[\d.]+/g) ?? []).slice(0, 3).map(Number);
    const rgb = channels ? (channels[0] << 16) | (channels[1] << 8) | channels[2] : parseInt(css.slice(1, 7), 16);
    rgbOf.set(color, rgb);
    return rgb;
}

export function hourMinute(d: Date): string {
    return `${d.getHours()}:${String(d.getMinutes()).padStart(2, "0")}`;
}

export function dateTimeStamp(d: Date): string {
    const p = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}
