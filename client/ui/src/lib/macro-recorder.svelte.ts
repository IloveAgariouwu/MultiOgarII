import type { MacroDef, MacroPhase } from "./game.svelte";

export class MacroRecorder {
    recording = $state<MacroDef | null>(null);
    draft = $state<MacroDef | null>(null);
    index = $state(-1);
    private last = 0;

    constructor(private readonly maxDelayMs: number) {}

    edit(index: number, macro: MacroDef): void {
        this.index = index;
        this.draft = $state.snapshot(macro);
    }

    close(): void {
        this.recording = null;
        this.draft = null;
        this.index = -1;
    }

    start(): void {
        if (!this.draft) return;
        this.draft.steps = [];
        this.last = 0;
        this.recording = this.draft;
    }

    record(action: string, phase: MacroPhase): void {
        if (!this.recording) return;
        const now = performance.now();
        const delayMs = this.last ? Math.min(this.maxDelayMs, Math.round(now - this.last)) : 0;
        this.last = now;
        this.recording.steps.push({ action, phase, delayMs });
    }

    stop(): void {
        this.recording = null;
    }
}
