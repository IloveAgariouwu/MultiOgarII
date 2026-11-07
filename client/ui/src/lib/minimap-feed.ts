import type { Renderer } from "../pkg/ogar_web";
import { cssRgb } from "./format";
import type { UnitEntry } from "./game.svelte";

export interface MapView {
    ex: number;
    ey: number;
    x0: number;
    y0: number;
    x1: number;
    y1: number;
}

type Input = "place" | "style" | "units" | "view" | "cells";
type Push = (r: Renderer) => void;

const UNIT_STRIDE = 5;

export class MinimapFeed {
    private readonly latest = new Map<Input, Push>();
    private readonly dirty = new Set<Input>();
    private target: Renderer | null = null;

    private set(input: Input, push: Push): void {
        this.latest.set(input, push);
        this.dirty.add(input);
    }

    place(left: number, top: number, side: number): void {
        this.set("place", (r) => r.setMinimapPlace(left, top, side));
    }

    clear(): void {
        this.set("place", (r) => r.clearMinimap());
    }

    style(text: number, shrink: number, family: string, watching: boolean, liveView: boolean): void {
        this.set("style", (r) => r.setMinimapStyle(text, shrink, family, watching, liveView));
    }

    units(units: UnitEntry[]): void {
        const nums = new Float64Array(units.length * UNIT_STRIDE);
        units.forEach((u, i) => nums.set([u.id, u.spot.x, u.spot.y, u.spot.r, cssRgb(u.color)], i * UNIT_STRIDE));
        const names = units.map((u) => u.name);
        this.set("units", (r) => r.setMinimapPlayers(nums, names));
    }

    view(v: MapView): void {
        const view = Float32Array.of(v.ex, v.ey, v.x0, v.y0, v.x1, v.y1);
        this.set("view", (r) => r.setMinimapView(view));
    }

    cells(cells: Float64Array): void {
        this.set("cells", (r) => r.setMinimapCells(cells));
    }

    apply(r: Renderer): void {
        if (r !== this.target) {
            this.target = r;
            for (const input of this.latest.keys()) this.dirty.add(input);
        }
        for (const input of this.dirty) this.latest.get(input)?.(r);
        this.dirty.clear();
    }
}
