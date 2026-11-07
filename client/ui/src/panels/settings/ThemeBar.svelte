<script lang="ts">
    import Select from "../../components/Select.svelte";
    import { useApp } from "../../lib/context";
    import { autofocus } from "../../lib/focus";
    import { saveFile } from "../../lib/replay-store";

    type Naming = "save" | "rename" | null;

    const BUILTIN = "";
    const app = useApp();
    const t = app.t;
    let naming = $state<Naming>(null);
    let name = $state("");
    let fileInput: HTMLInputElement;
    let selected = $derived(app.themes.selected);
    let options = $derived([
        { value: BUILTIN, label: t("themeBuiltin") },
        ...app.themes.names.map((n) => ({ value: n, label: n })),
    ]);

    function startNaming(next: Naming) {
        app.themeError = "";
        naming = naming === next ? null : next;
        name = next === "rename" ? (selected ?? "") : "";
    }

    function commitName() {
        const done = naming === "save" ? app.saveThemeAs(name) : app.renameTheme(name);
        if (done) naming = null;
    }

    function confirmRemoveTheme() {
        if (selected === null || !confirm(t("themeDeleteConfirm").replace("%s", () => selected))) return;
        naming = null;
        app.deleteTheme();
    }

    function exportTheme() {
        saveFile(new Blob([app.core.exportTheme()], { type: "application/json" }), `theme-${selected}`, "json");
    }

    async function upload(e: Event & { currentTarget: HTMLInputElement }) {
        const file = e.currentTarget.files?.[0];
        e.currentTarget.value = "";
        if (file) app.importTheme(await file.text());
    }
</script>

<div class="bar">
    <input type="file" accept="application/json" hidden bind:this={fileInput} onchange={upload} />
    <div class="pick">
        <span>{t("UiTheme")}</span>
        <Select
            {options}
            value={selected ?? BUILTIN}
            label={t("UiTheme")}
            onchange={(v) => ((naming = null), app.selectTheme(v === BUILTIN ? null : v))}
        />
    </div>
    <div class="actions">
        <button class="btn-line" aria-expanded={naming === "save"} onclick={() => startNaming("save")}
            >{t("themeSaveAs")}</button
        >
        <button
            class="btn-line"
            disabled={selected === null}
            aria-expanded={naming === "rename"}
            onclick={() => startNaming("rename")}>{t("themeRename")}</button
        >
        <button class="btn-line" disabled={selected === null} onclick={confirmRemoveTheme}>{t("themeDelete")}</button>
        <button class="btn-line" disabled={selected === null} onclick={exportTheme}>{t("themeExport")}</button>
        <button class="btn-line" onclick={() => fileInput.click()}>{t("themeImport")}</button>
    </div>
    {#if naming}
        <form class="actions" onsubmit={(e) => (e.preventDefault(), commitName())}>
            <input
                class="f"
                maxlength={app.limits.themeNameMax}
                placeholder={t("themeName")}
                aria-label={t("themeName")}
                bind:value={name}
                {@attach autofocus}
            />
            <button class="btn-accent" type="submit">{t(naming === "save" ? "themeSaveAs" : "themeRename")}</button>
        </form>
    {/if}
    {#if app.themeError}<div class="err">{app.themeError}</div>{/if}
</div>

<style>
    .bar {
        padding-bottom: 1rem;
    }

    .pick {
        display: grid;
        grid-template-columns: 1fr auto;
        align-items: center;
        min-height: 2.8rem;
        border-bottom: 1px solid color-mix(in srgb, var(--fg) 6%, transparent);
    }

    .actions {
        display: flex;
        flex-wrap: wrap;
        gap: 0.6rem;
        margin: 0;
        padding: 0.6rem 0 0;
    }

    .actions button {
        padding: 0.45rem 1rem;
    }

    .actions .f {
        flex: 1;
    }

    .err {
        padding-top: 0.4rem;
        color: var(--danger);
        font-size: var(--fs-xs);
    }
</style>
