<script lang="ts">
    import { onMount } from "svelte";
    import ChatLog from "../chat/ChatLog.svelte";
    import ClientStatus from "../hud/ClientStatus.svelte";
    import Leaderboard from "../hud/Leaderboard.svelte";
    import Minimap from "../hud/Minimap.svelte";
    import SelfPanel from "../hud/SelfPanel.svelte";
    import ReplayBar from "../ReplayBar.svelte";
    import StatusRows from "../hud/StatusRows.svelte";
    import { provideGame, provideHud, provideReplay, useApp, useGame, type HudPlacement } from "../../lib/context";
    import { chatVars, hudVars } from "../../lib/hud-style";
    import { PreviewStage, previewChat } from "../../lib/preview-game";
    import { PreviewSource } from "../../lib/preview-source";
    import { ReplayState } from "../../lib/replay.svelte";
    import type { HudPart } from "../../lib/types";

    let { parts }: { parts: HudPart[] } = $props();
    const STEP_MS = 100;
    const MARGIN_PX = 24;
    const VIEW_W = 0.18;
    const VIEW_H = 0.1;
    const REEL_MS = 60000;
    const REEL_AT = 0.4;
    const SERVER_STATUS = "MultiOgarII (Free For All)\nLoad: 4.1 %\nUptime: 3 h";
    const placement: HudPlacement = () => () => {};
    const app = useApp();
    const live = useGame();
    const me = app.profiles.entries[app.profiles.main].name;
    const stage = new PreviewStage(app.core.minimapLayout(), me);
    previewChat(stage.game, stage.roster, [app.t("qcDefault1"), `@${me} ${app.t("qcDefault2")}`, app.t("qcDefault8")]);
    provideGame(stage.game, new PreviewSource(stage.world));
    const self = stage.game.units[0]?.spot ?? { x: VIEW_W, y: VIEW_H };
    provideHud(placement, {
        ex: self.x,
        ey: self.y,
        x0: self.x - VIEW_W,
        y0: self.y - VIEW_H,
        x1: self.x + VIEW_W,
        y1: self.y + VIEW_H,
    });
    const replay = new ReplayState(
        () => {},
        () => {},
        () => {},
    );
    replay.reels = [
        { key: 1, id: 1, duration: REEL_MS, origin: "live", stored: null, size: null, thumb: null, clip: null },
    ];
    replay.position = REEL_MS * REEL_AT;
    replay.cont = true;
    provideReplay(replay);
    let s = $derived(app.settings);
    let width = $state(0);
    let height = $state(0);
    let fitWidth = $state(0);
    let fitHeight = $state(0);
    let scale = $derived(
        fitWidth && fitHeight ? Math.min(1, (width - MARGIN_PX) / fitWidth, (height - MARGIN_PX) / fitHeight) : 1,
    );

    onMount(() => {
        const timer = setInterval(() => {
            stage.step(performance.now());
            stage.game.stats = { ...live.stats, serverStatus: live.stats.serverStatus || SERVER_STATUS };
        }, STEP_MS);
        return () => {
            clearInterval(timer);
            stage.free();
        };
    });
</script>

<div class="pvhud" bind:clientWidth={width} bind:clientHeight={height}>
    <div
        class="fit"
        style={hudVars(app.colors)}
        style:transform="scale({scale})"
        bind:offsetWidth={fitWidth}
        bind:offsetHeight={fitHeight}
    >
        {#if s.ShowUI}
            {#each parts as part (part)}
                {#if part === "chat" || part === "mention"}
                    {#if s.ShowChatBox}<div class="part scrim" style={chatVars(app.colors, app.chat.size)}>
                            <ChatLog />
                        </div>{/if}
                {:else if part === "map" && s.ShowMap}<div class="part"><Minimap /></div>
                {:else if part === "rail" && s.ShowLeaderboard}<div class="part"><Leaderboard /></div>
                {:else if part === "self" && s.ShowSelfPanel}<div class="part"><SelfPanel /></div>
                {:else if part === "status" && (s.ShowClientStatus || s.ShowServerStatus)}<div class="part">
                        <ClientStatus />
                    </div>
                {:else if part === "replay" && s.ShowReplayBar}<div class="part"><ReplayBar /></div>
                {:else if part === "client" && s.ShowClientStatus}<div class="part stats">
                        <StatusRows server={false} client={true} />
                    </div>
                {:else if part === "server" && s.ShowServerStatus}<div class="part stats">
                        <StatusRows server={true} client={false} />
                    </div>{/if}
            {/each}
        {/if}
    </div>
</div>

<style>
    .pvhud {
        position: absolute;
        inset: 0;
        display: flex;
        align-items: center;
        justify-content: center;
        pointer-events: none;
        text-shadow: var(--shadow);
    }

    .fit {
        display: flex;
        flex: none;
        align-items: center;
        gap: 2rem;
    }

    .part {
        padding: var(--scrim-y) var(--scrim-x);
    }

    .stats {
        font-size: var(--fs-sm);
        font-variant-numeric: tabular-nums;
    }

    .pvhud .part :global(.replay) {
        position: relative;
        left: auto;
        top: auto;
        transform: none;
    }

    .pvhud .part > :global(*) {
        position: relative;
        inset: auto;
        margin: 0;
    }
</style>
