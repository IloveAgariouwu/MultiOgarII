<script lang="ts">
    import Icon from "../../components/Icon.svelte";
    import { useApp, useGame, useHudPlacement } from "../../lib/context";

    const app = useApp();
    const game = useGame();
    const placement = useHudPlacement();
    const t = app.t;
    let pop = $derived(game.population);
</script>

<section class="rail scrim" {@attach placement("rail")}>
    <header><b>{app.server.label}</b></header>

    <ol class="board">
        {#each game.leaderboard as p, i (i)}
            <li class="row" class:self={p.self}>
                <span class="rk">{p.rank}</span>
                <span class="nm">{p.name}</span>
            </li>
        {/each}
    </ol>

    <footer>
        <span class="counts">
            <span title={t("hudPlayers")}><Icon name="person" />{pop.players}</span>
            <span title={t("hudSpectators")}><Icon name="eye" />{pop.spectators}</span>
        </span>
        <span class="total" title={t("hudTotal")}><Icon name="users" />{pop.total}<i>/</i>{pop.max}</span>
    </footer>
    {#if game.teams.length}
        <ul class="teams">
            {#each game.teams as team, i (i)}
                <li style:--c={team.color || "var(--mute)"}>
                    <i></i>
                    <b>{(team.share * 100).toFixed(1)}%</b>
                </li>
            {/each}
        </ul>
    {/if}
</section>

<style>
    .rail {
        position: absolute;
        right: 2rem;
        top: 1.6rem;
        width: var(--hud-w);
        font-variant-numeric: tabular-nums;
    }

    footer {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 0.8rem;
        margin-top: 0.5rem;
        padding: 0.45rem 0.3rem 0 1.7rem;
        box-shadow: inset 0 1px var(--rule);
        color: var(--chat-text);
        font-size: var(--fs-xs);
        line-height: 1;
    }

    header b {
        display: block;
        overflow: hidden;
        color: var(--lb-head);
        font-size: var(--fs-md);
        font-weight: 700;
        white-space: nowrap;
        text-overflow: ellipsis;
    }

    .counts {
        display: flex;
        gap: 0.8rem;
        color: var(--dim);
    }

    .counts span {
        display: flex;
        align-items: center;
        gap: 0.15rem;
    }

    .counts :global(svg),
    .total :global(svg) {
        width: var(--icon-xs);
        height: var(--icon-xs);
        fill: none;
        stroke: currentColor;
        stroke-width: 1.8;
    }

    .total {
        display: flex;
        align-items: center;
        gap: 0.3rem;
        color: var(--fg);
    }

    .total i {
        color: var(--dim);
        font-style: normal;
    }

    .teams {
        margin: 0.4rem 0 0;
        padding: 0.35rem 0.3rem 0 1.7rem;
        list-style: none;
        box-shadow: inset 0 1px var(--rule);
        font-size: var(--fs-xs);
    }

    .teams li {
        display: grid;
        grid-template-columns: 0.45rem minmax(0, 1fr);
        gap: 0.5rem;
        align-items: center;
        padding: 0.1rem 0;
    }

    .teams i {
        width: 0.45rem;
        height: 0.45rem;
        background: var(--c);
    }

    .teams b {
        color: var(--mute);
        font-weight: 600;
        text-align: right;
    }

    .board {
        margin: 0.5rem 0 0;
        padding: 0;
        list-style: none;
    }

    .row {
        display: grid;
        grid-template-columns: calc(1.2rem * var(--text)) minmax(0, 1fr);
        gap: 0.5rem;
        align-items: baseline;
        padding: 0.12rem 0.3rem 0.12rem 0;
        font-size: var(--fs-sm);
        text-shadow: var(--shadow);
    }

    .rk {
        color: color-mix(in srgb, currentColor 60%, transparent);
        font-size: var(--fs-xs);
        text-align: right;
    }

    .nm {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .row.self {
        background: color-mix(in srgb, var(--fg) 10%, transparent);
        font-weight: 700;
    }

    :global(html.mobile) .rail {
        top: calc(var(--hud-top) + var(--scrim-y));
        right: calc(var(--hud-right) + var(--scrim-x));
        display: flex;
        flex-direction: column;
        width: calc(var(--lb-w) - 2 * var(--scrim-x));
        max-height: calc(100dvh - var(--hud-top) - 2 * var(--scrim-y) - var(--touch-top));
    }

    :global(html.mobile) header b {
        font-size: var(--fs-sm);
    }

    :global(html.mobile) .board {
        margin-top: 0.3rem;
    }

    :global(html.mobile) .row {
        padding: 0.05rem 0.3rem 0.05rem 0;
        font-size: var(--fs-xs);
    }

    :global(html.mobile) footer {
        margin-top: 0.3rem;
        padding-top: 0.3rem;
    }

    :global(html.mobile) .teams {
        flex: 0 1 auto;
        min-height: 0;
        overflow: hidden;
        margin-top: 0.3rem;
        padding-top: 0.25rem;
    }

    :global(html.mobile) .counts {
        display: none;
    }

    :global(html.mobile) .board li:nth-child(n + 7) {
        display: none;
    }

    @media (orientation: landscape) {
        :global(html.mobile) footer {
            display: none;
        }
    }

    @media (orientation: portrait) {
        :global(html.mobile) .board li:nth-child(n + 6) {
            display: none;
        }
    }
</style>
