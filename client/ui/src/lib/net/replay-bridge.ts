import { Replays, World, type Net } from "../../pkg/ogar_web";
import type { ReplayCamera, ReplayImport, ReplayReel } from "../game.svelte";
import { fromJson } from "../json";
import type { ReplayBackend } from "../replay.svelte";
import type { HudUpdate } from "./hud";

export class ReplayBridge implements ReplayBackend {
    readonly world = new World();
    readonly recorder = new Replays();
    private replaying = false;
    private held: string[] = [];
    private sightFrames = 0;

    constructor(
        private readonly apply: (update: HudUpdate, net: Net | null) => void,
        private readonly live: () => Net | null,
    ) {}

    get playing(): boolean {
        return this.replaying;
    }

    hold(text: string): void {
        this.held.push(text);
    }

    dropHeld(): void {
        this.held = [];
    }

    recordSight(world: World, x: number, y: number, scale: number, aimed: [number, number]): void {
        if (this.replaying || this.sightFrames++ % 2 !== 0) return;
        this.recorder.recordSight(world, new Float32Array([x, y, scale]), new Float32Array(aimed), performance.now());
    }

    replayCapture(): ReplayReel | null {
        const reel = this.recorder.captureWindow(performance.now());
        return reel === undefined ? null : fromJson(reel);
    }

    replayToggleRecording(): { recording: boolean; reel: ReplayReel | null } {
        const reel = this.recorder.toggleRecording();
        return { recording: this.recorder.isRecording(), reel: reel === undefined ? null : fromJson(reel) };
    }

    replayDelete(id: number): void {
        this.recorder.deleteReel(id);
    }

    replayReset(): void {
        this.replayEnd();
        this.recorder.clearLibrary();
    }

    replayExport(id: number): Uint8Array | null {
        return this.recorder.exportReel(id) ?? null;
    }

    replayTrim(id: number, from: number, to: number): Uint8Array | null {
        try {
            return this.recorder.exportTrimmedReel(id, from, to);
        } catch (e) {
            console.error(`replay ${id} was not trimmed: ${String(e)}`);
            return null;
        }
    }

    replayImport(bytes: Uint8Array): ReplayImport {
        try {
            return { ok: true, reel: fromJson(this.recorder.importReel(bytes)) };
        } catch (e) {
            return { ok: false, error: String(e) };
        }
    }

    replaySeek(id: number, positionMs: number): void {
        const error = this.recorder.seek(id, positionMs, this.world, performance.now());
        if (error) console.warn(`replay decode failed, ${error}`);
        this.replaying = true;
        const text = this.recorder.takeHud();
        if (text) this.apply(fromJson(text), null);
    }

    replayPrepare(id: number, stepMs: number): boolean {
        try {
            return this.recorder.scoutAhead(id, stepMs, performance.now());
        } catch (e) {
            console.warn(`replay prepare failed, ${String(e)}`);
            return true;
        }
    }

    replayEnd(): void {
        if (!this.replaying) return;
        this.replaying = false;
        this.recorder.stopPlayback(this.world);
        const net = this.live();
        if (net) for (const text of this.held) this.apply(fromJson(text), net);
        this.held = [];
    }

    replayCamera(id: number, positionMs: number): ReplayCamera | null {
        const camera = this.recorder.cameraAt(id, positionMs);
        if (!camera) return null;
        const [eyeX, eyeY, scale, aimX, aimY, splitting, aimPlayer] = camera;
        return { eyeX, eyeY, scale, aimX, aimY, splitting: splitting === 1, aimPlayer };
    }

    exportSeek(id: number, positionMs: number, world: World, now: number): void {
        const error = this.recorder.seekForExport(id, positionMs, world, now);
        if (error) console.warn(`replay decode failed, ${error}`);
    }

    exportEnd(): void {
        this.recorder.endExport();
    }
}
