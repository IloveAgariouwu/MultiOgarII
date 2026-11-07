import { dateTimeStamp } from "./format";
import type { ReplayReel } from "./game.svelte";

const DB_NAME = "ogar-replays";
const REELS = "reels";
const META = "meta";
const VERSION = 2;

export interface ReelMeta {
    id: number;
    duration: number;
    origin: ReplayReel["origin"];
    size: number;
    thumb: Blob | null;
}

type StoredMeta = Omit<ReelMeta, "size"> & { size?: number };

interface StoredReel {
    id: number;
    bytes: Blob | Uint8Array;
}

let connection: Promise<IDBDatabase> | null = null;

function openDatabase(): Promise<IDBDatabase> {
    connection ??= new Promise((resolve, reject) => {
        const req = indexedDB.open(DB_NAME, VERSION);
        req.onupgradeneeded = () => {
            const db = req.result;
            if (!db.objectStoreNames.contains(REELS))
                db.createObjectStore(REELS, { keyPath: "id", autoIncrement: true });
            if (!db.objectStoreNames.contains(META)) db.createObjectStore(META, { keyPath: "id" });
        };
        req.onsuccess = () => resolve(req.result);
        req.onerror = () => reject(req.error);
    });
    return connection;
}

async function runRequest<T>(
    store: string,
    mode: IDBTransactionMode,
    body: (store: IDBObjectStore) => IDBRequest<T>,
): Promise<T> {
    const db = await openDatabase();
    return new Promise((resolve, reject) => {
        const req = body(db.transaction(store, mode).objectStore(store));
        req.onsuccess = () => resolve(req.result);
        req.onerror = () => reject(req.error);
    });
}

async function withReelsAndMeta(body: (reels: IDBObjectStore, meta: IDBObjectStore) => void): Promise<void> {
    const db = await openDatabase();
    return new Promise((resolve, reject) => {
        const tx = db.transaction([REELS, META], "readwrite");
        body(tx.objectStore(REELS), tx.objectStore(META));
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
        tx.onabort = () => reject(tx.error);
    });
}

let persistRequested = false;

export async function saveReel(bytes: Uint8Array, info: Pick<ReelMeta, "duration" | "origin">): Promise<number> {
    let id = 0;
    await withReelsAndMeta((reels, meta) => {
        const req = reels.add({ bytes: new Blob([bytes.slice()]) });
        req.onsuccess = () => {
            id = Number(req.result);
            meta.put({ id, thumb: null, size: bytes.length, ...info });
        };
    });
    if (!persistRequested) {
        persistRequested = true;
        void navigator.storage.persist();
    }
    return id;
}

export function isStorageFull(e: unknown): boolean {
    return e instanceof DOMException && e.name === "QuotaExceededError";
}

export async function storageUsage(): Promise<{ used: number; quota: number } | null> {
    const estimate = await navigator.storage.estimate();
    return estimate.usage !== undefined && estimate.quota !== undefined
        ? { used: estimate.usage, quota: estimate.quota }
        : null;
}

export function storagePersisted(): Promise<boolean> {
    return navigator.storage.persisted();
}

export async function loadMeta(): Promise<ReelMeta[]> {
    const metas: StoredMeta[] = await runRequest(META, "readonly", (s) => s.getAll());
    const loaded: ReelMeta[] = [];
    for (const m of metas) {
        if (m.size !== undefined) {
            loaded.push({ ...m, size: m.size });
            continue;
        }
        const stored = await runRequest<StoredReel | undefined>(REELS, "readonly", (s) => s.get(m.id));
        if (!stored) {
            await runRequest(META, "readwrite", (s) => s.delete(m.id));
            continue;
        }
        const sized = { ...m, size: stored.bytes instanceof Blob ? stored.bytes.size : stored.bytes.length };
        await putMeta(sized);
        loaded.push(sized);
    }
    return loaded;
}

export async function reelsWithoutMeta(): Promise<number[]> {
    const [reels, meta] = await Promise.all([
        runRequest(REELS, "readonly", (s) => s.getAllKeys()),
        runRequest(META, "readonly", (s) => s.getAllKeys()),
    ]);
    const known = new Set(meta.map(Number));
    return reels.map(Number).filter((id) => !known.has(id));
}

export async function readReel(id: number): Promise<Uint8Array | null> {
    const stored = await runRequest<StoredReel | undefined>(REELS, "readonly", (s) => s.get(id));
    if (!stored) return null;
    return stored.bytes instanceof Blob ? new Uint8Array(await stored.bytes.arrayBuffer()) : stored.bytes;
}

export function putMeta(meta: ReelMeta): Promise<IDBValidKey> {
    return runRequest(META, "readwrite", (s) => s.put(meta));
}

export async function setThumb(id: number, thumb: Blob): Promise<void> {
    const db = await openDatabase();
    return new Promise((resolve, reject) => {
        const tx = db.transaction(META, "readwrite");
        const meta = tx.objectStore(META);
        const req: IDBRequest<ReelMeta | undefined> = meta.get(id);
        req.onsuccess = () => {
            if (req.result) meta.put({ ...req.result, thumb });
        };
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
        tx.onabort = () => reject(tx.error);
    });
}

export function deleteReel(id: number): Promise<void> {
    return withReelsAndMeta((reels, meta) => {
        reels.delete(id);
        meta.delete(id);
    });
}

export function clearReels(): Promise<void> {
    return withReelsAndMeta((reels, meta) => {
        reels.clear();
        meta.clear();
    });
}

const UNSAFE_FILE_CHARS = /[\\/:*?"<>|\s]+/g;

export function saveFile(blob: Blob, kind: string, ext: string): void {
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = `multiogarii-${kind.replace(UNSAFE_FILE_CHARS, "_")}-${dateTimeStamp(new Date()).replace(/[: ]/g, "-")}.${ext}`;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 10 * 60 * 1000);
}
