import type { World } from "../pkg/ogar_web";
import type { GameSource, MacroDef } from "./game.svelte";
import type { ServerChoice } from "./types";

export class PreviewSource implements GameSource {
    constructor(readonly world: World) {}

    start(): void {}
    connect(_server: ServerChoice): void {}
    play(_tab?: number): void {}
    spectate(): void {}
    watch(_free: boolean): void {}
    say(_text: string): void {}
    refreshTab(_tab: 0 | 1): void {}
    closeTab(_tab: 0 | 1): void {}
    mouse(_button: number, _down: boolean): void {}
    recordCamera(_x: number, _y: number, _scale: number): void {}
    setTabsAsUnits(_on: boolean): void {}
    runMacro(_macro: MacroDef): void {}
    stopMacros(): void {}
    act(_action: string): void {}
    release(_action: string): void {}
    frame(_now: number, _toWorld: (x: number, y: number) => Float32Array): void {}
    pointer(_x: number, _y: number): void {}
}
