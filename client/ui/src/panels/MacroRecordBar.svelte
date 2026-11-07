<script lang="ts">
    import { useApp } from "../lib/context";
    import type { MacroDef } from "../lib/game.svelte";

    let { macro }: { macro: MacroDef } = $props();
    const app = useApp();
    const t = app.t;
</script>

<div class="rec" {@attach app.layer(() => app.finishRecording())}>
    <i class="dot"></i>
    <span>{t("macroRecording")}</span>
    <b>{macro.steps.length} {t("macroSteps")}</b>
    <button class="btn-accent" onclick={() => app.finishRecording()}>{t("macroStop")}</button>
</div>

<style>
    .rec {
        position: fixed;
        left: 50%;
        top: 4.5rem;
        z-index: var(--z-bar);
        display: flex;
        align-items: center;
        gap: 0.9rem;
        padding: 0.6rem 1rem;
        transform: translateX(-50%);
        background: var(--panel);
        box-shadow: 0 0 0 1px var(--rule);
    }

    .dot {
        width: 0.6rem;
        height: 0.6rem;
        border-radius: 50%;
        background: var(--danger);
        animation: pulse 1s ease-in-out infinite alternate;
    }

    @keyframes pulse {
        to {
            opacity: 0.3;
        }
    }

    b {
        color: var(--mute);
        font-weight: 500;
    }
</style>
