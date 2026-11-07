export type SettingValue = boolean | number | string;
export type SettingsMap = Record<string, SettingValue>;
type HotkeyMap = Record<string, number>;

export interface Hotkeys {
    control: HotkeyMap;
    toggle: HotkeyMap;
    hold: HotkeyMap;
    mouse: HotkeyMap;
}

export const HotkeyKind = { Control: 0, Toggle: 1, Hold: 2, Mouse: 3 } as const;
export type HotkeyKind = (typeof HotkeyKind)[keyof typeof HotkeyKind];

export interface Profile {
    name: string;
    skinUrl: string;
    selfSkinUrl: string;
    selfSkinUrl2: string;
    selfTabSkinUrl: string;
    selfTabSkinUrl2: string;
    skinUrl2: string;
    tabName: string;
    tabSkinUrl: string;
    tabSkinUrl2: string;
    profileIndex: number;
    label: string;
    used: boolean;
    displayName: string;
}

export type ProfileField = Exclude<keyof Profile, "profileIndex" | "used" | "displayName">;

export interface ProfileState {
    entries: Profile[];
    main: number;
    second: number;
    linked: boolean;
}

export interface StoredSelection {
    selected: string;
    selectedExtra: string | null;
    selectedTab2: string;
}

interface StoredProfiles extends StoredSelection {
    entries: string;
    extra: string | null;
}

export type StoredProfileKeys = Record<keyof StoredProfiles, string | null>;

export interface ProfileChanges extends StoredSelection {
    entries?: string;
    extra?: string | null;
}

export type ProfileSelection = Omit<ProfileState, "entries">;

export interface RangeSpec {
    key: string;
    label: string;
    min: number;
    max: number;
    step: number;
    show?: string;
    unit?: string;
    when?: { key: string; lt?: number; eq?: number };
}

export interface TextSpec {
    key: string;
    label: string;
    hint?: string;
    placeholder?: string;
    maxLength?: number;
}

export interface GroupSpec {
    kind: "group";
    key?: string;
    label?: string;
    keys: string[];
    labels?: string[];
    exclusive?: string[];
}

export type SettingItem =
    | string
    | ({ kind: "range" } & RangeSpec)
    | ({ kind: "text" } & TextSpec)
    | { kind: "slot"; key: string }
    | { kind: "flag"; key: string; label: string }
    | { kind: "font"; key: string; label: string }
    | GroupSpec;

export const STAGE_FOCUSES = [
    "scene",
    "virus",
    "autosplit",
    "border",
    "jelly",
    "marker",
    "grid",
    "field",
    "second",
] as const;
export type StageFocus = (typeof STAGE_FOCUSES)[number];
export type HudPart = "map" | "rail" | "self" | "status" | "client" | "server" | "chat" | "mention" | "replay";
export const HUD_PREVIEWS = ["hud", "map", "rail", "self", "client", "server", "chat", "mention", "replay"] as const;
export type HudPreview = (typeof HUD_PREVIEWS)[number];
const PREVIEW_KINDS = ["none", ...STAGE_FOCUSES, ...HUD_PREVIEWS] as const;
export type PreviewKind = (typeof PREVIEW_KINDS)[number];

export function stageFocus(kind: PreviewKind): kind is StageFocus {
    return STAGE_FOCUSES.some((k) => k === kind);
}

export function previewKind(preview: string): PreviewKind {
    const kind = PREVIEW_KINDS.find((k) => k === preview);
    if (!kind) throw new Error(`unknown preview ${preview}`);
    return kind;
}

export interface CategorySpec {
    id: string;
    title: string;
    preview: string;
    items: SettingItem[];
    shared?: string[];
}

export type BrowserFeature = "touch";

export interface UiSchema {
    categories: CategorySpec[];
    previews: Record<string, string>;
    previewOverrides: Record<string, SettingsMap>;
    requires: Record<string, string>;
    browserFeatures: Record<string, BrowserFeature>;
    pickSources: Record<string, (string | number)[]>;
    reloadRequired: string[];
    uiActions: string[];
}

export type Groups = Record<string, string[]>;

export interface ServerChoice {
    id: string;
    label: string;
    addr: string;
}

export interface Population {
    pop: number;
    max: number;
}

export interface CoreLimits {
    macroMaxDelayMs: number;
    macroMaxRepeat: number;
    macroNameMax: number;
    macroMaxSteps: number;
    macroMaxCount: number;
    profileNameMax: number;
    profileLabelMax: number;
    themeNameMax: number;
    themeMaxCount: number;
}

export interface ThemeBook {
    names: string[];
    selected: string | null;
}

export interface ThemeImport {
    ok: boolean;
    error: string;
    issues: string[];
}

export interface MinimapLayout {
    stride: number;
    x: number;
    y: number;
    radius: number;
    rgb: number;
    flags: number;
    owner: number;
    own: number;
}
