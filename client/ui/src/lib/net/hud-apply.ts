import type { Net } from "../../pkg/ogar_web";
import type { GameState } from "../game.svelte";
import type { AppState } from "../state.svelte";
import type { HudUpdate } from "./hud";

export class HudApplier {
    constructor(
        private readonly app: AppState,
        private readonly game: GameState,
    ) {}

    apply(u: HudUpdate, net: Net | null): void {
        const game = this.game;
        if (u.leaderboard) game.leaderboard = u.leaderboard;
        if (u.teams) game.teams = u.teams;
        if (u.units) game.units = u.units;
        if (u.self) game.self = { ...game.self, ...u.self };
        if (u.population) game.population = u.population;
        if (u.serverStatus !== undefined || u.latency !== undefined || u.latency2 !== undefined) {
            const s = game.stats;
            game.stats = {
                ...s,
                serverStatus: u.serverStatus ?? s.serverStatus,
                latency: u.latency !== undefined ? Math.round(u.latency) : s.latency,
                latency2: u.latency2 !== undefined ? Math.round(u.latency2) : s.latency2,
            };
        }
        for (const m of u.chat ?? [])
            game.pushChat({ name: m.name, text: m.text, color: m.color, time: new Date(), system: m.system });
        if (!net) return;
        if (u.tabs) game.tabs = u.tabs;
        const t = this.app.t;
        for (const e of u.events ?? []) {
            switch (e.kind) {
                case "death":
                    game.death = { tab: e.tab, id: (game.death?.id ?? 0) + 1 };
                    break;
                case "chatTooLong":
                    game.system(t("netChatTooLong"), t("chatSenderConnection"));
                    break;
                default: {
                    const unhandled: never = e;
                    console.error("unhandled hud event", unhandled);
                }
            }
        }
    }
}
