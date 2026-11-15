import { cubicOut } from "svelte/easing";
import type { TransitionConfig } from "svelte/transition";

export const PANEL_IN = { y: 16, duration: 300, easing: cubicOut };
export const PANEL_OUT = { duration: 140 };
export const MESSAGE_IN = { x: -8, duration: 220 };
export const FROST_IN = { duration: 200 };

const FROST_BLUR_PX = 14;
const FROST_SATURATE = 0.15;

export function frostFilter(t: number): string {
    return `blur(${FROST_BLUR_PX * t}px) saturate(${1 + FROST_SATURATE * t})`;
}

export function frost(_node: Element, { duration }: { duration: number }): TransitionConfig {
    return { duration, easing: cubicOut, css: (t) => `opacity: ${t}; backdrop-filter: ${frostFilter(t)};` };
}
