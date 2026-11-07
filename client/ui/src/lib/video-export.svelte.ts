import {
    BufferTarget,
    CanvasSource,
    Mp4OutputFormat,
    Output,
    QUALITY_HIGH,
    QUALITY_LOW,
    QUALITY_MEDIUM,
    QUALITY_VERY_HIGH,
    QUALITY_VERY_LOW,
} from "mediabunny";
import { untrack } from "svelte";
import { Renderer, World } from "../pkg/ogar_web";
import type { ReplayEntry } from "./replay.svelte";
import type { AppState } from "./state.svelte";

const QUALITIES = [QUALITY_VERY_LOW, QUALITY_LOW, QUALITY_MEDIUM, QUALITY_HIGH, QUALITY_VERY_HIGH];

let queue: Promise<unknown> = Promise.resolve();

function runExclusive<T>(task: () => Promise<T>): Promise<T> {
    const run = queue.then(task);
    queue = run.catch(() => undefined);
    return run;
}

class ReelCanvas {
    private constructor(
        readonly canvas: HTMLCanvasElement,
        private renderer: Renderer,
        private world: World,
        private app: AppState,
    ) {}

    static async create(app: AppState, width: number, height: number, still: boolean): Promise<ReelCanvas> {
        const canvas = document.createElement("canvas");
        Object.assign(canvas.style, { position: "fixed", left: "-99999px", top: "0", width: "10px", height: "10px" });
        document.body.append(canvas);
        const renderer = await Renderer.create(canvas, Number(app.settings.GpuCellRenderer));
        renderer.setFixedSize(width, height);
        renderer.setFixedWorkMs(0);
        if (still) {
            renderer.setFit(150);
            renderer.setOverlays(false);
        } else renderer.setZoom((0.5 * height) / 1080);
        return new ReelCanvas(canvas, renderer, new World(), app);
    }

    async frame(id: number, ms: number, base: number, settle: boolean): Promise<void> {
        const draw = () => {
            const shot = this.app.replay.exportSeek(id, ms, this.world, base + ms);
            if (shot)
                this.renderer.setReplayCamera(
                    shot.eyeX,
                    shot.eyeY,
                    this.app.flag("AffectZoomingOnReplay") ? shot.scale : undefined,
                );
            else this.renderer.clearReplayCamera();
            this.renderer.prepareFrame(base + ms, this.world, this.app.core.wasm);
            this.renderer.draw(this.world);
        };
        draw();
        const deadline = performance.now() + 3000;
        while (settle && this.renderer.pendingAssets() > 0 && performance.now() < deadline) {
            await new Promise((resolve) => setTimeout(resolve, 50));
            draw();
        }
    }

    free(): void {
        this.app.replay.exportEnd();
        this.renderer.free();
        this.world.free();
        this.canvas.remove();
    }
}

export class VideoExport {
    progress = $state<number | null>(null);
    shown = $state(0);
    private stop = new AbortController();

    readonly show = () => {
        untrack(() => this.shown++);
        return () => untrack(() => void this.shown--);
    };

    constructor(private app: AppState) {}

    get format(): { width: number; height: number; fps: number } {
        const height = Number(this.app.ui.pickSources.videoHeight[this.app.num("VideoExportHeight")]);
        return {
            width: Math.round(height * (16 / 9)),
            height,
            fps: Number(this.app.ui.pickSources.videoFps[this.app.num("VideoExportFps")]),
        };
    }

    get label(): string {
        const f = this.format;
        return this.app.t("replayVideo").replace("%s", `${f.width}×${f.height} ${f.fps}fps MP4`);
    }

    get active(): boolean {
        return this.progress !== null;
    }

    cancel(): void {
        this.stop.abort();
    }

    record(reel: ReplayEntry): Promise<Blob | null> {
        if (this.active) return Promise.resolve(null);
        this.progress = 0;
        const stop = new AbortController();
        this.stop = stop;
        const replay = this.app.replay;
        return runExclusive(async () => {
            const pin = await replay.pin(reel);
            if (!pin) return null;
            try {
                const signal = AbortSignal.any([stop.signal, pin.signal]);
                const { width, height, fps } = this.format;
                const clip = reel.clip ?? { from: 0, to: reel.duration };
                const target = await ReelCanvas.create(this.app, width, height, false);
                try {
                    const base = performance.now();
                    await target.frame(pin.id, clip.from, base, true);
                    const video = new CanvasSource(target.canvas, {
                        codec: "avc",
                        quality: QUALITIES[this.app.num("VideoExportQuality")],
                        hardwareAcceleration: "prefer-hardware",
                    });
                    const output = new Output({ format: new Mp4OutputFormat(), target: new BufferTarget() });
                    output.addVideoTrack(video, { frameRate: fps });
                    await output.start();
                    const frames = Math.max(1, Math.round(((clip.to - clip.from) * fps) / 1000));
                    for (let k = 0; k < frames && !signal.aborted; k++) {
                        await target.frame(pin.id, clip.from + (k * 1000) / fps, base, false);
                        await video.add(k / fps, 1 / fps);
                        this.progress = (k + 1) / frames;
                        if (k % 30 === 0) await new Promise((resolve) => setTimeout(resolve));
                    }
                    if (signal.aborted) await output.cancel();
                    else await output.finalize();
                    const buffer = output.target.buffer;
                    return signal.aborted || !buffer ? null : new Blob([buffer], { type: "video/mp4" });
                } finally {
                    target.free();
                }
            } finally {
                pin.release();
            }
        }).finally(() => (this.progress = null));
    }
}

export class ReplayThumbs {
    constructor(private app: AppState) {}

    render(reels: ReplayEntry[]): Promise<void> {
        const replay = this.app.replay;
        return runExclusive(async () => {
            const missing = reels.filter((r) => !r.thumb);
            if (!missing.length) return;
            const target = await ReelCanvas.create(this.app, 320, 180, true);
            try {
                for (const reel of missing) {
                    const pin = await replay.pin(reel);
                    if (!pin) continue;
                    try {
                        await target.frame(pin.id, reel.duration / 2, performance.now(), true);
                        const blob = await new Promise<Blob | null>((resolve) =>
                            target.canvas.toBlob(resolve, "image/webp"),
                        );
                        replay.exportEnd();
                        if (blob) replay.attachThumb(reel, blob);
                    } finally {
                        pin.release();
                    }
                }
            } finally {
                target.free();
            }
        });
    }
}
