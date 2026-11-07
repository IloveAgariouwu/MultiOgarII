<script lang="ts">
    import { useApp, useGame } from "../../lib/context";
    import { hudVars } from "../../lib/hud-style";
    import SpectateInfo from "./SpectateInfo.svelte";
    import { HUD_WIDGETS } from "./widgets";

    const app = useApp();
    const game = useGame();
    let s = $derived(app.settings);
    let style = $derived(hudVars(app.colors));
</script>

<div class="hud" {style}>
    {#if (game.spectating || app.hud.editing) && !gMobile}<SpectateInfo />{/if}
    {#if s.ShowUI}
        {#each HUD_WIDGETS as w (w.kind)}
            {#if w.shown(s) && !(w.desktopOnly && gMobile)}<w.component />{/if}
        {/each}
    {/if}
</div>

<style>
    .hud {
        position: fixed;
        inset: 0;
        pointer-events: none;
        text-shadow: var(--shadow);
    }

    .hud :global(> *) {
        pointer-events: auto;
    }

    :global(html.mobile) {
        --hud-pad: 0.6rem;
        --hud-top: max(var(--hud-pad), env(safe-area-inset-top));
        --hud-left: max(var(--hud-pad), env(safe-area-inset-left));
        --hud-right: max(var(--hud-pad), env(safe-area-inset-right));
        --mm-size: 88px;
        --mm-top: var(--hud-top);
        --lb-w: 188px;
        --mlog-h: 66px;
        --mlog-chat: 0px;
    }

    :global(html.mobile:has(.mlog)) {
        --mlog-chat: calc(var(--mlog-h) + var(--hud-pad));
    }

    @media (orientation: portrait) {
        :global(html.mobile) {
            --mm-top: calc(var(--hud-top) + var(--hit) * 1.25 + var(--hud-pad));
        }
    }

    :global(html.mobile) .hud :global(> *) {
        pointer-events: none;
    }
</style>
