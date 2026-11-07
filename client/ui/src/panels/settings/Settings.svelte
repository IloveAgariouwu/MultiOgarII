<script lang="ts">
    import type { Snippet } from "svelte";
    import { fade, fly } from "svelte/transition";
    import { PANEL_IN, PANEL_OUT } from "../../lib/transitions";
    import { useApp } from "../../lib/context";
    import { rangeVisible } from "../../lib/settings-format";
    import { bindable, categoryRows, settingKeys, type RowEntry } from "../../lib/settings-rows";
    import { previewKind, type BrowserFeature, type CategorySpec } from "../../lib/types";
    import { exportConfig, importConfig } from "../../lib/store";
    import { packMessage } from "../../lib/i18n";
    import Icon from "../../components/Icon.svelte";
    import { saveFile } from "../../lib/replay-store";
    import MacroEditor from "./MacroEditor.svelte";
    import PreviewPane from "./PreviewPane.svelte";
    import RowView from "./RowView.svelte";
    import SettingInfo from "./SettingInfo.svelte";
    import LanguageBar from "./LanguageBar.svelte";
    import ThemeBar from "./ThemeBar.svelte";
    import ReplayStorage from "../ReplayStorage.svelte";

    interface Borrowed {
        cat: CategorySpec;
        rows: RowEntry[];
    }

    const RESET_PARTS = [
        ["part:hotkeys", "setCat_hotkeys"],
        ["part:colors", "setCat_colors"],
        ["part:macros", "macroTitle"],
    ];
    const MOBILE_HIDDEN = "hotkeys";
    const MOBILE_FIRST = "touch";
    const app = useApp();
    const browser: Record<BrowserFeature, boolean> = { touch: gMobile };
    for (const [key, feature] of Object.entries(app.ui.browserFeatures))
        if (!(feature in browser)) throw new Error(`${key} needs unknown browser feature ${feature}`);
    const t = app.t;
    const lookup = app.lookup;
    const colorKeys = Object.keys(app.colors);
    const catalog = app.ui.categories.map((cat) => ({ cat, rows: categoryRows(cat, app.groups, colorKeys, lookup) }));
    const homes = new Map(catalog.flatMap(({ cat, rows }) => rows.map((e): [string, string] => [e.key, cat.id])));
    const homePreviews = new Map(
        catalog.flatMap(({ cat, rows }) => rows.map((e): [string, string] => [e.key, cat.preview])),
    );
    for (const cat of app.ui.categories)
        for (const key of cat.shared ?? [])
            if (!homes.has(key) || homes.get(key) === cat.id)
                throw new Error(`${cat.id} shares ${key}, which no other category lists`);

    const resettable = catalog
        .map(({ cat, rows }) => ({
            id: cat.id,
            title: cat.title,
            keys: [...new Set(rows.flatMap((e) => settingKeys(e.row)))].filter((k) => k in app.defaults),
        }))
        .filter((c) => c.keys.length);
    const resetChoices = [...resettable.map((c) => [c.id, c.title]), ...RESET_PARTS];

    let current = $state(app.ui.categories.find((c) => c.id === app.settingsCategory) ?? app.ui.categories[0]);
    let opened = $state(app.settingsCategory !== null);
    app.settingsCategory = null;
    let query = $state("");
    let binding = $state("");
    let focused = $state("");
    let fileInput: HTMLInputElement;
    let resetting = $state(false);
    let picked = $state<string[]>([]);
    let importError = $state("");

    let owned = $derived(
        app.ui.categories.map((cat) => ({
            cat,
            rows: categoryRows(cat, app.groups, colorKeys, lookup).filter(
                (e) =>
                    (e.row.kind !== "range" || rangeVisible(e.row.spec, app.settings)) &&
                    settingKeys(e.row).every(
                        (k) =>
                            (!app.ui.requires[k] || app.flag(app.ui.requires[k])) &&
                            (!app.ui.browserFeatures[k] || browser[app.ui.browserFeatures[k]]),
                    ),
            ),
        })),
    );
    let sections = $derived(owned.map((s) => ({ ...s, borrowed: borrowedSettings(s.cat) })));
    let q = $derived(query.trim().toLowerCase());
    const extras: Record<string, { body: Snippet; found?: () => boolean }> = {
        hud: { body: hudActions, found: () => t("lbtLanguage").toLowerCase().includes(q) },
        hotkeys: { body: macros },
        colors: { body: themes, found: () => t("UiTheme").toLowerCase().includes(q) },
        replay: { body: replayStorage, found: () => t("replayStorageTitle").toLowerCase().includes(q) },
        data: { body: dataActions },
    };
    let reachable = $derived(gMobile ? sections.filter((s) => s.cat.id !== MOBILE_HIDDEN) : sections);
    let listed = $derived(reachable.filter((s) => s.rows.length || s.borrowed.length || s.cat.id in extras));
    let pages = $derived([
        ...listed.filter((s) => s.cat.id === MOBILE_FIRST),
        ...listed.filter((s) => s.cat.id !== MOBILE_FIRST),
    ]);
    let active = $derived(listed.find((s) => s.cat.id === current.id) ?? listed[0]);
    let shown = $derived(
        q
            ? reachable
                  .map((s) => ({ ...s, rows: s.rows.filter((r) => r.label.toLowerCase().includes(q)), borrowed: [] }))
                  .filter((s) => s.rows.length || extras[s.cat.id]?.found?.())
            : [active],
    );
    let shownRows = $derived(shown.flatMap((s) => [...s.rows, ...s.borrowed.flatMap((b) => b.rows)]));
    let canBind = $derived(shownRows.some((e) => bindable(e.row)));
    let needsReload = $derived(shownRows.some((e) => app.core.needsReload(e.key)));
    for (const p of [...Object.values(app.ui.previews), ...app.ui.categories.map((c) => c.preview)]) previewKind(p);
    for (const [key, overrides] of Object.entries(app.ui.previewOverrides))
        for (const target of [key, ...Object.keys(overrides)])
            if (!(target in app.defaults) && !(target in app.colors))
                throw new Error(`preview override ${key} names unknown setting ${target}`);
    let preview = $derived(app.ui.previews[focused] ?? homePreviews.get(focused) ?? "none");
    let previewOverrides = $derived(app.ui.previewOverrides[focused] ?? {});
    let entry = $derived(owned.flatMap((s) => s.rows).find((e) => e.key === focused) ?? null);
    let flags = $derived(
        app.ui.categories
            .flatMap((c) => c.items)
            .flatMap((i) => (typeof i !== "string" && i.kind === "flag" && app.settings[i.key] ? [i.key] : [])),
    );

    function borrowedSettings(cat: CategorySpec): Borrowed[] {
        const out: Borrowed[] = [];
        for (const key of cat.shared ?? [])
            for (const home of owned) {
                if (home.cat.id !== homes.get(key)) continue;
                const entry = home.rows.find((e) => e.key === key);
                if (!entry) continue;
                const group = out.find((b) => b.cat.id === home.cat.id);
                if (group) group.rows.push(entry);
                else out.push({ cat: home.cat, rows: [entry] });
            }
        return out;
    }

    function download() {
        app.store.flush();
        saveFile(new Blob([exportConfig(flags)], { type: "application/json" }), "settings", "json");
    }

    async function upload(e: Event & { currentTarget: HTMLInputElement }) {
        const file = e.currentTarget.files?.[0];
        e.currentTarget.value = "";
        if (!file) return;
        const text = await file.text();
        app.store.flush();
        const result = importConfig(text, flags);
        importError = result.error ? `${t("setExport_languages")}: ${packMessage(result.error, lookup)}` : "";
        if (result.sections) location.reload();
    }

    function editHud() {
        app.open(null);
        app.hud.editing = true;
    }

    function resetPicked() {
        if (!confirm(t("setResetConfirm"))) return;
        app.reset({
            keys: resettable.filter((c) => picked.includes(c.id)).flatMap((c) => c.keys),
            hotkeys: picked.includes("part:hotkeys"),
            colors: picked.includes("part:colors"),
            macros: picked.includes("part:macros"),
        });
        picked = [];
        resetting = false;
    }
</script>

{#snippet row(entry: RowEntry)}
    <div
        class="rw"
        role="presentation"
        onpointerdown={() => {
            focused = entry.key;
            if (binding !== entry.id) binding = "";
        }}
        onfocusin={() => {
            focused = entry.key;
            if (binding !== entry.id) binding = "";
        }}
    >
        <RowView
            {entry}
            binding={binding === entry.id}
            selected={focused === entry.key}
            onlabel={() => (binding = binding === entry.id ? "" : entry.id)}
        />
    </div>
{/snippet}

{#snippet hudActions()}
    <LanguageBar />
    {#if !gMobile && !q}
        <div class="actions">
            <button class="btn-line" onclick={editHud}>{t("hudEditStart")}</button>
            <button class="btn-line" disabled={!app.hud.customized} onclick={() => app.hud.reset()}
                >{t("hudEditReset")}</button
            >
        </div>
    {/if}
{/snippet}

{#snippet macros()}<MacroEditor />{/snippet}

{#snippet themes()}<ThemeBar />{/snippet}

{#snippet replayStorage()}
    <div class="sec">{t("replayStorageTitle")}</div>
    <ReplayStorage />
{/snippet}

{#snippet dataActions()}
    <div class="actions">
        <button class="btn-line" onclick={download}>{t("lbtOutputConfig")}</button>
        <button class="btn-line" onclick={() => fileInput.click()}>{t("lbtInputConfig")}</button>
        <button class="btn-line" aria-expanded={resetting} onclick={() => ((resetting = !resetting), (picked = []))}
            >{t("lbtResetConfig")}</button
        >
    </div>
    {#if resetting}
        <div class="resetpick">
            {#each resetChoices as [id, title] (id)}<label
                    ><input type="checkbox" bind:group={picked} value={id} />{lookup(title)}</label
                >{/each}
        </div>
        <div class="actions">
            <button
                class="btn-line"
                onclick={() => (picked = picked.length === resetChoices.length ? [] : resetChoices.map(([id]) => id))}
                >{t(picked.length === resetChoices.length ? "setResetNone" : "setResetAll")}</button
            >
            <button class="btn-accent" disabled={!picked.length} onclick={resetPicked}>{t("setResetPicked")}</button>
        </div>
    {/if}
    {#if importError}<div class="err">{importError}</div>{/if}
    <div class="sec">{t("setDataScope")}</div>
{/snippet}

{#snippet results()}
    {#key q ? "" : active.cat.id}
        <div in:fly={{ y: 6, duration: 200 }}>
            {#each shown as s (s.cat.id)}
                {#if q}<div class="sec">{lookup(s.cat.title)}</div>{/if}
                {@const extra = extras[s.cat.id]}
                {#if extra && (!q || extra.found?.())}{@render extra.body()}{/if}
                {#each s.rows as entry (entry.id)}{@render row(entry)}{/each}
                {#each s.borrowed as b (b.cat.id)}
                    <div class="sec">{t("setSharedFrom").replace("%s", lookup(b.cat.title))}</div>
                    {#each b.rows as entry (entry.id)}{@render row(entry)}{/each}
                {/each}
            {:else}
                <div class="empty">{t("setNoMatch").replace("%s", () => query)}</div>
            {/each}
        </div>
    {/key}
{/snippet}

<input type="file" accept="application/json" hidden bind:this={fileInput} onchange={upload} />
{#if gMobile}
    <div
        class="msettings"
        role="dialog"
        aria-label={t("menuSettings")}
        in:fly={PANEL_IN}
        out:fade={PANEL_OUT}
        {@attach app.layer(() => (opened && !q ? (opened = false) : app.open(null)))}
    >
        {#if opened && !q}
            <header class="mhead">
                <button class="back" onclick={() => (opened = false)}
                    ><span class="icon chev"><Icon name="left" /></span>{lookup(active.cat.title)}</button
                >
            </header>
        {:else}
            <header class="mhead">
                <h1>{t("menuSettings")}</h1>
                <button class="icon close" aria-label={t("pickClose")} onclick={() => app.open(null)}
                    ><Icon name="close" /></button
                >
            </header>
            <input class="f msearch" type="search" placeholder={t("setSearch")} bind:value={query} />
        {/if}
        <div class="mlist">
            {#if (opened || q) && needsReload}<div class="hint mhint">{t("lbtReloadHint")}</div>{/if}
            {#if opened || q}
                {@render results()}
            {:else}
                <nav class="mcats">
                    {#each pages as { cat } (cat.id)}
                        <button onclick={() => ((current = cat), (opened = true), (resetting = false))}
                            ><span>{lookup(cat.title)}</span><span class="icon chev"><Icon name="right" /></span
                            ></button
                        >
                    {/each}
                </nav>
            {/if}
        </div>
    </div>
{:else}
    <div
        class="settings"
        role="dialog"
        aria-label={t("menuSettings")}
        in:fly={PANEL_IN}
        out:fade={PANEL_OUT}
        {@attach app.layer(() => app.open(null))}
    >
        <nav class="cats">
            <h1>{t("menuSettings")}</h1>
            {#each listed as { cat } (cat.id)}
                <button
                    aria-current={!q && cat.id === active.cat.id}
                    onclick={() => ((current = cat), (query = ""), (focused = ""), (binding = ""))}
                    >{lookup(cat.title)}</button
                >
            {/each}
            <span class="sp"></span>
            <button class="low" onclick={() => app.open(null)}>{t("setBack")}{gMobile ? "" : " Esc"}</button>
        </nav>

        <div class="opts">
            <div class="head">
                <input class="f" type="search" placeholder={t("setSearch")} bind:value={query} />
                {#if canBind}<span class="hint">{t("lbtHotkeyHint")}</span>{/if}
                {#if needsReload}<span class="hint">{t("lbtReloadHint")}</span>{/if}
            </div>
            <div class="list">
                {@render results()}
            </div>
        </div>

        <aside class="side">
            <div class="pvslot" class:gone={preview === "none"}>
                <PreviewPane {preview} overrides={previewOverrides} />
            </div>
            <div class="infoslot"><SettingInfo {entry} category={active.cat} /></div>
        </aside>
    </div>
{/if}

<style>
    .settings {
        position: fixed;
        left: 50%;
        top: 50%;
        width: min(110rem, 92vw);
        height: min(56rem, 86vh);
        display: grid;
        grid-template-columns: 12rem minmax(0, 1.2fr) minmax(0, 1fr);
        gap: 0 3rem;
        transform: translate(-50%, -50%);
        text-shadow: var(--shadow);
    }

    .side {
        display: flex;
        flex-direction: column;
        gap: 1rem;
        min-height: 0;
    }

    .pvslot {
        display: grid;
        flex: 1 1 0;
        min-height: 0;
    }

    .pvslot.gone {
        flex: 0 0 0;
        overflow: hidden;
    }

    .infoslot {
        display: grid;
        flex: 0 0 14rem;
        min-height: 0;
    }

    .cats {
        display: flex;
        flex-direction: column;
        gap: 0.15rem;
    }

    h1 {
        margin: 0 0 1.4rem;
        font-size: var(--fs-2xl);
    }

    .cats button {
        padding: 0.4rem 0 0.4rem 0.9rem;
        border: 0;
        border-left: 2px solid transparent;
        background: none;
        color: var(--mute);
        text-align: left;
        font-size: var(--fs-lg);
        text-shadow: var(--shadow);
    }

    .cats button:hover {
        color: var(--fg);
    }

    .cats button[aria-current="true"] {
        color: var(--fg);
        border-left-color: var(--accent);
    }

    .cats .low {
        color: var(--dim);
        font-size: var(--fs-md);
    }

    .sp {
        flex: 1;
    }

    .opts {
        display: flex;
        flex-direction: column;
        min-height: 0;
    }

    .head {
        display: flex;
        flex-direction: column;
        gap: 0.2rem;
        margin-bottom: 0.6rem;
    }

    .hint {
        color: var(--dim);
        font-size: var(--fs-sm);
    }

    .list {
        flex: 1;
        overflow: auto;
        padding-right: 0.6rem;
    }

    .sec {
        padding: 1.1rem 0 0.3rem;
        border-bottom: 1px solid var(--rule);
        color: var(--dim);
        font-size: var(--fs-sm);
    }

    .actions {
        display: flex;
        gap: 0.6rem;
        padding: 0.6rem 0 1rem;
    }

    .resetpick {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(9rem, 1fr));
        gap: 0.4rem 1rem;
    }

    .resetpick label {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        cursor: pointer;
    }

    .actions button {
        padding: 0.45rem 1rem;
    }

    .err {
        padding-bottom: 0.6rem;
        color: var(--danger);
        font-size: var(--fs-xs);
        overflow-wrap: anywhere;
    }

    .empty {
        padding: 2rem 0;
        color: var(--mute);
        text-align: center;
    }

    .rw {
        display: contents;
    }

    .msettings {
        position: fixed;
        inset: 0;
        display: flex;
        flex-direction: column;
        padding: env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom)
            env(safe-area-inset-left);
        text-shadow: var(--shadow);
    }

    .mhead,
    .msearch,
    .mlist {
        box-sizing: border-box;
        width: 100%;
        max-width: 46rem;
        margin: 0 auto;
    }

    .mhead {
        display: flex;
        flex: none;
        align-items: center;
        justify-content: space-between;
        min-height: calc(var(--hit) * 1.5);
        padding: 0 1rem;
    }

    .mhead h1 {
        margin: 0;
        font-size: var(--fs-lg);
    }

    .mhead button {
        display: flex;
        align-items: center;
        min-width: calc(var(--hit) * 1.5);
        min-height: calc(var(--hit) * 1.5);
        padding: 0;
        border: 0;
        background: none;
        color: var(--fg);
        text-shadow: var(--shadow);
    }

    .mhead .close {
        justify-content: flex-end;
    }

    .mhead .close:active,
    .back:active {
        color: var(--accent);
    }

    .back {
        gap: 0.4rem;
        font-size: var(--fs-lg);
    }

    .chev {
        color: var(--mute);
    }

    .msearch {
        flex: none;
        width: calc(100% - 2rem);
        max-width: 44rem;
        min-height: var(--hit);
        margin-bottom: 0.4rem;
    }

    .mlist {
        flex: 1;
        min-height: 0;
        overflow-y: auto;
        overscroll-behavior: contain;
        padding: 0 1rem 1rem;
    }

    .mhint {
        padding: 0.3rem 0;
    }

    .mcats {
        display: flex;
        flex-direction: column;
    }

    .mcats button {
        display: flex;
        align-items: center;
        justify-content: space-between;
        min-height: calc(var(--hit) * 1.5);
        padding: 0;
        border: 0;
        border-bottom: 1px solid var(--rule);
        background: none;
        color: var(--fg);
        text-align: left;
        font-size: var(--fs-md);
        text-shadow: var(--shadow);
    }

    .mcats button:active {
        color: var(--accent);
    }

    .msettings .actions {
        flex-wrap: wrap;
    }

    .msettings .actions button {
        min-height: var(--hit);
    }
</style>
