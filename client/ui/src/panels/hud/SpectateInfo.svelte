<script lang="ts">
    import { useApp, useGame } from "../../lib/context";

    const app = useApp();
    const game = useGame();
    const t = app.t;
</script>

<div class="spec" {@attach app.hud.slot("spectate")}>
    {#if game.spectating || app.hud.editing}
        <div class="who">{t("hudSpectating")}</div>
    {/if}
    {#if !game.focused}<div class="nofocus">{t("hudNoFocus")}</div>{/if}
</div>

<style>
    .spec {
        position: absolute;
        left: 50%;
        top: 7.5rem;
        transform: translateX(-50%);
        text-align: center;
        white-space: nowrap;
    }

    .who {
        color: var(--fg);
        font-size: var(--fs-md);
    }

    .nofocus {
        color: var(--accent);
        font-size: var(--fs-xs);
    }
</style>
