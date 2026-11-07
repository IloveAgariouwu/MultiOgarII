<script lang="ts">
    import { tick } from "svelte";
    import { fly } from "svelte/transition";
    import { selfNames } from "../../lib/chat-people";
    import { useApp, useGame } from "../../lib/context";
    import { hourMinute } from "../../lib/format";
    import { MESSAGE_IN } from "../../lib/transitions";
    import { localizeQuickChat } from "../../lib/quick-chat.svelte";

    const MOBILE_ROWS = 3;
    const app = useApp();
    const game = useGame();
    const lookup = app.lookup;
    const chat = app.chat;
    let list: HTMLDivElement;
    let s = $derived(app.settings);
    let selves = $derived(selfNames(app.profiles));
    let shown = $derived(game.chat);
    let first = $state(0);
    let settled = $state(0);
    let pinned = true;
    let tail = $derived(shown.slice(gMobile ? -MOBILE_ROWS : -app.num("ChatMaxMessages")));
    let visible = $derived(first ? shown.filter((m) => m.id >= first) : tail);

    $effect(() => {
        if (visible.length && pinned) list.scrollTop = list.scrollHeight;
    });

    function followNewMessages(content: HTMLDivElement) {
        const resize = new ResizeObserver(() => {
            if (pinned) list.scrollTop = list.scrollHeight;
        });
        resize.observe(content);
        return () => resize.disconnect();
    }

    async function onscroll() {
        pinned = list.scrollHeight - list.scrollTop - list.clientHeight <= 8;
        if (pinned || !visible.length) {
            first = 0;
            return;
        }
        first ||= visible[0].id;
        if (list.scrollTop > 0 || visible[0] === shown[0]) return;
        const height = list.scrollHeight;
        settled = shown[shown.length - 1].id;
        first = shown[Math.max(0, shown.indexOf(visible[0]) - app.num("ChatMaxMessages"))].id;
        await tick();
        list.scrollTop = list.scrollHeight - height;
    }
</script>

<div class="log" bind:this={list} {onscroll}>
    <div {@attach followNewMessages}>
        {#each visible as m (m.id)}
            {@const quick = m.system ? null : localizeQuickChat(m.text, lookup)}
            <div
                class="msg"
                class:sys={m.system}
                class:mention={s.ChatMentionNotify && chat.mentionsMe(m, selves)}
                in:fly={{ ...MESSAGE_IN, duration: m.id > settled ? MESSAGE_IN.duration : 0 }}
            >
                <span class="time">{hourMinute(m.time)}</span>
                <b class="sender" style:color={m.system ? m.color || undefined : undefined}>{m.name}</b>
                {#if quick !== null}
                    <span class="text" title={m.text}>{quick}</span>
                {:else}
                    <span class="text">{m.text}</span>
                {/if}
            </div>
        {/each}
    </div>
</div>

<style>
    .log {
        width: var(--chat-w);
        user-select: text;
        height: var(--log-h);
        overflow-y: auto;
        padding: 0.4rem 0.6rem;
        color: var(--chat-text);
        scrollbar-width: none;
    }

    .msg {
        margin: 0.1rem 0;
        font-size: var(--fs-sm);
        line-height: 1.35;
        word-break: break-word;
    }

    .time {
        margin-right: 0.4rem;
        color: var(--chat-time);
        font-size: var(--fs-xs);
    }

    b {
        margin-right: 0.4rem;
        font-weight: 600;
    }

    b.sender {
        color: var(--chat-name);
    }

    .mention {
        background: color-mix(in srgb, var(--accent) 16%, transparent);
    }

    .sys .text {
        white-space: pre-line;
    }

    :global(html.mobile) .log {
        width: 100%;
        height: 100%;
        overflow: hidden;
        padding: 0;
        user-select: none;
    }

    :global(html.mobile) .msg {
        margin: 0;
        overflow: hidden;
        white-space: nowrap;
        text-overflow: ellipsis;
        word-break: normal;
    }

    :global(html.mobile) .time {
        display: none;
    }

    :global(html.mobile) .sys .text {
        white-space: nowrap;
    }
</style>
