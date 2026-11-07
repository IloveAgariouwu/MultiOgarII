<script lang="ts">
    import { untrack } from "svelte";
    import { useApp, useGame } from "../../lib/context";
    import { chatVars } from "../../lib/hud-style";
    import type { ChatMessage } from "../../lib/game.svelte";
    import ChatInput from "./ChatInput.svelte";
    import ChatLog from "./ChatLog.svelte";
    import NoticeTick from "./NoticeTick.svelte";

    const app = useApp();
    const game = useGame();
    const t = app.t;
    const chat = app.chat;
    let s = $derived(app.settings);
    let inGame = $derived(!app.menuOpen || app.hud.editing);
    let notice = $derived(app.notices.latest);
    let logShown = $derived(inGame && app.flag("ShowChatBox"));
    let open = $derived(s.ShowUI && (inGame || !!notice));
    const systemNotices = app.notices.feed(4000);
    let seen = 0;

    $effect(() => {
        const fresh: ChatMessage[] = [];
        for (let i = game.chat.length - 1; i >= 0 && game.chat[i].id > seen; i--) fresh.push(game.chat[i]);
        seen = game.chat.at(-1)?.id ?? seen;
        untrack(() => {
            if (!inGame || logShown) return;
            for (const m of fresh.reverse())
                if (m.system) systemNotices.post({ key: "", name: m.name, text: m.text.split("\n")[0], open: null });
        });
    });

    let style = $derived(chatVars(app.colors, chat.size));
</script>

<div class="vars" {style}>
    {#if gMobile}
        {#if s.ShowUI && inGame && logShown}<div class="mlog scrim"><ChatLog /></div>{/if}
    {:else if open}
        <div class="chat scrim" {@attach app.hud.slot("chat")}>
            <div class="strip">
                {#if notice}<NoticeTick {notice} onopen={(n) => app.notices.open(n)} />{/if}
            </div>
            {#if logShown}
                <ChatLog />
                <span
                    class="grip"
                    role="presentation"
                    title={t("chatResize")}
                    onpointerdown={(e) => chat.startResize(e)}
                ></span>
            {/if}
        </div>
    {/if}
</div>

{#if inGame}<ChatInput />{/if}

<style>
    .vars {
        display: contents;
    }

    .chat {
        position: fixed;
        left: 1.4rem;
        top: 1rem;
        z-index: var(--z-chat);
        text-shadow: var(--shadow);
    }

    .mlog {
        --mlog-left: calc(var(--hud-left) + var(--mm-size) + var(--hud-pad) + var(--scrim-x));
        position: fixed;
        top: calc(var(--mm-top) + var(--scrim-y));
        left: var(--mlog-left);
        width: calc(50vw - var(--touch-bar) / 2 - 1.35rem - var(--mlog-left));
        height: calc(var(--mlog-h) - 2 * var(--scrim-y));
        pointer-events: none;
        text-shadow: var(--shadow);
    }

    @media (orientation: portrait) {
        .mlog {
            --mlog-left: calc(var(--hud-left) + var(--scrim-x));
            top: calc(var(--mm-top) + var(--mm-size) + var(--hud-pad) + var(--scrim-y));
            width: calc(100vw - var(--hud-left) - var(--hud-right) - var(--lb-w) - 2 * var(--scrim-x) - var(--hud-pad));
        }
    }

    .grip {
        position: absolute;
        right: -0.7rem;
        bottom: -0.5rem;
        width: 1rem;
        height: 1rem;
        cursor: nwse-resize;
        background: linear-gradient(
            135deg,
            transparent 55%,
            var(--rule-strong) 55%,
            var(--rule-strong) 65%,
            transparent 65%,
            transparent 75%,
            var(--rule-strong) 75%,
            var(--rule-strong) 85%,
            transparent 85%
        );
        opacity: 0;
        transition: opacity 0.2s;
    }

    .chat:hover .grip {
        opacity: 1;
    }

    .strip {
        display: flex;
        align-items: center;
        gap: 0.8rem;
        width: var(--chat-w);
        min-height: var(--hit);
        margin-bottom: 0.4rem;
    }
</style>
