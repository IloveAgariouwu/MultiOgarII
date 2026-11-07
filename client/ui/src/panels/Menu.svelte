<script lang="ts">
    import { untrack } from "svelte";
    import { Spring } from "svelte/motion";
    import { fade } from "svelte/transition";
    import Icon from "../components/Icon.svelte";
    import { useApp, useGame, useSource } from "../lib/context";
    import { population } from "../lib/format";
    import { MenuModel } from "../lib/menu-model.svelte";
    import { unitSkins } from "../lib/profile-fields";
    import { PANEL_OUT } from "../lib/transitions";
    import { skin } from "../lib/skin-images";
    import IdentityFields from "./menu/IdentityFields.svelte";
    import MenuTabs from "./menu/MenuTabs.svelte";
    import SkinFields from "./menu/SkinFields.svelte";

    const WHEEL_STEP = 80;
    const SPRING = { stiffness: 0.12, damping: 0.75 };
    const MAX_TRAVEL = 2;
    const app = useApp();
    const game = useGame();
    const source = useSource();
    const m = new MenuModel(app, game, source);
    const t = app.t;

    let wheel = 0;
    let numberInput: HTMLInputElement;

    const spot = new Spring(0, SPRING);
    const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
    let spotTab = -1;
    let visible = $derived.by(() => {
        const lo = Math.max(0, Math.floor(spot.current)),
            hi = Math.min(m.last, Math.ceil(spot.current));
        return lo === hi ? [lo] : [lo, hi];
    });

    $effect(() => {
        const target = m.index,
            t = m.tab;
        untrack(() => {
            const far = target - spot.current;
            if (still || t !== spotTab) spot.set(target, { instant: true });
            else if (Math.abs(far) > MAX_TRAVEL) spot.set(target - Math.sign(far) * MAX_TRAVEL, { instant: true });
            spotTab = t;
            spot.target = target;
        });
    });

    function onWheel(e: WheelEvent) {
        e.preventDefault();
        const delta = e.deltaMode === WheelEvent.DOM_DELTA_LINE ? e.deltaY * 33 : e.deltaY;
        if (Math.sign(delta) !== Math.sign(wheel)) wheel = 0;
        wheel += delta;
        const steps = Math.trunc(wheel / WHEEL_STEP);
        if (!steps) return;
        wheel -= steps * WHEEL_STEP;
        m.go(m.index + Math.sign(steps));
    }

    function onNumberKey(e: KeyboardEvent) {
        if (e.key === "Enter") {
            const n = parseInt(numberInput.value, 10);
            if (n) m.go(n - 1);
            numberInput.blur();
        } else if (e.key === "Escape") {
            numberInput.blur();
        } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
            e.preventDefault();
            m.go(m.index + (e.key === "ArrowUp" ? -1 : 1));
            queueMicrotask(() => numberInput.select());
        }
    }
</script>

<main class="menu" in:fade={{ duration: 300 }} out:fade={PANEL_OUT}>
    <h1 class="brand">MultiOgarII</h1>

    <button class="stage" title={t("menuScrollHint")} onclick={() => m.openProfiles()} onwheel={onWheel}>
        {#key `${m.tab}|${m.external}`}
            <span class="strip" transition:fade={{ duration: 200 }}>
                {#each visible as i (i)}
                    <span class="pair" style:--d={i - spot.current}>
                        {#each unitSkins(app.profiles.entries[i], m.tab, m.external) as url, u (u)}
                            <span class="c" use:skin={{ url }}></span>
                        {/each}
                    </span>
                {/each}
            </span>
        {/key}
    </button>

    <div class="bar">
        <MenuTabs {m} />
        <span class="sp"></span>
        <div class="nav">
            <button
                class="icon step"
                disabled={m.index === 0}
                aria-label={t("menuPrevProfile")}
                onclick={() => m.go(m.index - 1)}><Icon name="up" /></button
            >
            <input
                class="no"
                bind:this={numberInput}
                value={m.index + 1}
                inputmode="numeric"
                maxlength={String(m.last + 1).length}
                title={m.profile.displayName}
                aria-label={t("menuProfileNumber")}
                onfocus={() => numberInput.select()}
                onblur={() => (numberInput.value = String(m.index + 1))}
                onkeydown={onNumberKey}
            />
            <span class="of">/ {m.last + 1}</span>
            <button
                class="icon step"
                disabled={m.index === m.last}
                aria-label={t("menuNextProfile")}
                onclick={() => m.go(m.index + 1)}><Icon name="down" /></button
            >
            <i class="div"></i>
            <button
                class="icon all"
                title={t("menuAllProfiles")}
                aria-label={t("menuAllProfiles")}
                onclick={() => m.openProfiles()}><Icon name="grid" /></button
            >
            <button
                class="icon all"
                class:on={game.tabsAsUnits}
                title={t("menuTabsAsUnits")}
                aria-label={t("menuTabsAsUnits")}
                aria-pressed={game.tabsAsUnits}
                onclick={() => source.setTabsAsUnits(!game.tabsAsUnits)}><Icon name="swap" /></button
            >
            <button
                class="icon all gear"
                title={t("menuSettings")}
                aria-label={t("menuSettings")}
                onclick={() => app.open("settings")}><Icon name="gear" /></button
            >
        </div>
    </div>

    <SkinFields {m} />
    <IdentityFields {m} />
    {#if app.profileError}<p class="err">{app.profileError}</p>{/if}

    <button class="srv" aria-label={t("menuChangeServer")} onclick={() => app.open("servers")}>
        <b>{app.server.label}</b><span>{population(m.pop)}</span><i class="chev">›</i>
    </button>

    {#if game.closeNotice}<p class="notice">{game.closeNotice.text}</p>{/if}

    <div class="go">
        <button class="btn-accent play" onclick={() => m.play()}>{t("menuPlay")}</button>
        <button class="btn-line spec" onclick={() => m.spectate()}>{t("menuSpectate")}</button>
    </div>
</main>

<style>
    .menu {
        --cell: 15rem;
        position: fixed;
        left: 50%;
        top: 53%;
        transform: translate(-50%, -50%);
        width: calc(var(--cell) * 2 + 1.2rem);
        text-shadow: var(--shadow);
    }

    .brand {
        margin: 0 0 3.6rem;
        font-size: var(--fs-3xl);
        font-weight: 800;
        line-height: 1;
        letter-spacing: 0.06em;
        text-align: center;
    }

    .stage {
        position: relative;
        display: block;
        width: 100%;
        height: var(--cell);
        padding: 0;
        border: 0;
        background: none;
        overflow: clip;
        overflow-clip-margin: 0.2rem;
    }

    .strip {
        position: absolute;
        inset: 0;
    }

    .pair {
        position: absolute;
        inset: 0;
        display: flex;
        justify-content: center;
        gap: 1.2rem;
        transform: translateY(calc(var(--d) * (100% + 1.2rem)));
    }

    .c {
        width: var(--cell);
        height: var(--cell);
        border-radius: 50%;
        box-shadow: 0 0 0 1px var(--rule);
    }

    .nav {
        display: flex;
        align-items: center;
        gap: 0.15rem;
    }

    .div {
        width: 1px;
        height: 1.2rem;
        margin: 0 0.4rem;
        background: var(--rule);
    }

    .step {
        width: var(--hit);
        height: var(--hit);
    }

    .step:disabled {
        opacity: 0.25;
        cursor: default;
    }

    .no {
        width: 2.4rem;
        padding: 0;
        border: 1px solid transparent;
        background: none;
        text-align: center;
        font-size: var(--fs-lg);
        font-weight: 700;
        line-height: 1.1;
        text-shadow: var(--shadow);
        font-variant-numeric: tabular-nums;
    }

    .no:hover {
        border-color: var(--rule);
    }

    .no:focus {
        outline: 0;
        border-color: var(--accent);
        background: var(--field);
        text-shadow: none;
    }

    .of {
        margin-right: 0.2rem;
        color: var(--dim);
        font-size: var(--fs-sm);
    }

    .all {
        width: var(--hit);
        height: var(--hit);
    }

    .all.on {
        color: var(--accent);
        box-shadow: inset 0 -2px var(--accent);
    }

    .bar {
        display: flex;
        align-items: center;
        gap: 1.4rem;
        margin-top: 1rem;
        font-size: var(--fs-md);
    }

    .sp {
        flex: 1;
    }

    .err {
        margin-top: 0.4rem;
        color: var(--danger);
        font-size: var(--fs-xs);
    }

    .srv {
        display: flex;
        align-items: baseline;
        gap: 0.8rem;
        width: 100%;
        margin-top: 1.6rem;
        padding: 0.55rem 0;
        border: 0;
        border-top: 1px solid var(--rule);
        border-bottom: 1px solid var(--rule);
        background: none;
        text-align: left;
        text-shadow: var(--shadow);
    }

    .srv b {
        font-size: var(--fs-lg);
        font-weight: 700;
    }

    .srv span {
        color: var(--mute);
    }

    .chev {
        margin-left: auto;
        color: var(--mute);
        font-size: var(--fs-lg);
        font-style: normal;
    }

    .srv:hover b,
    .srv:hover .chev {
        color: var(--fg);
    }

    .go {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 0.6rem;
        margin-top: 0.8rem;
    }

    .notice {
        margin: 0.5rem 0 0;
        color: var(--danger);
        font-size: var(--fs-sm);
    }

    .play,
    .spec {
        padding: 0.55rem 0;
        font-size: var(--fs-lg);
        font-weight: 700;
        white-space: nowrap;
    }

    .gear :global(svg) {
        transition: transform 0.4s var(--ease);
    }

    .gear:hover :global(svg) {
        transform: rotate(45deg);
    }
</style>
