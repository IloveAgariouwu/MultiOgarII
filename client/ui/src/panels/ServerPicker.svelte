<script lang="ts">
    import PickerPanel from "../components/PickerPanel.svelte";
    import { useApp, useGame, useSource } from "../lib/context";
    import { livePopulation, population } from "../lib/format";
    import {
        addCustomServer,
        customChoice,
        customServers,
        isValidAddress,
        localServer,
        normalizeAddress,
        queryServer,
        removeCustomServer,
        soloServers,
    } from "../lib/servers";
    import type { ServerChoice } from "../lib/types";

    const app = useApp();
    const game = useGame();
    const source = useSource();
    const t = app.t;

    let query = $state("");
    let customs = $state(customServers());
    let addValue = $state("");
    let addInvalid = $state(false);

    const solos = soloServers();
    const local = localServer();
    const fromUrl = queryServer();
    let q = $derived(query.trim().toLowerCase());
    let builtin = $derived(
        [...solos, local, ...(fromUrl && !customs.includes(fromUrl.addr) ? [fromUrl] : [])].filter(
            (s) => !q || `${s.label}${s.addr}`.toLowerCase().includes(q),
        ),
    );
    let customRows = $derived(customs.filter((addr) => !q || addr.toLowerCase().includes(q)));
    let currentPop = $derived(population(livePopulation(game.population)));

    function connectToServer(choice: ServerChoice) {
        app.setServer(choice);
        source.connect(choice);
        app.open(null);
    }

    function add() {
        const addr = normalizeAddress(addValue);
        addInvalid = !isValidAddress(addr);
        if (addInvalid) return;
        addCustomServer(addr);
        customs = customServers();
        addValue = "";
    }

    function remove(addr: string) {
        removeCustomServer(addr);
        customs = customServers();
    }
</script>

<PickerPanel
    title={t("pickServersTitle")}
    placeholder={t("pickServersSearch")}
    closeLabel={t("pickClose")}
    bind:query
    onclose={() => app.open(null)}
>
    {#each builtin as s (s.id)}
        <div class="row" class:current={s.id === app.server.id}>
            <button class="pick stretch" onclick={() => connectToServer(s)}><b>{s.label}</b></button>
            <span class="md">{s.addr}</span>
            <span class="pp">{s.id === app.server.id ? currentPop : "-/-"}</span>
            <span></span>
        </div>
    {/each}
    <div class="grp">{t("srvGroupCustom")}</div>
    {#each customRows as addr (addr)}
        {@const choice = customChoice(addr)}
        <div class="row" class:current={app.server.id === choice.id}>
            <button class="pick stretch" onclick={() => connectToServer(choice)}><b>{addr}</b></button>
            <span class="md"></span>
            <span class="pp">{choice.id === app.server.id ? currentPop : "-/-"}</span>
            <button class="ex" onclick={() => remove(addr)}>{t("srvRemove")}</button>
        </div>
    {/each}
    <div class="row add">
        <input
            class="f"
            class:bad={addInvalid}
            placeholder={t("srvAddPlaceholder")}
            title={addInvalid ? t("srvAddInvalid") : ""}
            spellcheck="false"
            bind:value={addValue}
            onkeydown={(e) => e.key === "Enter" && add()}
        />
        <span></span>
        <span></span>
        <button class="ex" onclick={add}>{t("srvAdd")}</button>
    </div>
</PickerPanel>

<style>
    .grp {
        padding: 1rem 0.8rem 0.3rem;
        color: var(--dim);
        font-size: var(--fs-sm);
    }

    .row {
        position: relative;
        display: grid;
        grid-template-columns: 1fr 8rem 7rem 8rem;
        align-items: baseline;
        gap: 1rem;
        padding: 0.6rem 0.8rem;
        border-bottom: 1px solid color-mix(in srgb, var(--fg) 7%, transparent);
        cursor: pointer;
    }

    .row:hover {
        background: color-mix(in srgb, var(--bg) 50%, transparent);
    }

    .row.current {
        box-shadow: inset 2px 0 var(--accent);
    }

    .pick {
        display: flex;
        min-width: 0;
    }

    .row b {
        font-size: var(--fs-lg);
    }

    .ex {
        color: var(--mute);
    }

    .md {
        color: var(--mute);
        font-size: var(--fs-sm);
    }

    .pp {
        text-align: right;
        font-variant-numeric: tabular-nums;
    }

    .ex {
        position: relative;
        justify-self: end;
        padding: 0;
        border: 0;
        background: none;
        text-shadow: var(--shadow);
    }

    .ex:hover {
        color: var(--fg);
    }

    .add {
        cursor: default;
        align-items: center;
    }

    .add .f {
        grid-column: 1 / 2;
    }

    .bad {
        border-color: var(--danger);
    }

    :global(html.mobile) .row {
        grid-template-columns: minmax(0, 1fr) auto auto auto;
        align-items: center;
        gap: 0.8rem;
        min-height: calc(var(--hit) * 1.5);
        padding: 0.3rem 0.6rem;
    }

    :global(html.mobile) .row b {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    :global(html.mobile) .md,
    :global(html.mobile) .pp,
    :global(html.mobile) .ex {
        white-space: nowrap;
    }

    :global(html.mobile) .ex {
        min-height: var(--hit);
    }

    :global(html.mobile) .add .f {
        height: calc(var(--hit) * 1.5);
    }

    :global(html.mobile) .add {
        grid-template-columns: minmax(0, 1fr) auto;
    }

    :global(html.mobile) .add span {
        display: none;
    }
</style>
