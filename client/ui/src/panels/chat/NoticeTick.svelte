<script lang="ts">
    import { fly } from "svelte/transition";
    import type { Notice } from "../../lib/notices.svelte";
    import { MESSAGE_IN } from "../../lib/transitions";

    let { notice, onopen }: { notice: Notice; onopen: (n: Notice) => void } = $props();
</script>

{#key notice.id}
    {@const n = notice}
    <button class="tick" in:fly={MESSAGE_IN} onclick={() => onopen(n)}>
        {#if n.name}<b>{n.name}</b>{/if}<span class="text">{n.text}</span>
    </button>
{/key}

<style>
    .tick {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        min-width: 0;
        max-width: 36rem;
        padding: 0;
        border: 0;
        background: none;
        color: var(--chat-text);
        text-align: left;
        text-shadow: var(--shadow);
        white-space: nowrap;
    }

    .tick b {
        flex: none;
        color: var(--chat-name);
        font-weight: 600;
    }

    .tick .text {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
    }
</style>
