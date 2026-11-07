import { Net, World } from "../pkg/ogar_web";
import type { GameSource, GameState, MacroDef } from "./game.svelte";
import { IDLE_TABS } from "./game.svelte";
import { fromJson } from "./json";
import { HudApplier } from "./net/hud-apply";
import { tabIdentity } from "./net/identity";
import { SocketPipe, SoloPipe, type PipeEvents } from "./net/pipes";
import { reconnectDelay } from "./net/reconnect";
import { ReplayBridge } from "./net/replay-bridge";
import { TABS, type TabIndex } from "./net/tabs";
import { isSolo, socketUrl, soloMode } from "./servers";
import type { AppState } from "./state.svelte";
import type { ServerChoice } from "./types";

const QUICK_CAPTURE_SECONDS = [10, 20, 30, 40, 50, 60];

export class NetSource implements GameSource {
    private liveWorld = new World();
    readonly replays = new ReplayBridge(
        (u, net) => this.hud.apply(u, net),
        () => this.net,
    );
    private net: Net | null = null;
    private url = "";
    private solo = false;
    private soloMode = 0;
    private pipes: [SocketPipe | SoloPipe | null, SocketPipe | SoloPipe | null] = [null, null];
    private retries: [number, number] = [0, 0];
    private retryTimers: [number, number] = [0, 0];
    private tabsAsUnits = false;
    private applied = { recording: false, captureMs: 0 };
    private cursor = { x: 0, y: 0 };
    private tick = 0;
    private toWorld: ((x: number, y: number) => Float32Array) | null = null;
    private timer = 0;
    private timerDue = Number.POSITIVE_INFINITY;
    private aimed: [number, number] = [Number.NaN, Number.NaN];
    private frames = 0;
    private fpsSince = 0;
    private readonly hud: HudApplier;

    constructor(
        private app: AppState,
        private game: GameState,
    ) {
        this.hud = new HudApplier(app, game);
    }

    get world(): World {
        return this.replays.playing ? this.replays.world : this.liveWorld;
    }

    start(): void {
        addEventListener("pagehide", (e) => {
            if (e.persisted) this.disconnect();
        });
        addEventListener("pageshow", (e) => {
            if (e.persisted) location.reload();
        });
    }

    connect(server: ServerChoice): void {
        this.disconnect();
        this.solo = isSolo(server);
        this.soloMode = soloMode(server);
        this.url = this.solo ? "" : socketUrl(server.addr);
        this.game.closeNotice = null;
        this.game.population = { total: 0, max: 0, players: 0, spectators: 0 };
        const net = new Net();
        const recording = this.app.flag("ReplayBuffer"),
            captureMs = QUICK_CAPTURE_SECONDS[this.app.num("QuickCaptureTimeOption")] * 1000;
        this.replays.recorder.setWindow(recording ? captureMs : undefined);
        this.replays.recorder.onConnect();
        this.applied = { recording, captureMs };
        net.setTabsAsUnits(this.tabsAsUnits);
        this.net = net;
        this.openPipe(net, 0);
    }

    setTabsAsUnits(on: boolean): void {
        this.tabsAsUnits = on;
        if (this.net) this.net.setTabsAsUnits(on);
        else this.game.tabs = { ...this.game.tabs, tabsAsUnits: on };
    }

    runMacro(macro: MacroDef): void {
        this.withNet((net, now) => {
            this.refreshPoint(net);
            net.runMacro(JSON.stringify(macro), now);
        });
    }

    stopMacros(): void {
        this.withNet((net, now) => net.stopMacros(now));
    }

    closeTab(tab: TabIndex): void {
        const net = this.net;
        if (!net) return;
        this.flush(net);
        this.closePipe(net, tab);
    }

    refreshTab(tab: TabIndex): void {
        const net = this.net;
        if (!net) return;
        this.flush(net);
        this.closePipe(net, tab);
        this.openPipe(net, tab);
    }

    play(tab?: number): void {
        this.withNet((net, now) => {
            this.refreshIdentity(net);
            net.join(tab, now);
        });
    }

    spectate(): void {
        this.withNet((net, now) => {
            this.refreshIdentity(net);
            net.spectate(now);
        });
    }

    watch(free: boolean): void {
        this.withNet((net, now) => net.setFreeRoam(free, now));
    }

    say(text: string): void {
        this.withNet((net, now) => net.sendChat(text, now));
    }

    act(action: string): void {
        this.withNet((net, now) => {
            this.refreshPoint(net);
            net.onHotkey(action, true, now);
        });
    }

    release(action: string): void {
        this.withNet((net, now) => {
            this.refreshPoint(net);
            net.onHotkey(action, false, now);
        });
    }

    private refreshPoint(net: Net): void {
        const toWorld = this.toWorld;
        if (!toWorld) return;
        const [ax, ay] = toWorld(this.cursor.x, this.cursor.y);
        net.updatePointer(ax, ay);
    }

    mouse(button: number, down: boolean): void {
        const toWorld = this.toWorld;
        if (!toWorld) return;
        const [x, y] = toWorld(this.cursor.x, this.cursor.y);
        this.withNet((net, now) => {
            this.refreshPoint(net);
            net.onMouse(button, down, x, y, this.world, now);
        });
    }

    recordCamera(x: number, y: number, scale: number): void {
        if (this.net) this.replays.recordSight(this.liveWorld, x, y, scale, this.aimed);
    }

    frame(now: number, toWorld: (x: number, y: number) => Float32Array): void {
        const net = this.net;
        if (!net) return;
        net.tick(now);
        const recording = this.app.flag("ReplayBuffer"),
            captureMs = QUICK_CAPTURE_SECONDS[this.app.num("QuickCaptureTimeOption")] * 1000;
        const a = this.applied;
        if (recording !== a.recording || captureMs !== a.captureMs) {
            this.replays.recorder.setWindow(recording ? captureMs : undefined);
            a.recording = recording;
            a.captureMs = captureMs;
        }
        this.toWorld = toWorld;
        this.countFrame(now);
        if (this.tick++ % 2 === 0) {
            const [x, y] = toWorld(this.cursor.x, this.cursor.y);
            if ((x !== this.aimed[0] || y !== this.aimed[1]) && net.aim(x, y, now)) this.aimed = [x, y];
        }
        this.flush(net);
    }

    pointer(x: number, y: number): void {
        this.cursor = { x, y };
    }

    private countFrame(now: number): void {
        this.frames++;
        if (now - this.fpsSince < 1000) return;
        this.game.stats = { ...this.game.stats, fps: Math.round((this.frames * 1000) / (now - this.fpsSince)) };
        this.frames = 0;
        this.fpsSince = now;
    }

    private withNet(run: (net: Net, now: number) => void): void {
        if (!this.net) return;
        run(this.net, performance.now());
        this.flush(this.net);
    }

    private refreshIdentity(net: Net): void {
        for (const tab of TABS) if (net.isConnected(tab)) net.setIdentity(tab, tabIdentity(this.app, tab));
    }

    private openPipe(net: Net, tab: TabIndex): void {
        net.markDialing(tab);
        const on: PipeEvents = {
            open: () => {
                net.onOpen(tab, tabIdentity(this.app, tab), performance.now());
                this.aimed = [Number.NaN, Number.NaN];
                this.flush(net);
            },
            message: (data) => {
                this.retries[tab] = 0;
                if (this.game.closeNotice?.tab === tab) this.game.closeNotice = null;
                const error = net.receive(tab, data, this.liveWorld, this.replays.recorder, performance.now());
                if (error) console.warn(`tab ${tab + 1} decode failed, ${error}`);
                this.flush(net);
            },
            closed: (reason) => this.pipeClosed(net, tab, reason),
        };
        try {
            this.pipes[tab] = this.solo ? new SoloPipe(on, this.soloMode) : new SocketPipe(this.url, on);
        } catch (e) {
            this.pipeClosed(net, tab, e instanceof Error ? e.message : String(e));
        }
    }

    private pipeClosed(net: Net, tab: TabIndex, reason: string): void {
        this.pipes[tab] = null;
        net.onClose(tab, this.liveWorld);
        this.flush(net);
        if (reason) this.game.closeNotice = { tab, text: reason };
        if (TABS.every((t) => !net.isConnected(t)) && !this.app.replay.playback) this.app.menuOpen = true;
        if (this.retries[tab] === 0) {
            const t = this.app.t;
            const text = t("netTabClosed").replace("%s", String(tab + 1));
            this.game.system(reason ? `${text} (${reason})` : text, t("chatSenderConnection"));
        }
        if (this.app.flag("AutoReconnect")) this.scheduleReconnect(net, tab);
    }

    private disconnect(): void {
        const net = this.net;
        this.replays.dropHeld();
        if (net) for (const tab of TABS) this.closePipe(net, tab);
        clearTimeout(this.timer);
        this.timerDue = Number.POSITIVE_INFINITY;
        net?.free();
        this.net = null;
        this.game.tabs = IDLE_TABS;
    }

    private scheduleReconnect(net: Net, tab: TabIndex): void {
        const limit = this.app.ui.pickSources.reconnectTries[this.app.num("ReconnectTries")];
        if (typeof limit === "number" && this.retries[tab] >= limit) {
            const t = this.app.t;
            const text = t("netReconnectGaveUp")
                .replace("%s", String(tab + 1))
                .replace("%s", String(limit));
            this.game.system(text, t("chatSenderConnection"));
            return;
        }
        const delay = reconnectDelay(
            this.app.num("ReconnectInterval"),
            this.app.flag("ReconnectDoubling"),
            this.retries[tab],
        );
        this.retries[tab] += 1;
        this.retryTimers[tab] = window.setTimeout(() => {
            this.retryTimers[tab] = 0;
            if (this.net === net && !this.pipes[tab] && this.app.flag("AutoReconnect")) this.openPipe(net, tab);
        }, delay);
    }

    private closePipe(net: Net, tab: TabIndex): void {
        clearTimeout(this.retryTimers[tab]);
        this.retryTimers[tab] = 0;
        this.retries[tab] = 0;
        if (this.game.closeNotice?.tab === tab) this.game.closeNotice = null;
        const pipe = this.pipes[tab];
        if (!pipe) return;
        this.pipes[tab] = null;
        pipe.close();
        net.onClose(tab, this.liveWorld);
        this.flush(net);
    }

    private flush(net: Net): void {
        for (const tab of TABS) {
            const pipe = this.pipes[tab];
            if (!pipe?.ready) continue;
            for (let p = net.takeOutgoing(tab); p; p = net.takeOutgoing(tab)) pipe.send(p);
        }
        const text = net.takeHud();
        if (text && this.replays.playing) this.replays.hold(text);
        else if (text) this.hud.apply(fromJson(text), net);
        this.schedule(net);
    }

    private schedule(net: Net): void {
        const due = net.nextTickAt();
        if (due === undefined || due >= this.timerDue) return;
        clearTimeout(this.timer);
        this.timerDue = due;
        this.timer = window.setTimeout(
            () => {
                this.timerDue = Number.POSITIVE_INFINITY;
                this.withNet((n, now) => n.tick(now));
            },
            Math.max(0, due - performance.now()),
        );
    }
}
