<script lang="ts">
    import StatusRows from "./StatusRows.svelte";
    import { useApp, useHudPlacement } from "../../lib/context";

    const app = useApp();
    const placement = useHudPlacement();
</script>

<div class="status" {@attach placement("status")}>
    <StatusRows server={app.flag("ShowServerStatus")} client={app.flag("ShowClientStatus")} />
</div>

<style>
    .status {
        position: absolute;
        left: 1.4rem;
        bottom: 1rem;
        font-size: var(--fs-sm);
        font-variant-numeric: tabular-nums;
    }

    :global(html.mobile) .status {
        top: calc(var(--mm-top) + var(--mm-size) + 0.4rem);
        left: var(--hud-left);
        bottom: auto;
        font-size: var(--fs-xs);
    }

    @media (orientation: portrait) {
        :global(html.mobile) .status {
            top: calc(var(--mm-top) + var(--mm-size) + var(--mlog-chat) + 0.4rem);
        }
    }
</style>
