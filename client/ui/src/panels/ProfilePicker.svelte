<script lang="ts">
    import { tick } from "svelte";
    import PickerPanel from "../components/PickerPanel.svelte";
    import { useApp } from "../lib/context";
    import { autoselect } from "../lib/focus";
    import { profileIndex } from "../lib/profile-fields";
    import Icon from "../components/Icon.svelte";
    import { skin } from "../lib/skin-images";

    const OVERSCAN_ROWS = 2;

    const app = useApp();
    const t = app.t;

    let query = $state("");
    let hideUnused = $state(false);
    let renaming = $state<number | null>(null);
    let dragged = $state<number | null>(null);
    let over = $state<number | null>(null);
    let list: HTMLDivElement;
    let grid: HTMLDivElement;
    let top = $state(0);
    let viewH = $state(0);
    let cols = $state(1);
    let rowH = $state(0);
    let revealed = false;

    let current = $derived(profileIndex(app.profiles, app.pickerTab));
    let rows = $derived.by(() => {
        const q = query.trim().toLowerCase();
        return app.profiles.entries
            .map((p, i) => ({ p, i }))
            .filter(
                ({ p }) =>
                    (!hideUnused || p.used) && (!q || `${p.label}${p.name}${p.tabName}`.toLowerCase().includes(q)),
            );
    });
    let adding = $derived(!query.trim());
    let rowCount = $derived(Math.ceil((rows.length + (adding ? 1 : 0)) / cols));
    let at = $derived(Math.min(top, Math.max(0, rowCount * rowH - viewH)));
    let first = $derived(rowH ? Math.max(0, Math.floor(at / rowH) - OVERSCAN_ROWS) : 0);
    let last = $derived(rowH ? Math.min(rowCount, Math.ceil((at + viewH) / rowH) + OVERSCAN_ROWS) : 1);
    let shown = $derived(rows.slice(first * cols, last * cols));

    async function scrollToProfile(i: number) {
        const row = Math.floor(rows.findIndex((r) => r.i === i) / cols);
        await tick();
        list.scrollTop = row * rowH - (viewH - rowH) / 2;
    }

    function measureGrid() {
        const style = getComputedStyle(grid);
        const tile = grid.querySelector<HTMLElement>(".tile");
        cols = style.gridTemplateColumns.split(" ").length;
        if (tile) rowH = tile.offsetHeight + parseFloat(style.rowGap);
        viewH = list.clientHeight;
        if (revealed) return;
        revealed = true;
        scrollToProfile(current);
    }

    function observeGridSize(el: HTMLDivElement) {
        const ro = new ResizeObserver(measureGrid);
        ro.observe(el);
        return () => ro.disconnect();
    }

    function selectProfileAt(i: number) {
        app.selectProfile(app.profiles.linked ? 0 : app.pickerTab, i);
        app.open(null);
    }

    function commitProfileLabel(i: number, value: string) {
        if (renaming !== i) return;
        app.setProfileField(i, "label", value.trim());
        renaming = null;
    }

    function addAndRenameProfile() {
        hideUnused = false;
        if (!app.addProfile()) return;
        renaming = app.profiles.entries.length - 1;
        scrollToProfile(renaming);
    }

    function endDrag() {
        dragged = null;
        over = null;
    }

    function dropProfileAt(to: number) {
        const from = dragged;
        endDrag();
        if (from !== null && from !== to) app.moveProfile(from, to);
    }

    function confirmRemoveProfile(i: number) {
        if (confirm(t("pickRemoveConfirm").replace("%s", () => app.profiles.entries[i].displayName)))
            app.removeProfile(i);
    }
</script>

<svelte:window ondragend={endDrag} ondrop={endDrag} onpointermove={endDrag} />

<PickerPanel
    title={t("pickProfilesTitle")}
    placeholder={t("pickProfilesSearch")}
    closeLabel={t("pickClose")}
    bind:query
    onclose={() => app.open(null)}
>
    {#snippet tools()}
        <label class="chk"><input type="checkbox" bind:checked={hideUnused} />{t("pickHideUnused")}</label>
        <span class="hint">{t("pickDragHint")}</span>
        {#if app.profileError}<span class="err">{app.profileError}</span>{/if}
    {/snippet}
    <div class="list" bind:this={list} onscroll={(e) => (top = e.currentTarget.scrollTop)} {@attach observeGridSize}>
        <div
            class="grid"
            bind:this={grid}
            {@attach observeGridSize}
            style:padding-top="{first * rowH}px"
            style:padding-bottom="{(rowCount - last) * rowH}px"
        >
            {#each shown as { p, i } (i)}
                <div
                    class="tile"
                    class:unused={!p.used}
                    class:current={i === current}
                    class:dragged={i === dragged}
                    class:before={i === over && dragged !== null && i < dragged}
                    class:after={i === over && dragged !== null && i > dragged}
                    role="listitem"
                    draggable={renaming !== i}
                    ondragstart={(e) => {
                        dragged = i;
                        if (!e.dataTransfer) return;
                        e.dataTransfer.effectAllowed = "move";
                        e.dataTransfer.setData("text/plain", p.displayName);
                    }}
                    ondragover={(e) => {
                        if (dragged === null) return;
                        e.preventDefault();
                        over = i;
                    }}
                    ondrop={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        dropProfileAt(i);
                    }}
                >
                    <span class="k">{i + 1}</span>
                    <span class="two">
                        <span class="c well" use:skin={{ url: p.skinUrl }}></span>
                        <span class="c well" use:skin={{ url: p.tabSkinUrl }}></span>
                    </span>
                    {#if renaming === i}
                        <input
                            class="rn"
                            maxlength={app.limits.profileLabelMax}
                            placeholder={p.name}
                            value={p.label}
                            {@attach autoselect}
                            onkeydown={(e) => {
                                e.stopPropagation();
                                if (e.key === "Enter") e.currentTarget.blur();
                                if (e.key === "Escape") renaming = null;
                            }}
                            onblur={(e) => commitProfileLabel(i, e.currentTarget.value)}
                        />
                    {:else}
                        <button class="pick stretch" onclick={() => selectProfileAt(i)}
                            ><span class="nm"
                                >{#if p.label}{p.label}{:else}<span class="dim">{p.name}</span>{/if}</span
                            ></button
                        >
                    {/if}
                    <button
                        class="ren"
                        title={t("pickRename")}
                        aria-label={t("pickRename")}
                        onclick={() => (renaming = i)}>✎</button
                    >
                    <button
                        class="icon del"
                        title={t("pickRemove")}
                        aria-label={t("pickRemove")}
                        onclick={() => confirmRemoveProfile(i)}><Icon name="close" /></button
                    >
                </div>
            {/each}
            {#if !rows.length}<div class="empty">{t("pickNoProfile").replace("%s", () => query)}</div>{/if}
            {#if adding && last * cols > rows.length}
                <button class="tile add" onclick={addAndRenameProfile}><span class="plus">+</span>{t("pickAddProfile")}</button>
            {/if}
        </div>
    </div>
</PickerPanel>

<style>
    .chk {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        color: var(--mute);
        cursor: pointer;
    }

    .list {
        height: 100%;
        overflow-y: auto;
    }

    .grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(11rem, 1fr));
        gap: 0.5rem;
        align-content: start;
    }

    .tile {
        position: relative;
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 0.45rem;
        padding: 0.9rem 0.4rem 0.7rem;
        border: 1px solid transparent;
        background: color-mix(in srgb, var(--bg) 30%, transparent);
        cursor: pointer;
    }

    .tile:hover {
        border-color: var(--rule);
        background: color-mix(in srgb, var(--bg) 55%, transparent);
    }

    .tile.current {
        border-color: var(--accent);
    }

    .tile.unused {
        opacity: 0.45;
    }

    .tile.dragged {
        opacity: 0.25;
    }

    .tile.before {
        box-shadow: -0.25rem 0 0 var(--accent);
    }

    .tile.after {
        box-shadow: 0.25rem 0 0 var(--accent);
    }

    .hint {
        color: var(--dim);
        font-size: var(--fs-sm);
    }

    .two {
        display: flex;
        gap: 0.3rem;
    }

    .c {
        width: 3.6rem;
        height: 3.6rem;
        border-radius: 50%;
        box-shadow: inset 0 0 0 0.2rem rgba(0, 0, 0, 0.25);
    }

    .pick {
        display: flex;
        max-width: 100%;
    }

    .nm {
        max-width: 100%;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        font-size: var(--fs-md);
    }

    .dim {
        color: var(--mute);
    }

    .k {
        position: absolute;
        left: 0.45rem;
        top: 0.3rem;
        color: var(--dim);
        font-size: var(--fs-sm);
        font-variant-numeric: tabular-nums;
    }

    .ren {
        position: absolute;
        right: 0.3rem;
        top: 0.2rem;
        padding: 0 0.3rem;
        border: 0;
        background: none;
        color: var(--dim);
        opacity: 0;
        transition: opacity 0.15s;
    }

    .tile:hover .ren,
    .ren:focus-visible {
        opacity: 1;
    }

    .del {
        --icon: 1rem;
        position: absolute;
        right: 1.9rem;
        top: 0.3rem;
        color: var(--dim);
        opacity: 0;
        transition: opacity 0.15s;
    }

    .tile:hover .del,
    .del:focus-visible {
        opacity: 1;
    }

    .ren:hover,
    .del:hover {
        color: var(--fg);
    }

    .add {
        justify-content: center;
        color: var(--mute);
        font: inherit;
    }

    .plus {
        font-size: 2.4rem;
        line-height: 1;
    }

    .err {
        color: var(--danger);
        font-size: var(--fs-xs);
    }

    .rn {
        width: 100%;
        padding: 0.1rem 0.3rem;
        border: 1px solid var(--accent);
        background: var(--field-focus);
        text-align: center;
    }

    .rn:focus {
        outline: 0;
    }

    :global(html.mobile) .grid {
        grid-template-columns: repeat(auto-fill, minmax(7.5rem, 1fr));
    }

    :global(html.mobile) .del,
    :global(html.mobile) .ren {
        min-width: var(--hit);
        min-height: var(--hit);
        opacity: 1;
    }

    :global(html.mobile) .del {
        right: calc(var(--hit) + 0.3rem);
    }

    .empty {
        grid-column: 1 / -1;
        padding: 3rem 0;
        color: var(--mute);
        text-align: center;
    }
</style>
