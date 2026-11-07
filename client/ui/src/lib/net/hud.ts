import type { LeaderEntry, ServerPopulation, TabsState, TeamEntry, UnitEntry } from "../game.svelte";

interface HudSelf {
    score?: number;
    max?: number;
    split1?: number;
    split2?: number;
    pos1?: string;
    pos2?: string;
}

export interface HudChat {
    name: string;
    text: string;
    color: string;
    system: boolean;
}

export type HudEvent = { kind: "death"; tab: number } | { kind: "chatTooLong" };

export interface HudUpdate {
    leaderboard?: LeaderEntry[];
    teams?: TeamEntry[];
    units?: UnitEntry[];
    self?: HudSelf;
    population?: ServerPopulation;
    serverStatus?: string;
    latency?: number;
    latency2?: number;
    tabs?: TabsState;
    chat?: HudChat[];
    events?: HudEvent[];
}
