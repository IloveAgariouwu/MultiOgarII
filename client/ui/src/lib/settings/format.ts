import type { RangeSpec, SettingValue, SettingsMap, UiSchema } from "./types";

const LEVELS = ["lbtLevelLow", "lbtLevelMid", "lbtLevelHigh", "lbtLevelCustom"];
const INTERPOLATION_SPEED_BASE = -50;
export const PERCENT = 100;
const SOFTEN_ANIMATION_BASE = 80;
const SOFTEN_ANIMATION_SCALE = 2;

export function rangeVisible(spec: RangeSpec, settings: SettingsMap): boolean {
    if (!spec.when) return true;
    const v = Number(settings[spec.when.key]);
    if (spec.when.lt !== undefined) return v < spec.when.lt;
    return v === spec.when.eq;
}

export function isChoice(spec: RangeSpec): boolean {
    const show = spec.show ?? "";
    return show.startsWith("pick:") || show === "level3" || show === "level4";
}

export function rangeText(spec: RangeSpec, value: SettingValue, ui: UiSchema, t: (k: string) => string): string {
    const v = Number(value);
    const unit = spec.unit === "sec" ? t("unitSec") : (spec.unit ?? "");
    const show = spec.show ?? "";
    if (show.startsWith("pick:")) return `${t(String(ui.pickSources[show.slice(5)][v] ?? ""))}${unit}`;
    switch (show) {
        case "percent":
            return `${rangeShown(spec, v)}%`;
        case "level3":
        case "level4":
            return t(LEVELS[v]);
        case "interpolationSpeed":
            return String(rangeShown(spec, v));
        case "softenAnimation":
            return `${rangeShown(spec, v)}${unit}`;
        default:
            return `${v}${unit ? ` ${unit}` : ""}`;
    }
}

export function rangeShown(spec: RangeSpec, v: number): number {
    switch (spec.show) {
        case "percent":
            return Math.round(v * PERCENT);
        case "interpolationSpeed":
            return Math.round(INTERPOLATION_SPEED_BASE + PERCENT * v);
        case "softenAnimation":
            return (v - SOFTEN_ANIMATION_BASE) / SOFTEN_ANIMATION_SCALE;
        default:
            return v;
    }
}

function rangeRaw(spec: RangeSpec, shown: number): number {
    switch (spec.show) {
        case "percent":
            return shown / PERCENT;
        case "interpolationSpeed":
            return (shown - INTERPOLATION_SPEED_BASE) / PERCENT;
        case "softenAnimation":
            return shown * SOFTEN_ANIMATION_SCALE + SOFTEN_ANIMATION_BASE;
        default:
            return shown;
    }
}

function clampValue(v: number, min: number, max: number, integer: boolean): number {
    return Math.min(max, Math.max(min, integer ? Math.round(v) : +v.toFixed(6)));
}

export function rangeParse(spec: RangeSpec, shown: number): number {
    return clampValue(rangeRaw(spec, shown), spec.min, spec.max, Number.isInteger(spec.step));
}

export function cursorImageUri(raw: string): string {
    try {
        const url = new URL(raw);
        const allowed = url.protocol === "https:" || url.protocol === "http:" || raw.startsWith("data:image/");
        return allowed ? url.href : "";
    } catch {
        return "";
    }
}
