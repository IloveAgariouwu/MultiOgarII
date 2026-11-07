import type { Component } from "svelte";
import type { SettingsMap } from "../../lib/types";
import ReplayBar from "../ReplayBar.svelte";
import ClientStatus from "./ClientStatus.svelte";
import Leaderboard from "./Leaderboard.svelte";
import Minimap from "./Minimap.svelte";
import SelfPanel from "./SelfPanel.svelte";

export interface HudWidget {
    kind: string;
    component: Component;
    shown: (s: SettingsMap) => boolean;
    desktopOnly: boolean;
}

export const HUD_WIDGETS: HudWidget[] = [
    {
        kind: "status",
        component: ClientStatus,
        shown: (s) => !!(s.ShowClientStatus || s.ShowServerStatus),
        desktopOnly: false,
    },
    { kind: "rail", component: Leaderboard, shown: (s) => !!s.ShowLeaderboard, desktopOnly: false },
    { kind: "self", component: SelfPanel, shown: (s) => !!s.ShowSelfPanel, desktopOnly: false },
    { kind: "map", component: Minimap, shown: (s) => !!s.ShowMap, desktopOnly: false },
    { kind: "replay", component: ReplayBar, shown: (s) => !!s.ShowReplayBar, desktopOnly: true },
];
