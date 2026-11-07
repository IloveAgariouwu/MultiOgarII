import type { World } from "../pkg/ogar_web";
import type { ReplayCamera, ReplayImport, ReplayReel } from "./game.svelte";
import {
    clearReels,
    deleteReel,
    isStorageFull,
    loadMeta,
    putMeta,
    readReel,
    reelsWithoutMeta,
    saveReel,
    setThumb,
    storagePersisted,
    storageUsage,
    type ReelMeta,
} from "./replay-store";

const MAX_FRAME_MS = 100;
export const STORE_LOW_MS = 10 * 60 * 1000;
const PREPARE_STEP_MS = 500;
const PREPARE_BUDGET_MS = 4;

export interface ReplayClip {
    from: number;
    to: number;
}

export interface ReplayThumb {
    blob: Blob;
    url: string;
}

export type ReplayEntry = {
    key: number;
    duration: number;
    origin: ReplayReel["origin"];
    thumb: ReplayThumb | null;
    clip: ReplayClip | null;
} & ({ stored: null; id: number; size: null } | { stored: number; id: number | null; size: number });

export interface ReelPin {
    id: number;
    signal: AbortSignal;
    release(): void;
}

export interface ReplayBackend {
    replayCapture(): ReplayReel | null;
    replayToggleRecording(): { recording: boolean; reel: ReplayReel | null };
    replayDelete(id: number): void;
    replayReset(): void;
    replaySeek(id: number, positionMs: number): void;
    replayPrepare(id: number, stepMs: number): boolean;
    replayEnd(): void;
    replayExport(id: number): Uint8Array | null;
    replayTrim(id: number, from: number, to: number): Uint8Array | null;
    replayImport(bytes: Uint8Array): ReplayImport;
    exportSeek(id: number, positionMs: number, world: World, now: number): void;
    exportEnd(): void;
    replayCamera(id: number, positionMs: number): ReplayCamera | null;
}

export class ReplayState {
    reels = $state<ReplayEntry[]>([]);
    current = $state(0);
    position = $state(0);
    recording = $state(false);
    playing = $state(false);
    playback = $state(false);
    speed = $state(1);
    cont = $state(false);
    captured = $state(false);
    usage = $state<{ used: number; quota: number } | null>(null);
    persisted = $state<boolean | null>(null);
    private frame = 0;
    private run = 0;
    private preparing = 0;
    private prepared: number | null = null;
    private focusing: ReplayEntry | null = null;
    private generation = 0;
    private nextKey = 1;
    private loading = new Map<number, Promise<number | null>>();
    private pins = new Set<{ key: number; abort: AbortController }>();
    private backend!: ReplayBackend;

    constructor(
        private onMode: (replaying: boolean) => void,
        private onStoreFull: () => void,
        private onStoreLow: (recordableMs: number) => void,
    ) {}

    bind(backend: ReplayBackend): void {
        this.backend = backend;
        this.restore();
    }

    private async restore(): Promise<void> {
        const generation = this.generation;
        const metas = await loadMeta();
        for (const stored of await reelsWithoutMeta()) {
            const bytes = await readReel(stored);
            if (generation !== this.generation) return;
            if (!bytes) {
                console.error(`stored replay ${stored} is missing`);
                deleteReel(stored);
                continue;
            }
            const result = this.backend.replayImport(bytes);
            if (!result.ok) {
                console.error(`stored replay ${stored} is unreadable: ${result.error}`);
                deleteReel(stored);
                continue;
            }
            this.backend.replayDelete(result.reel.id);
            const meta: ReelMeta = {
                id: stored,
                duration: result.reel.duration,
                origin: result.reel.origin,
                size: bytes.length,
                thumb: null,
            };
            await putMeta(meta);
            metas.push(meta);
        }
        if (generation !== this.generation) return;
        const listed = metas
            .sort((a, b) => a.id - b.id)
            .map((m): ReplayEntry => ({
                key: this.nextKey++,
                duration: m.duration,
                origin: m.origin,
                thumb: m.thumb && { blob: m.thumb, url: URL.createObjectURL(m.thumb) },
                clip: null,
                stored: m.id,
                id: null,
                size: m.size,
            }));
        if (this.reels.length) this.current += listed.length;
        this.reels = [...listed, ...this.reels];
        this.measure();
    }

    private measure(): void {
        storageUsage().then((usage) => (this.usage = usage));
        storagePersisted().then((persisted) => (this.persisted = persisted));
    }

    get storedBytes(): number {
        let bytes = 0;
        for (const reel of this.reels) if (reel.size !== null) bytes += reel.size;
        return bytes;
    }

    get recordableMs(): number | null {
        let bytes = 0,
            ms = 0;
        for (const reel of this.reels) {
            if (reel.size === null) continue;
            bytes += reel.size;
            ms += reel.duration;
        }
        return this.usage && bytes > 0 ? ((this.usage.quota - this.usage.used) * ms) / bytes : null;
    }

    private load(entry: ReplayEntry): Promise<number | null> {
        if (!this.reels.includes(entry)) return Promise.resolve(null);
        if (entry.stored === null || entry.id !== null) return Promise.resolve(entry.id);
        const stored = entry.stored;
        const pending = this.loading.get(entry.key);
        if (pending) return pending;
        const loaded = readReel(stored)
            .then(
                (bytes): ReplayImport => (bytes ? this.backend.replayImport(bytes) : { ok: false, error: "missing" }),
                (e: unknown): ReplayImport => ({ ok: false, error: String(e) }),
            )
            .then((result) => {
                if (!result.ok) {
                    console.error(`stored replay ${stored} is unreadable: ${result.error}`);
                    return null;
                }
                if (!this.reels.includes(entry)) {
                    this.backend.replayDelete(result.reel.id);
                    return null;
                }
                entry.id = result.reel.id;
                return result.reel.id;
            })
            .finally(() => this.loading.delete(entry.key));
        this.loading.set(entry.key, loaded);
        return loaded;
    }

    private unload(entry: ReplayEntry): void {
        if (entry.id === null || [...this.pins].some((p) => p.key === entry.key)) return;
        if (!this.reels.includes(entry)) return this.backend.replayDelete(entry.id);
        if (entry.stored === null || entry === this.reel) return;
        this.backend.replayDelete(entry.id);
        entry.id = null;
    }

    async pin(entry: ReplayEntry): Promise<ReelPin | null> {
        const pin = { key: entry.key, abort: new AbortController() };
        this.pins.add(pin);
        const id = await this.load(entry);
        if (id === null) {
            this.pins.delete(pin);
            return null;
        }
        return {
            id,
            signal: pin.abort.signal,
            release: () => {
                if (this.pins.delete(pin)) this.unload(entry);
            },
        };
    }

    private async focus(index: number): Promise<{ reel: ReplayEntry; id: number } | null> {
        const entry = this.reels[index];
        if (!entry) return null;
        if (entry === this.reel && entry.id !== null) return { reel: entry, id: entry.id };
        this.focusing = entry;
        const id = await this.load(entry);
        if (id === null) return null;
        if (this.focusing !== entry) {
            this.unload(entry);
            return null;
        }
        const previous = this.reels[this.current];
        this.current = this.reels.indexOf(entry);
        if (previous !== entry) this.unload(previous);
        return { reel: entry, id };
    }

    private show(entry: ReplayEntry): void {
        this.focusing = entry;
        const previous = this.reels[this.current];
        this.current = this.reels.indexOf(entry);
        if (previous !== entry) this.unload(previous);
        this.moveTo(0);
    }

    private live(reel: ReplayReel): ReplayEntry {
        this.reels.push({
            key: this.nextKey++,
            duration: reel.duration,
            origin: reel.origin,
            thumb: null,
            clip: null,
            stored: null,
            id: reel.id,
            size: null,
        });
        return this.reels[this.reels.length - 1];
    }

    async exportReel(entry = this.reel): Promise<Uint8Array | null> {
        if (!entry) return null;
        if (entry.stored === null) return this.backend.replayExport(entry.id);
        const stored = entry.stored;
        return readReel(stored).catch((e: unknown) => {
            console.error(`stored replay ${stored} is unreadable: ${String(e)}`);
            return null;
        });
    }

    importFile(bytes: Uint8Array): string {
        const result = this.backend.replayImport(bytes);
        if (!result.ok) return result.error;
        const entry = this.live(result.reel);
        this.show(entry);
        this.store(entry, bytes);
        return "";
    }

    private persist(entry: ReplayEntry, id: number): void {
        const bytes = this.backend.replayExport(id);
        if (bytes) this.store(entry, bytes);
    }

    private store(entry: ReplayEntry, bytes: Uint8Array): void {
        const duration = entry.duration;
        saveReel(bytes, { duration, origin: entry.origin }).then(
            (stored) => {
                if (!this.reels.includes(entry) || entry.duration !== duration)
                    return void deleteReel(stored).then(() => this.measure());
                entry.stored = stored;
                entry.size = bytes.length;
                this.unload(entry);
                this.measure();
                if (entry.thumb)
                    setThumb(stored, entry.thumb.blob).then(
                        () => this.measure(),
                        (e: unknown) => console.error(`replay ${entry.key} thumbnail was not stored: ${String(e)}`),
                    );
            },
            (e: unknown) => {
                if (isStorageFull(e)) this.onStoreFull();
                else console.error(`replay ${entry.key} was not stored: ${String(e)}`);
            },
        );
    }

    attachThumb(entry: ReplayEntry, blob: Blob): void {
        if (!this.reels.includes(entry)) return;
        entry.thumb = { blob, url: URL.createObjectURL(blob) };
        if (entry.stored !== null)
            setThumb(entry.stored, blob).then(
                () => this.measure(),
                (e: unknown) => console.error(`replay ${entry.key} thumbnail was not stored: ${String(e)}`),
            );
    }

    exportSeek(id: number, positionMs: number, world: World, now: number): ReplayCamera | null {
        this.backend.exportSeek(id, positionMs, world, now);
        return this.backend.replayCamera(id, positionMs);
    }

    exportEnd(): void {
        this.backend.exportEnd();
    }

    camera(): ReplayCamera | null {
        const reel = this.reel;
        return reel && reel.id !== null ? this.backend.replayCamera(reel.id, this.position) : null;
    }

    async open(key: number): Promise<boolean> {
        if (!(await this.focus(this.reels.findIndex((r) => r.key === key)))) return false;
        this.enter();
        this.moveTo(0);
        return true;
    }

    get reel(): ReplayEntry | undefined {
        return this.reels[this.current];
    }

    markIn(): void {
        const reel = this.reel;
        if (!reel) return;
        const to = reel.clip?.to ?? reel.duration;
        reel.clip = { from: this.position, to: to > this.position ? to : reel.duration };
    }

    markOut(): void {
        const reel = this.reel;
        if (!reel) return;
        const from = reel.clip?.from ?? 0;
        reel.clip = { from: from < this.position ? from : 0, to: this.position };
    }

    clearClip(): void {
        if (this.reel) this.reel.clip = null;
    }

    async keepClip(): Promise<void> {
        const clip = this.reel?.clip;
        if (!clip) return;
        const shown = await this.focus(this.current);
        if (!shown) return;
        const entry = shown.reel;
        const bytes = this.backend.replayTrim(shown.id, clip.from, clip.to);
        if (!bytes) return;
        const trimmed = this.backend.replayImport(bytes);
        if (!trimmed.ok) return console.error(`trimmed replay ${entry.key} is unreadable: ${trimmed.error}`);
        const { id, duration } = trimmed.reel;
        const stored = await saveReel(bytes, { duration, origin: entry.origin }).catch((e: unknown) => {
            if (isStorageFull(e)) this.onStoreFull();
            else console.error(`trimmed replay ${entry.key} was not stored: ${String(e)}`);
            return null;
        });
        if (stored === null || !this.reels.includes(entry)) {
            this.backend.replayDelete(id);
            if (stored !== null) deleteReel(stored).then(() => this.measure());
            return;
        }
        for (const pin of this.pins) if (pin.key === entry.key) pin.abort.abort();
        const { id: previous, stored: previousStored, thumb } = entry;
        if (thumb) URL.revokeObjectURL(thumb.url);
        Object.assign(entry, { id, stored, duration, size: bytes.length, clip: null, thumb: null });
        if (previous !== null) this.backend.replayDelete(previous);
        if (previousStored !== null) deleteReel(previousStored).then(() => this.measure());
        else this.measure();
        if (entry !== this.reel) return;
        this.prepared = null;
        this.moveTo(this.position - clip.from);
    }

    get progress(): number {
        return this.reel ? this.position / this.reel.duration : 0;
    }

    capture(): void {
        const reel = this.backend.replayCapture();
        if (reel === null) return;
        const entry = this.live(reel);
        this.show(entry);
        this.persist(entry, reel.id);
        this.captured = true;
        setTimeout(() => (this.captured = false), 500);
    }

    toggleRecording(): void {
        const { recording, reel } = this.backend.replayToggleRecording();
        this.recording = recording;
        const recordable = this.recordableMs;
        if (recording && recordable !== null && recordable < STORE_LOW_MS) this.onStoreLow(recordable);
        if (reel === null) return;
        const entry = this.live(reel);
        this.show(entry);
        this.persist(entry, reel.id);
    }

    private setPlayback(on: boolean): void {
        if (this.playback === on) return;
        this.playback = on;
        this.onMode(on);
        if (!on) this.prepared = null;
        this.keepPreparing();
    }

    private keepPreparing(): void {
        const reel = this.reel;
        if (!this.playback || !reel || reel.id === null || reel.id === this.prepared || this.preparing) return;
        this.prepared = null;
        this.preparing = requestAnimationFrame(this.prepare);
    }

    private prepare = (): void => {
        this.preparing = 0;
        const id = this.reel?.id;
        if (!this.playback || id === undefined || id === null) return;
        const until = performance.now() + PREPARE_BUDGET_MS;
        while (performance.now() < until)
            if (this.backend.replayPrepare(id, PREPARE_STEP_MS)) return void (this.prepared = id);
        this.keepPreparing();
    };

    private halt(): void {
        this.run++;
        cancelAnimationFrame(this.frame);
    }

    private tick(): void {
        this.halt();
        if (!this.playing) return;
        const run = this.run;
        let last = performance.now();
        const step = (at: number) => {
            if (run !== this.run) return;
            const elapsed = Math.min(Math.max(at - last, 0), MAX_FRAME_MS);
            last = at;
            this.advance(elapsed * this.speed);
            if (run === this.run && this.playing) this.frame = requestAnimationFrame(step);
        };
        this.frame = requestAnimationFrame(step);
    }

    async togglePlay(): Promise<void> {
        if (this.recording) return;
        const shown = await this.focus(this.current);
        if (!shown) return;
        if (!this.playing && this.position >= shown.reel.duration) this.position = 0;
        this.setPlayback(true);
        this.playing = !this.playing;
        this.backend.replaySeek(shown.id, this.position);
        this.tick();
    }

    stop(): void {
        this.halt();
        this.playing = false;
        this.position = 0;
        this.backend.replayEnd();
        this.setPlayback(false);
    }

    private enter(): void {
        if (!this.recording) this.setPlayback(true);
    }

    seek(deltaMs: number): void {
        if (this.playback) this.moveTo(this.position + deltaMs);
    }

    step(dir: number): void {
        this.seek(dir * 100);
    }

    holdSeek(dir: number): void {
        this.seek(dir * 500);
    }

    async seekTo(ratio: number): Promise<void> {
        const shown = await this.focus(this.current);
        if (!shown) return;
        this.enter();
        this.moveTo(ratio * shown.reel.duration);
    }

    changeSpeed(dir: number): void {
        this.speed = Math.max(0.5, Math.min(4, dir > 0 ? this.speed * 2 : this.speed / 2));
    }

    async selectReel(dir: number): Promise<void> {
        if (!this.reels.length) return;
        if (!(await this.focus((this.current + dir + this.reels.length) % this.reels.length))) return;
        this.enter();
        this.moveTo(0);
    }

    deleteReel(key = this.reel?.key): void {
        const entry = this.reels.find((r) => r.key === key);
        if (!entry) return;
        const index = this.reels.indexOf(entry);
        this.reels.splice(index, 1);
        for (const pin of this.pins) if (pin.key === entry.key) pin.abort.abort();
        this.unload(entry);
        if (entry.stored !== null) deleteReel(entry.stored).then(() => this.measure());
        if (entry.thumb) URL.revokeObjectURL(entry.thumb.url);
        if (index < this.current) return void this.current--;
        if (index > this.current) return;
        if (!this.reels.length) return this.stop();
        this.prepared = null;
        this.current = Math.min(index, this.reels.length - 1);
        this.focus(this.current).then((shown) => shown && this.moveTo(0));
    }

    reset(): void {
        this.halt();
        this.playing = false;
        this.setPlayback(false);
        this.position = 0;
        this.recording = false;
        for (const entry of this.reels) if (entry.thumb) URL.revokeObjectURL(entry.thumb.url);
        for (const pin of this.pins) pin.abort.abort();
        this.pins.clear();
        this.reels = [];
        this.current = 0;
        this.focusing = null;
        this.generation++;
        this.backend.replayReset();
        clearReels().then(() => this.measure());
    }

    private moveTo(ms: number): void {
        const reel = this.reel;
        if (!reel) return;
        this.position = Math.max(0, Math.min(reel.duration, ms));
        if (this.playback && reel.id !== null) this.backend.replaySeek(reel.id, this.position);
        this.keepPreparing();
    }

    private advance(ms: number): void {
        const reel = this.reel;
        if (!reel) return this.stop();
        if (this.position + ms < reel.duration) return this.moveTo(this.position + ms);
        if (this.cont && this.current < this.reels.length - 1) {
            this.halt();
            this.focus(this.current + 1).then((shown) => {
                if (!shown) return this.stop();
                this.moveTo(0);
                this.tick();
            });
            return;
        }
        this.stop();
    }
}

export function timeText(ms: number): string {
    const s = ms / 1000;
    return `${Math.floor(s / 60)}:${(s % 60).toFixed(1).padStart(4, "0")}`;
}
