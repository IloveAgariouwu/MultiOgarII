<script lang="ts">
    import Icon from "../../components/Icon.svelte";
    import { useApp, useGame, useHudPlacement, useSource } from "../../lib/context";
    import { mass } from "../../lib/format";
    import { DEAD_POSITION } from "../../lib/game.svelte";
    import { skin } from "../../lib/skin-images";

    const SPLIT_MAX = 16;
    const app = useApp();
    const game = useGame();
    const source = useSource();
    const placement = useHudPlacement();
    const t = app.t;

    let main = $derived(app.profiles.entries[app.profiles.main]);
    let second = $derived(app.profiles.entries[app.profiles.second]);
    let tabs = $derived([
        {
            skin: main.skinUrl,
            name: main.name,
            pos: game.self.pos1,
            split: game.self.split1,
            connected: true,
            spectating: game.tabs.tabs[0].phase === "spectating",
        },
        {
            skin: second.tabSkinUrl,
            name: second.tabName,
            pos: game.self.pos2,
            split: game.self.split2,
            connected: game.tab2Connected,
            spectating: game.tabs.tabs[1].phase === "spectating",
        },
    ]);
</script>

<section class="me scrim" {@attach placement("self")}>
    <div class="score">
        <b>{mass(game.self.score)}</b>
        <span>{t("hudMax")} {mass(game.self.max)}</span>
    </div>
    {#each tabs as tab, i (i)}
        {@const tabName = t("tabN").replace("%s", String(i + 1))}
        <div class="tab" class:on={game.activeTab === i} class:second={i === 1}>
            <span class="c" use:skin={{ url: tab.skin }}></span>
            <span class="nm">{tab.name}</span>
            <span class="pos"
                >{tab.spectating ? t("hudSpectating") : tab.pos && tab.pos !== DEAD_POSITION ? tab.pos : "—"}</span
            >
            <span class="split" title={t("hudCellCount")}>{tab.split}<small>/{SPLIT_MAX}</small></span>
            <button
                class="icon sync"
                title={t(tab.connected ? "hudResync" : "hudConnectTab2")}
                aria-label={`${t(tab.connected ? "hudResync" : "hudConnectTab2")} ${tabName}`}
                onclick={() => source.refreshTab(i === 0 ? 0 : 1)}><Icon name="sync" /></button
            >
            <button
                class="icon sync"
                class:gone={!tab.connected}
                disabled={!tab.connected}
                title={t("hudDisconnect")}
                aria-label={`${t("hudDisconnect")} ${tabName}`}
                onclick={() => source.closeTab(i === 0 ? 0 : 1)}><Icon name="close" /></button
            >
        </div>
    {/each}
</section>

<style>
    .me {
        --tab: var(--accent);
        position: absolute;
        right: 2rem;
        bottom: 22rem;
        width: var(--hud-w);
        font-size: var(--fs-sm);
        font-variant-numeric: tabular-nums;
    }

    .score {
        display: flex;
        align-items: baseline;
        justify-content: space-between;
        margin-bottom: 0.35rem;
    }

    .score b {
        font-size: var(--fs-md);
        line-height: 1.1;
    }

    .score span {
        color: var(--mute);
        font-size: var(--fs-xs);
    }

    .tab {
        display: grid;
        grid-template-columns: 1.45rem minmax(0, 1fr) auto calc(2.6rem * var(--text)) 1.5rem 1.5rem;
        column-gap: 0.55rem;
        align-items: center;
        padding: 0.25rem 0;
    }

    .tab.second {
        --tab: var(--fg);
    }

    .tab.on .nm {
        font-weight: 700;
    }

    .c {
        width: 1.45rem;
        height: 1.45rem;
        border-radius: 50%;
        box-shadow: 0 0 0 1.5px var(--tab);
    }

    .nm {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .pos {
        color: var(--chat-text);
        font-size: var(--fs-xs);
    }

    .split {
        color: var(--chat-text);
        font-weight: 600;
        text-align: right;
    }

    .split small {
        color: var(--dim);
        font-size: var(--fs-xs);
        font-weight: 500;
    }

    .sync {
        width: 1.5rem;
        height: 1.5rem;
    }

    .sync.gone {
        visibility: hidden;
    }

    .sync :global(svg) {
        width: var(--icon-sm);
        height: var(--icon-sm);
        transition: transform 0.35s var(--ease);
    }

    .sync:active :global(svg) {
        transform: rotate(180deg);
    }

    :global(html.mobile) .me {
        display: none;
    }
</style>
