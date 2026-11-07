import type { ServerChoice } from "./types";

const SOLO_ID = "solo";
const SOLO_MODE_PREFIX = "solo:";
// serverGamemode of each solo server; FFA keeps the plain id so saved choices still work.
const SOLO_MODES: readonly [number, string][] = [
    [0, "FFA"],
    [1, "Teams"],
    [2, "Experimental"],
    [3, "Rainbow"],
];
const LOCAL_ID = "local";
const LOCAL_LABEL = "Local";
const DEFAULT_PORT = 8080;
const CUSTOM_PREFIX = "custom:";
const SELECTED_KEY = "ogarSelectedServer";
const CUSTOM_KEY = "ogarCustomServers";
const QUERY_PARAM = "ip";
const MAX_PORT = 65535;
const ADDRESS = /^(?:wss?:\/\/)?(?:\[[0-9a-f:.]+\]|[a-z0-9._-]+)(?::(\d{1,5}))?$/i;
const SCHEME = /^wss?:\/\//i;

export function normalizeAddress(raw: string): string {
    return raw.trim().replace(/\/+$/, "");
}

export function isValidAddress(addr: string): boolean {
    const match = ADDRESS.exec(addr);
    return match !== null && Number(match[1] ?? 0) <= MAX_PORT;
}

function soloId(mode: number): string {
    return mode === 0 ? SOLO_ID : SOLO_MODE_PREFIX + mode;
}

export function soloServers(): ServerChoice[] {
    return SOLO_MODES.map(([mode, name]) => ({ id: soloId(mode), label: `Solo ${name}`, addr: "" }));
}

function soloById(id: string): ServerChoice | null {
    return soloServers().find((s) => s.id === id) ?? null;
}

export function isSolo(choice: ServerChoice): boolean {
    return soloById(choice.id) !== null;
}

export function soloMode(choice: ServerChoice): number {
    return choice.id.startsWith(SOLO_MODE_PREFIX) ? Number(choice.id.slice(SOLO_MODE_PREFIX.length)) : 0;
}

export function localServer(): ServerChoice {
    return { id: LOCAL_ID, label: LOCAL_LABEL, addr: `${location.hostname || "localhost"}:${DEFAULT_PORT}` };
}

export function customChoice(addr: string): ServerChoice {
    return { id: CUSTOM_PREFIX + addr, label: addr, addr };
}

export function queryServer(): ServerChoice | null {
    const addr = normalizeAddress(new URLSearchParams(location.search).get(QUERY_PARAM) ?? "");
    if (!addr) return null;
    if (isValidAddress(addr)) return customChoice(addr);
    console.warn(`ignored invalid ?${QUERY_PARAM}= address ${addr}`);
    return null;
}

export function savedServer(): ServerChoice {
    const query = queryServer();
    if (query) return query;
    const saved = localStorage.getItem(SELECTED_KEY) ?? "";
    if (saved === LOCAL_ID) return localServer();
    const solo = soloById(saved);
    if (solo) return solo;
    const addr = saved.startsWith(CUSTOM_PREFIX) ? saved.slice(CUSTOM_PREFIX.length) : "";
    return isValidAddress(addr) ? customChoice(addr) : soloServers()[0];
}

export function saveSelectedServer(choice: ServerChoice): void {
    localStorage.setItem(SELECTED_KEY, choice.id);
}

export function socketUrl(addr: string): string {
    if (SCHEME.test(addr)) return addr;
    return `${location.protocol === "https:" ? "wss" : "ws"}://${addr}`;
}

export function customServers(): string[] {
    try {
        const list: unknown = JSON.parse(localStorage.getItem(CUSTOM_KEY) ?? "[]");
        return Array.isArray(list) ? list.filter((a): a is string => typeof a === "string" && isValidAddress(a)) : [];
    } catch (e) {
        console.error(`${CUSTOM_KEY} is not valid JSON`, e);
        return [];
    }
}

function saveCustomServers(list: string[]): void {
    localStorage.setItem(CUSTOM_KEY, JSON.stringify(list));
}

export function addCustomServer(addr: string): void {
    const list = customServers();
    if (!list.includes(addr)) saveCustomServers([...list, addr]);
}

export function removeCustomServer(addr: string): void {
    saveCustomServers(customServers().filter((a) => a !== addr));
}
