import type { World } from "../pkg/ogar_web";
import type { ServerChoice } from "./types";

export const DEAD_POSITION = "NaN";

export interface LeaderEntry {
    rank: number;
    name: string;
    self: boolean;
}

export interface TeamEntry {
    color: string;
    share: number;
}

export interface SelfInfo {
    score: number;
    max: number;
    split1: number;
    split2: number;
    pos1: string;
    pos2: string;
}

export interface ClientStats {
    fps: number;
    latency: number;
    latency2: number;
    render: number;
    serverStatus: string;
}

export interface ServerPopulation {
    total: number;
    max: number;
    players: number;
    spectators: number;
}

export interface UnitEntry {
    id: number;
    name: string;
    mass: number;
    color: string;
    cell: string;
    spot: MapSpot;
}

export interface MapSpot {
    x: number;
    y: number;
    r: number;
}

export interface ChatMessage {
    id: number;
    name: string;
    text: string;
    color: string;
    time: Date;
    system?: boolean;
}

export interface CloseNotice {
    tab: 0 | 1;
    text: string;
}

export class GameState {
    leaderboard = $state.raw<LeaderEntry[]>([]);
    teams = $state.raw<TeamEntry[]>([]);
    self = $state<SelfInfo>({ score: 0, max: 0, split1: 0, split2: 0, pos1: "", pos2: "" });
    stats = $state<ClientStats>({ fps: 0, latency: 0, latency2: 0, render: 0, serverStatus: "" });
    population = $state<ServerPopulation>({ total: 0, max: 0, players: 0, spectators: 0 });
    units = $state.raw<UnitEntry[]>([]);
    chat = $state<ChatMessage[]>([]);
    focused = $state(true);
    tabs = $state<TabsState>(IDLE_TABS);
    closeNotice = $state.raw<CloseNotice | null>(null);
    death = $state<{ tab: number; id: number } | null>(null);
    activeTab = $derived(this.tabs.active);
    tab2Connected = $derived(LINKED.includes(this.tabs.tabs[1].phase));
    tabsAsUnits = $derived(this.tabs.tabsAsUnits);
    playing = $derived(this.tabs.tabs.some((t) => t.phase === "alive"));
    spectating = $derived(this.tabs.tabs[this.tabs.active].phase === "spectating");
    bench = $state(false);
    private nextChatId = 1;

    pushChat(m: Omit<ChatMessage, "id">): void {
        this.chat.push({ ...m, id: this.nextChatId++ });
    }

    system(text: string, name = "SERVER"): void {
        this.pushChat({ name, text, color: "", time: new Date(), system: true });
    }
}

export type ReplayImport = { ok: true; reel: ReplayReel } | { ok: false; error: string };

export interface ReplayCamera {
    eyeX: number;
    eyeY: number;
    scale: number;
    aimX: number;
    aimY: number;
    splitting: boolean;
    aimPlayer: number;
}

export type TabPhase = "closed" | "connecting" | "lobby" | "joining" | "alive" | "spectating";

export interface TabView {
    phase: TabPhase;
}

export interface TabsState {
    tabs: [TabView, TabView];
    mode: "single" | "dual" | "tabsAsUnits";
    tabsAsUnits: boolean;
    active: number;
    intent: "idle" | "play" | "spectate";
}

const CLOSED_TAB: TabView = { phase: "closed" };
export const IDLE_TABS: TabsState = {
    tabs: [CLOSED_TAB, CLOSED_TAB],
    mode: "single",
    tabsAsUnits: false,
    active: 0,
    intent: "idle",
};
const LINKED: TabPhase[] = ["lobby", "joining", "alive", "spectating"];

export interface ReplayReel {
    id: number;
    duration: number;
    origin: "live" | "imported";
}

export type MacroPhase = "down" | "up" | "tap";

export interface MacroStep {
    action: string;
    phase: MacroPhase;
    delayMs: number;
}

export interface MacroDef {
    name: string;
    trigger: number;
    repeat: number;
    steps: MacroStep[];
}

export interface GameSource {
    readonly world: World;
    start(): void;
    connect(server: ServerChoice): void;
    play(tab?: number): void;
    spectate(): void;
    watch(free: boolean): void;
    say(text: string): void;
    refreshTab(tab: 0 | 1): void;
    closeTab(tab: 0 | 1): void;
    mouse(button: number, down: boolean): void;
    recordCamera(x: number, y: number, scale: number): void;
    setTabsAsUnits(on: boolean): void;
    runMacro(macro: MacroDef): void;
    stopMacros(): void;
    act(action: string): void;
    release(action: string): void;
    frame(now: number, toWorld: (x: number, y: number) => Float32Array): void;
    pointer(x: number, y: number): void;
}
