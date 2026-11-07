<script lang="ts" module>
    import type { IconName } from "../components/Icon.svelte";
    import { QUICK_CHAT_SLOTS, quickChatAction } from "../lib/quick-chat.svelte";

    type Glyph = { icon: IconName } | { text: string } | { icon: IconName; text: string };
    type Key = { hk: string; k: number; x: number; y: number };

    const QUICK_CHAT: Record<string, Glyph> = Object.fromEntries(
        Array.from({ length: QUICK_CHAT_SLOTS }, (_, i): [string, Glyph] => [
            quickChatAction(i),
            { icon: "chat", text: `${i + 1}` },
        ]),
    );
    const GLYPHS: Record<string, Glyph> = {
        hkSplit: { icon: "split" },
        hkFeed: { icon: "feed" },
        hkFeedOne: { icon: "feedOne" },
        hkDoubleSplit: { icon: "double" },
        hkTripleSplit: { text: "×3" },
        hkQuadSplit: { icon: "x16" },
        hkSuperQuadSplit: { text: "S16" },
        hkQuintupleSplit: { text: "×32" },
        hkSextupleSplit: { text: "×64" },
        hkSeptupleSplit: { text: "×128" },
        hkOctupleSplit: { text: "×256" },
        hkTab: { icon: "swap" },
        hkChangeUnit: { icon: "sync" },
        hkInfernoSplit: { icon: "bolt" },
        hkStartNewGame: { icon: "play" },
        hkToggelSpectateTarget: { icon: "trophy" },
        hkQuickReplayCapture: { icon: "record" },
        hkToggleReplayRecording: { icon: "record" },
        hkPlaybackReplay: { icon: "film" },
        hkReplayPlayPause: { icon: "pause" },
        hkReplaySeekLeft: { icon: "stepBack" },
        hkReplaySeekRight: { icon: "stepForward" },
        hkTestSplit: { icon: "split", text: "T" },
        hkOverlapSplit: { icon: "layers" },
        hkOverlapSplitOnce: { icon: "layers", text: "1" },
        hkParallel: { icon: "split", text: "∥" },
        ...QUICK_CHAT,
    };
    const FIXED: Key[] = [
        { hk: "hkSplit", k: 1, x: 0, y: 0 },
        { hk: "hkFeed", k: 0.8, x: 1.12, y: 0.04 },
        { hk: "hkDoubleSplit", k: 0.64, x: 0.1, y: 1.12 },
        { hk: "hkTab", k: 0.64, x: 1.04, y: 0.98 },
        { hk: "hkChangeUnit", k: 0.56, x: 2, y: 1.514 },
        { hk: "hkQuadSplit", k: 0.56, x: 1.514, y: 2 },
    ];
    const SLOT_SPOTS = [
        { x: 2.42, y: 0.22 },
        { x: 2.312, y: 0.9 },
        { x: 0.9, y: 2.312 },
        { x: 0.22, y: 2.42 },
    ];
    const TAB_BADGED = ["hkTab", "hkChangeUnit"];
</script>

<script lang="ts">
    import { onDestroy, untrack } from "svelte";
    import { SvelteMap } from "svelte/reactivity";
    import Icon from "../components/Icon.svelte";
    import { useApp, useGame, useSource } from "../lib/context";
    import { Controls } from "../lib/controls";
    import { argbCss } from "../lib/format";
    import { PERCENT } from "../lib/settings-format";

    const JOY_RADIUS = 48;
    const JOY_ZONE = 0.45;
    const app = useApp();
    const game = useGame();
    const source = useSource();
    for (const hk of app.groups.control) if (!(hk in GLYPHS)) throw new Error(`Touch key ${hk} has no glyph`);
    const controls = new Controls(app, game, source);
    onDestroy(() => controls.releaseAll());
    $effect(() => {
        const root = document.documentElement.style;
        root.setProperty(
            "--touch-top",
            `calc(env(safe-area-inset-bottom) + 0.75rem + ${app.num("TouchButtonSize") * 3}px)`,
        );
        return () => root.removeProperty("--touch-top");
    });
    let barWidth = $state(0);
    $effect(() => {
        const root = document.documentElement.style;
        root.setProperty("--touch-bar", `${barWidth}px`);
        return () => root.removeProperty("--touch-bar");
    });
    const t = app.t;
    const s = $derived(app.settings);

    let joy = $state<{ id: number; x: number; y: number; dx: number; dy: number } | null>(null);
    let followId: number | null = null;
    const touches = new Map<number, { x: number; y: number }>();
    const held = new SvelteMap<number, string>();
    let pinch: { a: number; b: number; d: number } | null = null;
    let leftHanded = $derived(app.flag("TouchLeftHanded"));
    let keys = $derived([
        ...FIXED,
        ...[s.TouchSlot1, s.TouchSlot2, s.TouchSlot3, s.TouchSlot4]
            .map((v, i) => ({ hk: String(v), k: 0.56, ...SLOT_SPOTS[i] }))
            .filter((key) => key.hk),
    ]);
    let pressed = $derived(new Set(held.values()));
    let spectating = $derived(game.spectating);
    $effect(() => {
        if (!spectating && !app.chat.open) return;
        untrack(() => {
            held.clear();
            controls.releaseAll();
        });
    });

    function pressDown(e: PointerEvent & { currentTarget: HTMLButtonElement }, hk: string) {
        e.stopPropagation();
        e.currentTarget.setPointerCapture(e.pointerId);
        if (app.flag("TouchVibrate")) navigator.vibrate?.(10);
        held.set(e.pointerId, hk);
        controls.press(hk, `touch${e.pointerId}`);
    }

    function pressUp(e: PointerEvent) {
        held.delete(e.pointerId);
        controls.release(`touch${e.pointerId}`);
    }

    function aimAt(x: number, y: number) {
        app.touchAim = { x, y };
    }

    function surfaceDown(e: PointerEvent & { currentTarget: HTMLDivElement }) {
        touches.set(e.pointerId, { x: e.clientX, y: e.clientY });
        e.currentTarget.setPointerCapture(e.pointerId);
        if (!joy && followId === null) {
            if (app.flag("TouchFollowFinger")) {
                followId = e.pointerId;
                aimAt(e.clientX, e.clientY);
                return;
            }
            const inZone = leftHanded ? e.clientX > innerWidth * (1 - JOY_ZONE) : e.clientX < innerWidth * JOY_ZONE;
            if (inZone) {
                joy = { id: e.pointerId, x: e.clientX, y: e.clientY, dx: 0, dy: 0 };
                return;
            }
        }
        if (pinch) return;
        followId = null;
        const fingers = [...touches.keys()].filter((id) => id !== joy?.id);
        if (fingers.length >= 2) pinch = { a: fingers[0], b: fingers[1], d: touchDistance(fingers[0], fingers[1]) };
    }

    function touchDistance(a: number, b: number): number {
        const p = touches.get(a),
            q = touches.get(b);
        return p && q ? Math.hypot(p.x - q.x, p.y - q.y) : 0;
    }

    function surfaceMove(e: PointerEvent) {
        if (touches.has(e.pointerId)) touches.set(e.pointerId, { x: e.clientX, y: e.clientY });
        if (pinch && (e.pointerId === pinch.a || e.pointerId === pinch.b)) {
            const d = touchDistance(pinch.a, pinch.b);
            if (d && pinch.d) app.pinchZoom *= d / pinch.d;
            pinch.d = d;
            return;
        }
        if (e.pointerId === followId) return aimAt(e.clientX, e.clientY);
        if (!joy || e.pointerId !== joy.id) return;
        const dx = e.clientX - joy.x,
            dy = e.clientY - joy.y,
            d = Math.hypot(dx, dy),
            k = d > JOY_RADIUS ? JOY_RADIUS / d : 1;
        joy = { ...joy, dx: dx * k, dy: dy * k };
        const reach = Math.min(innerWidth, innerHeight) * 0.45;
        aimAt(innerWidth / 2 + (joy.dx / JOY_RADIUS) * reach, innerHeight / 2 + (joy.dy / JOY_RADIUS) * reach);
    }

    function surfaceUp(e: PointerEvent) {
        touches.delete(e.pointerId);
        if (e.pointerId === followId) followId = null;
        if (joy && e.pointerId === joy.id) joy = null;
        if (pinch && (e.pointerId === pinch.a || e.pointerId === pinch.b)) pinch = null;
    }

    function stopPointerPropagation(e: PointerEvent) {
        e.stopPropagation();
    }
</script>

{#snippet glyph(g: Glyph)}{#if "icon" in g}<Icon name={g.icon} />{/if}{#if "text" in g}<span class:sub={"icon" in g}
            >{g.text}</span
        >{/if}{/snippet}

<div
    class="touch"
    class:left={leftHanded}
    style:--touch-btn="{s.TouchButtonSize}px"
    style:--touch-alpha={app.num("TouchButtonAlpha") / PERCENT}
    style:--touch-color={argbCss(app.colors.clTouchButton)}
    role="presentation"
    onpointerdown={surfaceDown}
    onpointermove={surfaceMove}
    onpointerup={surfaceUp}
    onpointercancel={surfaceUp}
>
    <div class="top" role="toolbar" tabindex="-1" bind:clientWidth={barWidth} onpointerdown={stopPointerPropagation}>
        <button class="icon tb" aria-label={t("menuSettings")} onclick={() => (app.menuOpen = true)}
            ><Icon name="menu" /></button
        >
        <button
            class="icon tb"
            class:on={app.chat.open}
            aria-label={t("chatOpenDock")}
            onclick={() => (app.chat.open = !app.chat.open)}><Icon name="chat" /></button
        >
        <button
            class="icon tb"
            class:on={game.tab2Connected}
            aria-label={t("tabN").replace("%s", "2")}
            onclick={() => source.refreshTab(1)}><Icon name="two" /></button
        >
    </div>

    {#if !app.flag("TouchFollowFinger")}
        <div class="joy" class:held={joy !== null} style:left={joy && `${joy.x}px`} style:top={joy && `${joy.y}px`}>
            <span class="knob" style:translate={joy && `${joy.dx}px ${joy.dy}px`}></span>
        </div>
    {/if}

    {#if spectating}
        <div class="spec" role="toolbar" tabindex="-1" onpointerdown={stopPointerPropagation}>
            <div class="modes">
                <button class="icon mode" onclick={() => source.watch(false)}
                    ><Icon name="trophy" />{t("touchTop")}</button
                >
                <button class="icon mode" onclick={() => source.watch(true)}
                    ><Icon name="move" />{t("touchFree")}</button
                >
            </div>
        </div>
        <button
            class="icon play btn-accent"
            aria-label={t("menuPlay")}
            onpointerdown={stopPointerPropagation}
            onclick={() => source.play()}><Icon name="play" /></button
        >
    {:else if !app.chat.open}
        <div class="actions">
            {#each keys as key (key.hk)}<button
                    class="icon key"
                    class:down={pressed.has(key.hk)}
                    style:--k={key.k}
                    style:--x={key.x}
                    style:--y={key.y}
                    aria-label={app.lookup(key.hk)}
                    onpointerdown={(e) => pressDown(e, key.hk)}
                    onpointerup={pressUp}
                    onpointercancel={pressUp}
                    >{@render glyph(GLYPHS[key.hk])}{#if game.tab2Connected && TAB_BADGED.includes(key.hk)}<b
                            class="badge">{game.activeTab + 1}</b
                        >{/if}</button
                >{/each}
        </div>
    {/if}
</div>

<style>
    .touch {
        --frost: color-mix(in srgb, var(--touch-color) calc(var(--touch-alpha) * 100%), transparent);
        --edge: 0.75rem;
        --joy: calc(var(--touch-btn) * 1.77);
        position: fixed;
        inset: 0;
        z-index: var(--z-touch);
        touch-action: none;
        user-select: none;
        -webkit-user-select: none;
    }

    button {
        -webkit-tap-highlight-color: transparent;
    }

    .top {
        position: absolute;
        top: calc(env(safe-area-inset-top) + 0.5rem);
        left: 50%;
        transform: translateX(-50%);
        display: flex;
        gap: 0.5rem;
    }

    @media (orientation: portrait) {
        .top {
            left: calc(env(safe-area-inset-left) + var(--edge));
            transform: none;
        }
    }

    .tb {
        width: calc(var(--hit) * 1.25);
        height: calc(var(--hit) * 1.25);
        border-radius: 50%;
        background: color-mix(in srgb, var(--bg) 70%, transparent);
        backdrop-filter: blur(14px) saturate(1.15);
        color: var(--fg);
    }

    .icon.on {
        color: var(--accent);
    }

    .tb:active,
    .mode:active {
        background: var(--accent);
        color: var(--on-accent);
    }

    .joy {
        position: absolute;
        left: calc(env(safe-area-inset-left) + var(--edge) + var(--joy) / 2);
        top: calc(100% - env(safe-area-inset-bottom) - var(--edge) - var(--joy) / 2);
        width: var(--joy);
        height: var(--joy);
        translate: -50% -50%;
        border-radius: 50%;
        background: color-mix(in srgb, var(--fg) 12%, transparent);
        pointer-events: none;
    }

    .touch.left .joy {
        left: calc(100% - env(safe-area-inset-right) - var(--edge) - var(--joy) / 2);
    }

    .joy.held {
        background: color-mix(in srgb, var(--fg) 18%, transparent);
    }

    .knob {
        position: absolute;
        left: 50%;
        top: 50%;
        width: calc(var(--joy) * 0.42);
        height: calc(var(--joy) * 0.42);
        margin: calc(var(--joy) * -0.21) 0 0 calc(var(--joy) * -0.21);
        border-radius: 50%;
        background: color-mix(in srgb, var(--fg) 45%, transparent);
    }

    .actions {
        position: absolute;
        right: calc(env(safe-area-inset-right) + var(--edge));
        bottom: calc(env(safe-area-inset-bottom) + var(--edge));
        width: calc(var(--touch-btn) * 3);
        height: calc(var(--touch-btn) * 3);
        pointer-events: none;
    }

    .touch.left .actions {
        right: auto;
        left: calc(env(safe-area-inset-left) + var(--edge));
    }

    .key {
        position: absolute;
        right: calc(var(--touch-btn) * var(--x));
        bottom: calc(var(--touch-btn) * var(--y));
        width: calc(var(--touch-btn) * var(--k));
        height: calc(var(--touch-btn) * var(--k));
        border-radius: 50%;
        background: var(--frost);
        backdrop-filter: blur(14px) saturate(1.15);
        color: var(--fg);
        font-size: var(--fs-sm);
        font-weight: 700;
        pointer-events: auto;
        touch-action: none;
        transition:
            transform 0.05s,
            background-color 0.05s;
    }

    .touch.left .key {
        right: auto;
        left: calc(var(--touch-btn) * var(--x));
    }

    .key :global(svg) {
        width: calc(var(--touch-btn) * var(--k) * 0.42);
        height: calc(var(--touch-btn) * var(--k) * 0.42);
    }

    .key.down {
        transform: scale(0.9);
        background: var(--accent);
        color: var(--on-accent);
    }

    .sub {
        position: absolute;
        right: 22%;
        bottom: 18%;
        font-size: var(--fs-xs);
    }

    .badge {
        position: absolute;
        right: 0;
        top: 0;
        min-width: 1rem;
        border-radius: 0.5rem;
        background: var(--accent);
        color: var(--on-accent);
        font-size: var(--fs-xs);
        line-height: 1rem;
        text-align: center;
    }

    .spec {
        position: absolute;
        left: 50%;
        bottom: calc(env(safe-area-inset-bottom) + var(--edge));
        transform: translateX(-50%);
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 0.4rem;
    }

    @media (orientation: portrait) {
        .spec {
            bottom: calc(env(safe-area-inset-bottom) + var(--edge) * 2 + var(--joy));
        }
    }

    .modes {
        display: flex;
        gap: 0.4rem;
    }

    .mode {
        display: flex;
        gap: 0.3rem;
        height: calc(var(--hit) * 1.5);
        padding: 0 0.9rem;
        border-radius: calc(var(--hit) * 0.75);
        background: color-mix(in srgb, var(--bg) 70%, transparent);
        backdrop-filter: blur(14px) saturate(1.15);
        color: var(--fg);
        font-size: var(--fs-sm);
        white-space: nowrap;
    }

    .mode :global(svg) {
        width: var(--icon-sm);
        height: var(--icon-sm);
    }

    .play {
        position: absolute;
        right: calc(env(safe-area-inset-right) + var(--edge));
        bottom: calc(env(safe-area-inset-bottom) + var(--edge));
        width: calc(var(--hit) * 2.25);
        height: calc(var(--hit) * 1.5);
        border-radius: calc(var(--hit) * 0.75);
        color: var(--on-accent);
    }

    .play:hover {
        color: var(--on-accent);
    }

    .play:active {
        transform: scale(0.94);
    }

    .touch.left .play {
        right: auto;
        left: calc(env(safe-area-inset-left) + var(--edge));
    }
</style>
