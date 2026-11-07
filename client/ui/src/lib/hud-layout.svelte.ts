import type { Attachment } from "svelte/attachments";
import { loadGoogleFont } from "./google-fonts";
import type { TextKey } from "./i18n";
import { PERCENT } from "./settings-format";

export type HudSlot = "chat" | "status" | "input" | "replay" | "spectate" | "rail" | "self" | "map";
export interface HudPoint {
    ax: "left" | "right";
    ay: "top" | "bottom";
    remX: number;
    remY: number;
}
type FractionPoint = [number, number];
interface PixelPoint {
    ax: "left" | "right";
    ay: "top" | "bottom";
    dx: number;
    dy: number;
}
export type StoredPoint = HudPoint | FractionPoint | PixelPoint;

function isCurrentPoint(p: StoredPoint): p is HudPoint {
    return !Array.isArray(p) && "remX" in p;
}

function rootPx(): number {
    return parseFloat(getComputedStyle(document.documentElement).fontSize);
}
export interface HudSpot {
    point: StoredPoint | undefined;
    save(point: HudPoint): void;
    clear(): void;
}
export type HudLayout = Partial<Record<HudSlot, StoredPoint>>;
export type HudScales = Partial<Record<HudSlot, number>>;
export type HudFonts = Partial<Record<HudSlot, string>>;

export const HUD_ZOOM = { min: 50, max: 200, step: 5 };
const LAYOUT_KEY = "HudLayout";
const DRAG_SLOP = 4;
const DOCK_PX = 48;
const PLACED_STYLES = [
    "left",
    "top",
    "right",
    "bottom",
    "transform",
    "margin",
    "display",
    "zoom",
    "font-family",
    "--text",
];

interface StoredLayout extends HudLayout {
    hidden: HudSlot[];
    scale: HudScales;
    font: HudFonts;
    text: HudScales;
}

export function snapZoom(percent: number): number {
    const snapped = Math.round(percent / HUD_ZOOM.step) * HUD_ZOOM.step;
    return Math.min(HUD_ZOOM.max, Math.max(HUD_ZOOM.min, snapped)) / PERCENT;
}

function stored(raw: string): StoredLayout {
    const empty: StoredLayout = { hidden: [], scale: {}, font: {}, text: {} };
    if (!raw) return empty;
    try {
        return { ...empty, ...JSON.parse(raw) };
    } catch (e) {
        console.error("HUD layout setting is not valid JSON", raw, e);
        return empty;
    }
}

interface LayoutHost {
    settings: Record<string, unknown>;
    setSetting(key: string, value: string): void;
    t(key: TextKey): string;
}

export class HudEditor {
    selected = $state<HudSlot | null>(null);
    private active = $state(false);
    private host: LayoutHost;
    private viewport = $state({ w: innerWidth, h: innerHeight });
    private placed = new Set<HTMLElement>();

    constructor(host: LayoutHost) {
        this.host = host;
        addEventListener("resize", () => (this.viewport = { w: innerWidth, h: innerHeight }));
    }

    get editing(): boolean {
        return this.active;
    }

    set editing(on: boolean) {
        this.active = on;
        this.selected = null;
    }

    get layout(): HudLayout {
        const { hidden: _, scale: __, font: ___, text: ____, ...layout } = stored(this.raw);
        return layout;
    }

    get customized(): boolean {
        return (
            Object.keys(this.layout).length > 0 ||
            this.hidden.length > 0 ||
            Object.keys(this.scales).length > 0 ||
            Object.keys(this.fonts).length > 0 ||
            Object.keys(this.texts).length > 0
        );
    }

    get fonts(): HudFonts {
        return stored(this.raw).font;
    }

    get texts(): HudScales {
        return stored(this.raw).text;
    }

    get scales(): HudScales {
        return stored(this.raw).scale;
    }

    get hidden(): HudSlot[] {
        return stored(this.raw).hidden;
    }

    private get raw(): string {
        return String(this.host.settings[LAYOUT_KEY]);
    }

    private write(next: StoredLayout): void {
        this.host.setSetting(LAYOUT_KEY, JSON.stringify(next));
    }

    toggleHidden(slot: HudSlot): void {
        const now = stored(this.raw);
        const hidden = now.hidden.includes(slot) ? now.hidden.filter((s) => s !== slot) : [...now.hidden, slot];
        this.write({ ...now, hidden });
    }

    reset(): void {
        this.host.setSetting(LAYOUT_KEY, "");
    }

    private save(slot: HudSlot, point: HudPoint): void {
        this.write({ ...stored(this.raw), [slot]: point });
    }

    spot(slot: HudSlot): HudSpot {
        return {
            point: this.layout[slot],
            save: (point) => this.save(slot, point),
            clear: () => {
                const { [slot]: _, ...rest } = stored(this.raw);
                this.write(rest);
            },
        };
    }

    private point(box: DOMRect): HudPoint {
        const { w: vw, h: vh } = this.viewport;
        const right = vw - box.right,
            bottom = vh - box.bottom;
        const ax = right <= DOCK_PX && right < box.left ? "right" : "left",
            ay = bottom <= DOCK_PX && bottom < box.top ? "bottom" : "top";
        const rem = rootPx();
        return {
            ax,
            ay,
            remX: Math.max(0, ax === "left" ? box.left : right) / rem,
            remY: Math.max(0, ay === "top" ? box.top : bottom) / rem,
        };
    }

    private resolve(p: StoredPoint, w: number, h: number): [number, number] {
        const { w: vw, h: vh } = this.viewport;
        const room = [Math.max(0, vw - w), Math.max(0, vh - h)];
        if (Array.isArray(p)) return [p[0] * room[0], p[1] * room[1]];
        const rem = rootPx();
        const [dx, dy] = isCurrentPoint(p) ? [p.remX * rem, p.remY * rem] : [p.dx, p.dy];
        const left = p.ax === "left" ? dx : vw - w - dx,
            top = p.ay === "top" ? dy : vh - h - dy;
        return [Math.min(room[0], Math.max(0, left)), Math.min(room[1], Math.max(0, top))];
    }

    setScale(slot: HudSlot, zoom: number): void {
        const now = stored(this.raw);
        const { [slot]: _, ...rest } = now.scale;
        this.write({ ...now, scale: zoom === 1 ? rest : { ...rest, [slot]: zoom } });
    }

    setFont(slot: HudSlot, name: string): void {
        const now = stored(this.raw);
        const { [slot]: _, ...rest } = now.font;
        this.write({ ...now, font: name ? { ...rest, [slot]: name } : rest });
    }

    setText(slot: HudSlot, size: number): void {
        const now = stored(this.raw);
        const { [slot]: _, ...rest } = now.text;
        this.write({ ...now, text: size === 1 ? rest : { ...rest, [slot]: size } });
    }

    resetSlot(slot: HudSlot): void {
        const {
            [slot]: _,
            hidden,
            scale: { [slot]: __, ...scale },
            font: { [slot]: ___, ...font },
            text: { [slot]: ____, ...text },
            ...rest
        } = stored(this.raw);
        this.write({ ...rest, hidden: hidden.filter((s) => s !== slot), scale, font, text });
    }

    slot(slot: HudSlot): Attachment<HTMLElement> {
        const spot = this.spot(slot);
        return (node) => {
            const point = gMobile ? undefined : spot.point;
            const editing = this.editing;
            const { w: vw, h: vh } = this.viewport;
            let zoom = gMobile ? 1 : (this.scales[slot] ?? 1);
            if (zoom !== 1) node.style.zoom = String(zoom);
            const text = gMobile ? undefined : this.texts[slot];
            if (text) node.style.setProperty("--text", String(text));
            const font = gMobile ? undefined : this.fonts[slot];
            if (font) {
                loadGoogleFont(font);
                node.style.fontFamily = `${JSON.stringify(font)}, var(--font-base)`;
            }
            node.classList.add("hud-slot");
            const at = (x: number, y: number): HudPoint =>
                this.point(new DOMRect(x, y, node.offsetWidth * zoom, node.offsetHeight * zoom));
            const place = (p: StoredPoint) => {
                const [left, top] = this.resolve(p, node.offsetWidth * zoom, node.offsetHeight * zoom);
                Object.assign(node.style, {
                    left: `${left / zoom}px`,
                    top: `${top / zoom}px`,
                    right: "auto",
                    bottom: "auto",
                    transform: "none",
                    margin: "0",
                });
            };
            this.placed.add(node);
            const resize = new ResizeObserver(() => point && place(point));
            if (point) {
                place(point);
                resize.observe(node);
                if (!isCurrentPoint(point) && node.offsetWidth > 0) {
                    const [left, top] = this.resolve(point, node.offsetWidth * zoom, node.offsetHeight * zoom);
                    spot.save(at(left, top));
                }
            }
            node.classList.toggle("hud-edit", editing);
            const off = this.hidden.includes(slot);
            node.classList.toggle("hud-off", editing && off);
            node.classList.toggle("hud-picked", editing && this.selected === slot);
            if (off && !editing) node.style.display = "none";
            const eye = document.createElement("button");
            eye.className = "hud-eye";
            eye.textContent = this.host.t(off ? "hudShowPanel" : "hudHidePanel");
            eye.addEventListener("click", (e) => (e.stopPropagation(), this.toggleHidden(slot)));
            const grip = document.createElement("i");
            grip.className = "hud-grip";
            grip.title = `${Math.round(zoom * 100)}%`;
            if (editing) node.append(eye, grip);
            const scaleFrom = () => {
                const start = node.getBoundingClientRect();
                const from = zoom;
                const move = (ev: PointerEvent) => {
                    const grow = Math.max(
                        (ev.clientX - start.left) / start.width,
                        (ev.clientY - start.top) / start.height,
                    );
                    zoom = snapZoom(from * grow * PERCENT);
                    node.style.zoom = String(zoom);
                    grip.title = `${Math.round(zoom * 100)}%`;
                    if (point) place(point);
                };
                const up = () => {
                    removeEventListener("pointermove", move);
                    removeEventListener("pointerup", up);
                    removeEventListener("pointercancel", up);
                    this.setScale(slot, zoom);
                };
                addEventListener("pointermove", move);
                addEventListener("pointerup", up);
                addEventListener("pointercancel", up);
            };
            const down = (e: PointerEvent) => {
                if (e.target === eye) return;
                if (e.target === grip) {
                    e.preventDefault();
                    e.stopPropagation();
                    return scaleFrom();
                }
                e.preventDefault();
                e.stopPropagation();
                const start = node.getBoundingClientRect();
                const dx = e.clientX - start.left,
                    dy = e.clientY - start.top;
                const others = [...this.placed]
                    .filter((n) => n !== node && n.isConnected && n.offsetWidth > 0)
                    .map((n) => n.getBoundingClientRect());
                const linesX = [0, vw / 2, vw, ...others.flatMap((r) => [r.left, r.left + r.width / 2, r.right])];
                const linesY = [0, vh / 2, vh, ...others.flatMap((r) => [r.top, r.top + r.height / 2, r.bottom])];
                const guideX = document.createElement("i");
                const guideY = document.createElement("i");
                guideX.className = "hud-guide x";
                guideY.className = "hud-guide y";
                guideX.hidden = guideY.hidden = true;
                document.body.append(guideX, guideY);
                let point: HudPoint = at(start.left, start.top);
                const snap = (
                    from: number,
                    size: number,
                    lines: number[],
                    guide: HTMLElement,
                    prop: "left" | "top",
                ) => {
                    let best = { pos: from, line: Number.NaN, gap: 8 };
                    for (const edge of [0, size / 2, size]) {
                        for (const line of lines) {
                            const gap = Math.abs(from + edge - line);
                            if (gap <= best.gap) best = { pos: line - edge, line, gap };
                        }
                    }
                    guide.hidden = Number.isNaN(best.line);
                    guide.style[prop] = `${best.line}px`;
                    return best.pos;
                };
                let moved = false;
                const move = (ev: PointerEvent) => {
                    if (!moved && Math.hypot(ev.clientX - e.clientX, ev.clientY - e.clientY) < DRAG_SLOP) return;
                    moved = true;
                    const free = ev.altKey;
                    const x = free ? ev.clientX - dx : snap(ev.clientX - dx, start.width, linesX, guideX, "left");
                    const y = free ? ev.clientY - dy : snap(ev.clientY - dy, start.height, linesY, guideY, "top");
                    if (free) guideX.hidden = guideY.hidden = true;
                    point = at(x, y);
                    place(point);
                };
                const up = () => {
                    removeEventListener("pointermove", move);
                    removeEventListener("pointerup", up);
                    removeEventListener("pointercancel", up);
                    guideX.remove();
                    guideY.remove();
                    if (moved) spot.save(point);
                    else this.selected = slot;
                };
                addEventListener("pointermove", move);
                addEventListener("pointerup", up);
                addEventListener("pointercancel", up);
            };
            const block = (e: Event) => e.target !== eye && (e.preventDefault(), e.stopPropagation());
            const unscale = (e: Event) => e.target === grip && (e.stopPropagation(), this.setScale(slot, 1));
            if (editing) {
                node.addEventListener("pointerdown", down, true);
                node.addEventListener("click", block, true);
                node.addEventListener("dblclick", unscale, true);
            }
            return () => {
                this.placed.delete(node);
                resize.disconnect();
                node.removeEventListener("pointerdown", down, true);
                node.removeEventListener("click", block, true);
                node.removeEventListener("dblclick", unscale, true);
                node.classList.remove("hud-edit", "hud-off", "hud-slot", "hud-picked");
                eye.remove();
                grip.remove();
                for (const p of PLACED_STYLES) node.style.removeProperty(p);
            };
        };
    }
}
