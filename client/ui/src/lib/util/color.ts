export interface Hsva {
    h: number;
    s: number;
    v: number;
    a: number;
}

export function argbHsva(argb: number): Hsva {
    const r = ((argb >>> 16) & 255) / 255,
        g = ((argb >>> 8) & 255) / 255,
        b = (argb & 255) / 255;
    const max = Math.max(r, g, b),
        d = max - Math.min(r, g, b);
    const h = !d ? 0 : max === r ? ((g - b) / d + 6) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
    return { h: h * 60, s: max ? d / max : 0, v: max, a: ((argb >>> 24) & 255) / 255 };
}

export function hsvaArgb({ h, s, v, a }: Hsva): number {
    const ch = (n: number) => {
        const k = (n + h / 60) % 6;
        return Math.round((v - v * s * Math.max(0, Math.min(k, 4 - k, 1))) * 255);
    };
    return ((Math.round(a * 255) << 24) | (ch(5) << 16) | (ch(3) << 8) | ch(1)) >>> 0;
}

export const HUE_RANGE = 360;

export function hueCss(h: number): string {
    return `hsl(${h} 100% 50%)`;
}
