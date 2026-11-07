<script lang="ts">
    import { parseCommand } from "../../lib/chat-commands";
    import { useApp, useGame, useSource } from "../../lib/context";

    const EMOJI = [
        "😀",
        "😂",
        "🤣",
        "😊",
        "😍",
        "😘",
        "😎",
        "🤔",
        "😏",
        "😭",
        "😡",
        "😱",
        "👍",
        "👎",
        "👏",
        "🙏",
        "💪",
        "🔥",
        "💯",
        "❤️",
        "💔",
        "✨",
        "🎉",
        "⭐",
        "🍬",
        "🍰",
        "☕",
        "🐱",
        "🐶",
        "🦊",
        "⚔️",
        "🛡️",
        "👑",
        "💀",
        "👻",
        "🤡",
    ];
    const app = useApp();
    const game = useGame();
    const source = useSource();
    const t = app.t;

    let input: HTMLInputElement;
    let text = $state("");
    let emojiOpen = $state(false);

    let keyboard = $state(0);

    $effect(() => {
        if (app.chat.open) input.focus();
    });

    function aboveKeyboard() {
        const view = visualViewport;
        if (!view) return;
        const fit = () => (keyboard = Math.max(0, innerHeight - view.height - view.offsetTop));
        fit();
        view.addEventListener("resize", fit);
        view.addEventListener("scroll", fit);
        return () => {
            view.removeEventListener("resize", fit);
            view.removeEventListener("scroll", fit);
        };
    }

    function sendInput(raw: string) {
        const cmd = parseCommand(raw);
        switch (cmd.kind) {
            case "bench":
                if (!cmd.report) game.bench = !game.bench;
                return game.system(
                    cmd.report ? `${t("statFps")} ${game.stats.fps}` : t(game.bench ? "benchOn" : "benchOff"),
                    t("chatSenderBench"),
                );
            case "say":
                return source.say(cmd.text);
        }
    }

    function onkeydown(e: KeyboardEvent) {
        e.stopPropagation();
        if (e.key === "Enter") {
            e.preventDefault();
            const value = text.trim();
            text = "";
            app.chat.open = false;
            if (value) sendInput(value);
        }
    }

    function insertEmoji(emoji: string) {
        const at = input.selectionStart ?? text.length;
        text = text.slice(0, at) + emoji + text.slice(input.selectionEnd ?? at);
        requestAnimationFrame(() => (input.focus(), input.setSelectionRange(at + emoji.length, at + emoji.length)));
    }
</script>

<div
    class="line"
    class:hidden={!app.chat.open && !app.hud.editing}
    style:--keyboard="{keyboard}px"
    {@attach app.hud.slot("input")}
    {@attach gMobile && app.chat.open && aboveKeyboard}
>
    <div class="box">
        <input class="f" bind:this={input} bind:value={text} {onkeydown} spellcheck="false" />
    </div>
    <div class="emo">
        <button
            class="emo-btn"
            title={t("chatEmoji")}
            aria-label={t("chatEmoji")}
            aria-expanded={emojiOpen}
            onclick={() => (emojiOpen = !emojiOpen)}>🙂</button
        >
        {#if emojiOpen}
            <div class="emo-grid" role="listbox">
                {#each EMOJI as e (e)}<button onclick={() => insertEmoji(e)}>{e}</button>{/each}
            </div>
        {/if}
    </div>
</div>

<style>
    .line {
        position: fixed;
        left: 50%;
        bottom: 50px;
        transform: translateX(-50%);
        display: flex;
        align-items: center;
        gap: 0.4rem;
        width: 24rem;
        z-index: var(--z-chat);
    }

    .line.hidden {
        display: none;
    }

    :global(html.mobile) .line {
        left: max(var(--hud-pad), env(safe-area-inset-left));
        right: max(var(--hud-pad), env(safe-area-inset-right));
        bottom: calc(var(--keyboard) + max(var(--hud-pad), env(safe-area-inset-bottom)));
        width: auto;
        transform: none;
        z-index: var(--z-hud);
        gap: 0.6rem;
        padding: 0.5rem 0.6rem;
        backdrop-filter: blur(14px) saturate(1.15);
        background: var(--backdrop);
    }

    :global(html.mobile) .box .f {
        min-height: var(--hit);
    }

    .box {
        flex: 1;
    }

    .box .f {
        padding: 0.35rem 6px;
    }

    .emo {
        position: relative;
    }

    .emo-btn {
        padding: 0 0.3rem;
        border: 0;
        background: none;
        font-size: var(--fs-md);
        filter: grayscale(1);
        transition: filter 0.15s;
    }

    .emo-btn:hover,
    .emo-btn[aria-expanded="true"] {
        filter: none;
    }

    .emo-grid {
        position: absolute;
        right: 0;
        bottom: calc(100% + 0.5rem);
        display: grid;
        grid-template-columns: repeat(8, calc(2rem * var(--text)));
        gap: 2px;
        padding: 0.4rem;
        background: var(--panel);
        box-shadow: 0 0 0 1px var(--rule);
    }

    .emo-grid button {
        height: calc(2rem * var(--text));
        padding: 0;
        border: 0;
        background: none;
        font-size: var(--fs-lg);
        transition: background-color 0.15s;
    }

    .emo-grid button:hover {
        background: color-mix(in srgb, var(--fg) 10%, transparent);
    }
</style>
