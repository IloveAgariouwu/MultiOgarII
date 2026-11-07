<script lang="ts">
    import { fade } from "svelte/transition";
    import Icon from "../../components/Icon.svelte";
    import { useApp, useGame, useSource } from "../../lib/context";
    import { population } from "../../lib/format";
    import { MenuModel } from "../../lib/menu-model.svelte";
    import { unitSkins } from "../../lib/profile-fields";
    import { skin } from "../../lib/skin-images";
    import { PANEL_OUT } from "../../lib/transitions";
    import IdentityFields from "../menu/IdentityFields.svelte";
    import MenuTabs from "../menu/MenuTabs.svelte";
    import SkinFields from "../menu/SkinFields.svelte";

    const app = useApp();
    const game = useGame();
    const source = useSource();
    const m = new MenuModel(app, game, source);
    const t = app.t;

    let editing = $state(false);
</script>

<main class="mm" in:fade={{ duration: 300 }} out:fade={PANEL_OUT}>
    <header class="top">
        <h1 class="brand">MultiOgarII</h1>
        <span class="sp"></span>
        <button class="icon tb" aria-label={t("menuSettings")} onclick={() => app.open("settings")}
            ><Icon name="gear" /></button
        >
        <button class="icon tb" aria-label={t("menuAllProfiles")} onclick={() => m.openProfiles()}
            ><Icon name="grid" /></button
        >
    </header>

    <section class="left">
        <div class="cells">
            <button class="pair" aria-label={t("menuScrollHint")} onclick={() => (m.tab = 1 - m.tab)}>
                {#key `${m.index}|${m.tab}|${m.external}`}
                    {#each unitSkins(m.profile, m.tab, m.external) as url, u (u)}
                        {#if u === 1}<i class="gap"></i>{/if}
                        <span class="c" use:skin={{ url }}></span>
                    {/each}
                {/key}
            </button>
            <button
                class="icon units"
                class:on={game.tabsAsUnits}
                aria-label={t("menuTabsAsUnits")}
                aria-pressed={game.tabsAsUnits}
                onclick={() => source.setTabsAsUnits(!game.tabsAsUnits)}><Icon name="swap" /></button
            >
            <button
                class="icon pen"
                class:on={editing}
                aria-label={t("menuEditSkins")}
                aria-expanded={editing}
                onclick={() => (editing = !editing)}><Icon name="edit" /></button
            >
        </div>

        <div class="bar">
            <MenuTabs {m} />
            <span class="sp"></span>
            <button
                class="icon step"
                disabled={m.index === 0}
                aria-label={t("menuPrevProfile")}
                onclick={() => m.go(m.index - 1)}><Icon name="left" /></button
            >
            <span class="no">{m.index + 1}<small>/ {m.last + 1}</small></span>
            <button
                class="icon step"
                disabled={m.index === m.last}
                aria-label={t("menuNextProfile")}
                onclick={() => m.go(m.index + 1)}><Icon name="right" /></button
            >
        </div>

        {#if editing}
            <SkinFields {m} />
        {/if}
    </section>

    <section class="right">
        <button class="srv" aria-label={t("menuChangeServer")} onclick={() => app.open("servers")}>
            <i class="dot" class:live={m.pop}></i>
            <b>{app.server.label}</b>
            <span>{population(m.pop)}</span>
            <Icon name="right" />
        </button>

        {#if game.closeNotice}<p class="notice">{game.closeNotice.text}</p>{/if}

        <IdentityFields {m} />
        {#if app.profileError}<p class="err">{app.profileError}</p>{/if}

        <div class="go">
            <button class="btn-accent play" onclick={() => m.play()}><Icon name="play" />{t("menuPlay")}</button>
            <button class="spec" aria-label={t("menuSpectate")} onclick={() => m.spectate()}><Icon name="eye" /></button
            >
        </div>
    </section>
</main>

<style>
    .mm {
        --row: calc(var(--hit) * 1.5);
        --cell: calc(var(--hit) * 3.4);
        --gut: 1rem;
        position: fixed;
        inset: 0;
        display: flex;
        flex-direction: column;
        gap: 0.9rem;
        padding: max(0.6rem, env(safe-area-inset-top)) max(var(--gut), env(safe-area-inset-right))
            max(0.8rem, env(safe-area-inset-bottom)) max(var(--gut), env(safe-area-inset-left));
        overflow-y: auto;
        overscroll-behavior: contain;
        text-shadow: var(--shadow);
    }

    .left,
    .right {
        display: contents;
    }

    .top {
        order: 0;
        display: flex;
        align-items: center;
        gap: 0.2rem;
        min-height: var(--row);
    }

    .srv {
        order: 1;
    }

    .cells {
        order: 2;
    }

    .bar {
        order: 3;
    }

    .go {
        order: 6;
    }

    .err {
        order: 7;
        margin: 0;
        color: var(--danger);
        font-size: var(--fs-xs);
    }

    .brand {
        margin: 0;
        font-size: var(--fs-lg);
        font-weight: 800;
        line-height: 1;
        letter-spacing: 0.06em;
    }

    .sp {
        flex: 1;
    }

    .tb {
        width: calc(var(--hit) * 1.25);
        height: calc(var(--hit) * 1.25);
    }

    .srv {
        display: flex;
        align-items: center;
        gap: 0.7rem;
        width: 100%;
        min-height: var(--row);
        padding: 0;
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
        margin-left: auto;
        color: var(--mute);
        font-variant-numeric: tabular-nums;
    }

    .srv :global(svg) {
        width: var(--icon);
        height: var(--icon);
        fill: none;
        stroke: var(--mute);
        stroke-width: 1.7;
    }

    .dot {
        width: 0.5rem;
        height: 0.5rem;
        border-radius: 50%;
        background: var(--dim);
    }

    .dot.live {
        background: var(--ok);
    }

    .cells {
        position: relative;
        display: grid;
        grid-template-columns: minmax(var(--row), 1fr) auto minmax(var(--row), 1fr);
        margin-top: auto;
        align-items: center;
        min-height: calc(var(--cell) + 1rem);
    }

    .pair {
        grid-column: 2;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 0;
        border: 0;
        background: none;
    }

    .c {
        width: var(--cell);
        height: var(--cell);
        border-radius: 50%;
        box-shadow: 0 0 0 1px var(--rule);
    }

    .units {
        position: absolute;
        left: 50%;
        top: 50%;
        width: var(--row);
        height: var(--row);
        transform: translate(-50%, -50%);
    }

    .units.on,
    .pen.on {
        color: var(--accent);
    }

    .units.on {
        box-shadow: inset 0 -2px var(--accent);
    }

    .pen {
        grid-column: 3;
        justify-self: end;
        width: var(--row);
        height: var(--row);
    }

    .bar {
        display: flex;
        align-items: center;
        gap: 1.1rem;
        min-height: var(--row);
        font-size: var(--fs-md);
    }

    .step {
        width: var(--row);
        height: var(--row);
    }

    .step:disabled {
        opacity: 0.25;
    }

    .no {
        min-width: 3.4rem;
        font-size: var(--fs-lg);
        font-weight: 700;
        text-align: center;
        font-variant-numeric: tabular-nums;
        white-space: nowrap;
    }

    .no small {
        margin-left: 0.3rem;
        color: var(--dim);
        font-size: var(--fs-sm);
        font-weight: 500;
    }

    .go {
        display: flex;
        flex-wrap: wrap;
        gap: 0.5rem;
    }

    .notice {
        order: 1;
        margin: 0;
        color: var(--danger);
        font-size: var(--fs-sm);
    }

    .play {
        display: flex;
        flex: 1;
        align-items: center;
        justify-content: center;
        gap: 0.6rem;
        height: var(--row);
        font-size: var(--fs-lg);
        white-space: nowrap;
    }

    .play :global(svg),
    .spec :global(svg) {
        width: var(--icon);
        height: var(--icon);
        stroke: currentColor;
        stroke-width: 1.7;
        stroke-linejoin: round;
    }

    .play :global(svg) {
        fill: currentColor;
    }

    .spec {
        display: grid;
        place-items: center;
        width: calc(var(--hit) * 2.25);
        height: var(--row);
        padding: 0;
        border: 0;
        background: color-mix(in srgb, var(--fg) 12%, transparent);
        color: var(--fg);
    }

    .spec :global(svg) {
        fill: none;
    }

    .spec:active {
        color: var(--accent);
    }

    @media (orientation: landscape) {
        .mm {
            display: grid;
            grid-template-columns: minmax(0, 0.85fr) minmax(0, 1fr);
            grid-template-rows: auto minmax(0, 1fr);
            column-gap: 1.6rem;
            row-gap: 0.4rem;
            overflow: hidden;
        }

        .top {
            grid-column: 1 / -1;
        }

        .left,
        .right {
            display: flex;
            flex-direction: column;
            gap: 0.6rem;
            min-height: 0;
        }

        .left {
            justify-content: center;
            container-type: inline-size;
        }

        .right {
            overflow-y: auto;
            overscroll-behavior: contain;
        }

        .cells {
            --cell: min(calc(var(--hit) * 3.4), calc((100cqi - 3 * var(--row)) / 2));
            margin-top: 0;
        }

        .go {
            position: sticky;
            bottom: 0;
            z-index: 1;
            backdrop-filter: blur(14px) saturate(1.15);
            background: var(--backdrop);
        }
    }
    .gap {
        width: calc(var(--hit) * 1.5);
    }
</style>
