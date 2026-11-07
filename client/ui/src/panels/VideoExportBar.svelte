<script lang="ts">
    import Icon from "../components/Icon.svelte";
    import { useApp } from "../lib/context";

    let { progress }: { progress: number } = $props();
    const app = useApp();
</script>

<div class="export">
    <Icon name="film" />
    <b>{Math.round(progress * 100)}%</b>
    <button class="btn-line" onclick={() => app.video.cancel()}>{app.t("replayVideoStop")}</button>
</div>

<style>
    .export {
        position: fixed;
        left: 50%;
        top: 0.75rem;
        z-index: var(--z-bar);
        display: flex;
        align-items: center;
        gap: 0.9rem;
        padding: 0.5rem 0.9rem;
        transform: translateX(-50%);
        background: var(--panel);
        box-shadow: 0 0 0 1px var(--rule);
    }

    .export :global(svg) {
        width: var(--icon-sm);
        height: var(--icon-sm);
        fill: none;
        stroke: currentColor;
        stroke-width: 1.8;
    }

    b {
        font-weight: 600;
        font-variant-numeric: tabular-nums;
    }
</style>
