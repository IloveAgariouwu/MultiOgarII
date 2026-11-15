<script lang="ts">
    let { checked, label, onchange }: { checked: boolean; label: string; onchange: (v: boolean) => void } = $props();
</script>

<button class="tg" role="switch" aria-checked={checked} aria-label={label} onclick={() => onchange(!checked)}></button>

<style>
    .tg {
        position: relative;
        isolation: isolate;
        width: 2.8rem;
        height: 1.3rem;
        padding: 0;
        border: 1px solid var(--rule);
        background: var(--field);
        transition: border-color var(--switch-time) ease-out;
        zoom: var(--text);
    }

    .tg::before {
        content: "";
        position: absolute;
        inset: -1px;
        z-index: -1;
        background: var(--switch-on);
        opacity: 0;
        transition: opacity var(--switch-time) ease-out;
    }

    .tg::after {
        content: "";
        position: absolute;
        top: 0.15rem;
        left: 0.15rem;
        width: 1rem;
        height: 0.85rem;
        background: var(--mute);
        transition:
            transform var(--switch-time) var(--ease),
            width 0.12s ease-out,
            background-color var(--switch-time) ease-out;
    }

    .tg:hover {
        border-color: var(--rule-strong);
    }

    .tg:hover::after {
        background: var(--fg);
    }

    .tg:active::after {
        width: 1.25rem;
    }

    .tg[aria-checked="true"] {
        border-color: var(--on-line);
    }

    .tg[aria-checked="true"]::before {
        opacity: 1;
    }

    .tg[aria-checked="true"]::after {
        transform: translateX(1.45rem);
        background: var(--knob-on);
    }

    .tg[aria-checked="true"]:active::after {
        transform: translateX(1.2rem);
    }

    :global(html.mobile) .tg {
        width: calc(var(--hit) * 1.75);
        height: var(--hit);
        border: 0;
        background: color-mix(in srgb, var(--fg) 12%, transparent);
    }

    :global(html.mobile) .tg::before {
        inset: 0;
    }

    :global(html.mobile) .tg::after {
        top: 0.25rem;
        left: 0.25rem;
        width: calc(var(--hit) - 0.5rem);
        height: calc(var(--hit) - 0.5rem);
    }

    :global(html.mobile) .tg:active::after {
        width: calc(var(--hit) - 0.25rem);
    }

    :global(html.mobile) .tg[aria-checked="true"]::after {
        transform: translateX(calc(var(--hit) * 0.75));
    }

    :global(html.mobile) .tg[aria-checked="true"]:active::after {
        transform: translateX(calc(var(--hit) * 0.75 - 0.25rem));
    }
</style>
