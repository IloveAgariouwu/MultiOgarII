<script lang="ts">
    import Select from "../../components/Select.svelte";
    import { useApp } from "../../lib/context";
    import {
        englishCount,
        languages,
        packMessage,
        readPackFile,
        savePacks,
        storedPacks,
        translationTemplate,
        type PackError,
    } from "../../lib/i18n";
    import { saveFile } from "../../lib/replay-store";
    import { KEYS } from "../../lib/store";

    const app = useApp();
    const t = app.t;
    const active = document.documentElement.lang;
    let fileInput: HTMLInputElement;
    let imported = $state(storedPacks());
    let error = $state("");
    let notes = $state<string[]>([]);
    let options = $derived([
        { value: "", label: t("pickAuto") },
        ...languages(imported).map((l) => ({
            value: l.code,
            label: l.imported ? t("langPackLabel").replace("%s", () => l.name) : l.name,
        })),
    ]);
    let removable = $derived(imported.some((p) => p.code === active));

    const message = (e: PackError) => packMessage(e, app.lookup);

    function pickLanguage(code: string) {
        if (code) localStorage.setItem(KEYS.language, code);
        else localStorage.removeItem(KEYS.language);
        location.reload();
    }

    async function upload(e: Event & { currentTarget: HTMLInputElement }) {
        const file = e.currentTarget.files?.[0];
        e.currentTarget.value = "";
        if (!file) return;
        notes = [];
        const read = readPackFile(await file.text());
        if (!read.ok) {
            error = message(read.error);
            return;
        }
        const { pack, warnings, translated } = read;
        const replaced = imported.some((p) => p.code === pack.code);
        const next = replaced ? imported.map((p) => (p.code === pack.code ? pack : p)) : [...imported, pack];
        const failed = savePacks(next);
        if (failed) {
            error = message(failed);
            return;
        }
        error = "";
        imported = next;
        notes = [
            t(replaced ? "langReplaced" : "langImported").replace("%s", () => `${pack.name} (${pack.code})`),
            t("langCoverage").replace("%s", () => `${translated} / ${englishCount()}`),
            ...warnings.map(message),
            t(pack.code === active ? "langReloadToApply" : "langPickToUse"),
        ];
    }

    async function exportTemplate() {
        saveFile(
            new Blob([await translationTemplate(active, imported)], { type: "application/json" }),
            `language-${active}`,
            "json",
        );
    }

    function confirmRemoveLanguage() {
        const pack = imported.find((p) => p.code === active);
        if (!pack || !confirm(t("langRemoveConfirm").replace("%s", () => pack.name))) return;
        const failed = savePacks(imported.filter((p) => p !== pack));
        if (failed) {
            error = message(failed);
            return;
        }
        if (!languages([]).some((l) => l.code === active) && localStorage.getItem(KEYS.language) === active)
            localStorage.removeItem(KEYS.language);
        location.reload();
    }
</script>

<div class="bar" class:m={gMobile}>
    <input type="file" accept="application/json" hidden bind:this={fileInput} onchange={upload} />
    <div class="lang">
        <span>{t("lbtLanguage")}</span>
        <Select
            {options}
            value={localStorage.getItem(KEYS.language) ?? ""}
            label={t("lbtLanguage")}
            onchange={pickLanguage}
        />
    </div>
    <div class="actions">
        <button class="btn-line" onclick={() => fileInput.click()}>{t("langImport")}</button>
        <button class="btn-line" onclick={exportTemplate}>{t("langExportTemplate")}</button>
        <button class="btn-line" disabled={!removable} onclick={confirmRemoveLanguage}>{t("langRemove")}</button>
    </div>
    {#if error}<div class="err">{error}</div>{/if}
    {#each notes as note, i (i)}<div class="note">{note}</div>{/each}
</div>

<style>
    .bar {
        padding-bottom: 1rem;
    }

    .lang {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 1rem;
        padding: 0.45rem 0.6rem;
        border-bottom: 1px solid var(--rule);
    }

    .m .lang {
        min-height: calc(var(--hit) * 1.5);
        padding: 0;
    }

    .actions {
        display: flex;
        flex-wrap: wrap;
        gap: 0.6rem;
        padding: 0.6rem 0 0;
    }

    .actions button {
        padding: 0.45rem 1rem;
    }

    .m .actions button {
        min-height: var(--hit);
    }

    .err,
    .note {
        padding-top: 0.4rem;
        font-size: var(--fs-xs);
        overflow-wrap: anywhere;
    }

    .err {
        color: var(--danger);
    }

    .note {
        color: var(--mute);
    }
</style>
