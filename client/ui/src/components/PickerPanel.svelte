<script lang="ts">
    import type { Snippet } from "svelte";
    import { fade, fly } from "svelte/transition";
    import { PANEL_IN, PANEL_OUT } from "../lib/transitions";
    import { useApp } from "../lib/context";
    import { autofocus } from "../lib/focus";

    let {
        title,
        query = $bindable(""),
        placeholder,
        closeLabel,
        onclose,
        tools,
        children,
    }: {
        title: string;
        query?: string;
        placeholder?: string;
        closeLabel: string;
        onclose: () => void;
        tools?: Snippet;
        children: Snippet;
    } = $props();
    const app = useApp();
    const uid = $props.id();
</script>

<div
    class="picker"
    role="dialog"
    aria-labelledby="{uid}-title"
    in:fly={PANEL_IN}
    out:fade={PANEL_OUT}
    {@attach app.layer(onclose)}
>
    <div class="phead">
        <h2 id="{uid}-title">{title}</h2>
        {#if placeholder}<input class="f q" type="search" {placeholder} bind:value={query} {@attach autofocus} />{/if}
        {@render tools?.()}
        <span class="sp"></span>
        <button class="btn-line close" onclick={onclose}>{closeLabel}{gMobile ? "" : " Esc"}</button>
    </div>
    <div class="body">{@render children()}</div>
</div>

<style>
    .picker {
        position: fixed;
        left: 50%;
        top: 50%;
        width: min(78rem, 86vw);
        height: min(46rem, 78vh);
        display: flex;
        flex-direction: column;
        transform: translate(-50%, -50%);
        text-shadow: var(--shadow);
    }

    .phead {
        display: flex;
        align-items: center;
        gap: 1.2rem;
        padding-bottom: 0.9rem;
        border-bottom: 1px solid var(--rule);
    }

    h2 {
        margin: 0;
        font-size: var(--fs-2xl);
    }

    .q {
        width: 22rem;
    }

    .sp {
        flex: 1;
    }

    .close {
        padding: 0.3rem 0.8rem;
        color: var(--mute);
    }

    .body {
        flex: 1;
        min-height: 0;
        overflow: auto;
        padding-top: 1rem;
    }

    :global(html.mobile) .picker {
        inset: 0;
        width: auto;
        height: auto;
        padding: max(0.6rem, env(safe-area-inset-top)) max(1rem, env(safe-area-inset-right)) env(safe-area-inset-bottom)
            max(1rem, env(safe-area-inset-left));
        transform: none;
    }

    :global(html.mobile) .phead {
        flex-wrap: wrap;
        gap: 0.5rem 1rem;
        padding-bottom: 0.6rem;
    }

    :global(html.mobile) h2 {
        font-size: var(--fs-lg);
        white-space: nowrap;
    }

    :global(html.mobile) .q {
        order: 1;
        flex: 1 0 100%;
        width: auto;
        height: calc(var(--hit) * 1.5);
    }

    :global(html.mobile) .close {
        min-height: calc(var(--hit) * 1.25);
        white-space: nowrap;
    }
</style>
