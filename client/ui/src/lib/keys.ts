const KEY_NAMES: Record<number, string> = {
    32: "Space",
    13: "Enter",
    9: "Tab",
    37: "Left",
    38: "Up",
    39: "Right",
    40: "Down",
    96: "Num0",
    97: "Num1",
    98: "Num2",
    99: "Num3",
    100: "Num4",
    101: "Num5",
    102: "Num6",
    103: "Num7",
    104: "Num8",
    105: "Num9",
    106: "*",
    107: "+",
    109: "-",
    110: ".",
    111: "/",
    112: "F1",
    113: "F2",
    114: "F3",
    115: "F4",
    116: "F5",
    117: "F6",
    118: "F7",
    119: "F8",
    120: "F9",
    121: "F10",
    122: "F11",
    123: "F12",
    186: ":",
    187: ";",
    188: ",",
    189: "-",
    190: ".",
    191: "/",
    192: "@",
    219: "[",
    220: "\\",
    221: "]",
    222: "^",
    226: "\\",
};

const MOD = { Shift: 256, Ctrl: 512, Alt: 1024 } as const;
export const CLEAR = -1;
export const MOUSE_LEFT = 0;
export const MOUSE_RIGHT = 2;
export const NAV_BUTTONS: ReadonlySet<number> = new Set([3, 4]);
const KEY_MASK = 0xff;
const MODIFIER_ONLY = new Set([16, 17, 18, 229]);
const CLEAR_KEYS = new Set([8, 46]);

export function hotkeyText(code: number): string {
    if (code <= 0) return "";
    const key = code & KEY_MASK;
    const mods = code - key;
    const prefix = mods & MOD.Alt ? "alt+" : mods & MOD.Ctrl ? "ctl+" : mods & MOD.Shift ? "sft+" : "";
    return prefix + (KEY_NAMES[key] ?? String.fromCharCode(key));
}

export function eventCode(e: KeyboardEvent): number {
    return e.keyCode + (e.ctrlKey ? MOD.Ctrl : 0) + (e.shiftKey ? MOD.Shift : 0) + (e.altKey ? MOD.Alt : 0);
}

export function captureCode(e: KeyboardEvent): number | null {
    if (e.repeat || MODIFIER_ONLY.has(e.keyCode)) return null;
    if (CLEAR_KEYS.has(e.keyCode)) return CLEAR;
    return eventCode(e);
}

export function typingTarget(e: Event): boolean {
    return (
        e.target instanceof HTMLInputElement ||
        e.target instanceof HTMLTextAreaElement ||
        e.target instanceof HTMLSelectElement
    );
}

const MOUSE_NAMES = ["mouseLeft", "mouseMiddle", "mouseRight", "mouseSide2", "mouseSide1"];

export function mouseCode(e: MouseEvent): number {
    return e.button + (e.ctrlKey ? MOD.Ctrl : 0) + (e.shiftKey ? MOD.Shift : 0) + (e.altKey ? MOD.Alt : 0);
}

export function mouseText(code: number, t: (key: string) => string): string {
    if (code < 0) return "";
    const button = code & KEY_MASK;
    const mods = code - button;
    const prefix = mods & MOD.Alt ? "alt+" : mods & MOD.Ctrl ? "ctl+" : mods & MOD.Shift ? "sft+" : "";
    return prefix + t(MOUSE_NAMES[button] ?? `mouse${button}`);
}
