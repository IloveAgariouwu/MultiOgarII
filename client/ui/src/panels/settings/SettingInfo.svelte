<script lang="ts">
    import { useApp } from "../../lib/context";
    import { hotkeyText } from "../../lib/keys";
    import { rangeText } from "../../lib/settings-format";
    import type { RowEntry } from "../../lib/settings-rows";
    import type { CategorySpec } from "../../lib/types";

    let { entry, category }: { entry: RowEntry | null; category: CategorySpec } = $props();
    const app = useApp();
    const t = app.t;
    const lookup = app.lookup;

    function textOf(key: string): string | null {
        return key in app.texts ? lookup(key) : null;
    }

    let flagKey = $derived(
        entry?.row.kind === "toggle"
            ? entry.row.key
            : entry?.row.kind === "group"
              ? (entry.row.spec.key ?? null)
              : null,
    );
    let defaultText = $derived.by(() => {
        if (!entry) return null;
        const row = entry.row;
        if (flagKey !== null) return t(app.defaults[flagKey] === true ? "demoOn" : "demoOff");
        if (row.kind === "range") return rangeText(row.spec, app.defaults[row.spec.key], app.ui, lookup);
        return null;
    });
    let keys = $derived.by(() => {
        if (!entry) return [];
        const key = entry.key;
        return [
            [t("setKeyTap"), app.hotkeys.toggle[key] ?? app.hotkeys.control[key] ?? 0],
            [t("setKeyHold"), app.hotkeys.hold[key] ?? 0],
        ].filter(([, code]) => Number(code) > 0);
    });
    let desc = $derived(entry ? textOf(`desc_${entry.key}`) : textOf(`catdesc_${category.id}`));
    let requires = $derived(entry ? app.ui.requires[entry.key] : undefined);
</script>

<div class="info">
    {#if entry}
        <h3>{entry.label}</h3>
        {#if desc}<p>{desc}</p>{/if}
        <dl>
            {#if defaultText !== null}<dt>{t("infoDefault")}</dt>
                <dd>{defaultText}</dd>{/if}
            {#each keys as [label, code] (label)}<dt>{label}</dt>
                <dd><kbd>{hotkeyText(Number(code))}</kbd></dd>{/each}
            {#if requires}<dt>{t("infoRequires")}</dt>
                <dd>{lookup(requires)}</dd>{/if}
            {#if app.ui.reloadRequired.includes(entry.key)}<dt>{t("infoReload")}</dt>
                <dd>{t("infoReloadNeeded")}</dd>{/if}
        </dl>
    {:else}
        <h3>{lookup(category.title)}</h3>
        <p class:dim={!desc}>{desc ?? t("infoPick")}</p>
    {/if}
</div>

<style>
    .info {
        overflow-y: auto;
        padding: 1rem 1.2rem;
        background: var(--box-back);
        background-image: var(--sheen);
        box-shadow: var(--box-line);
    }

    h3 {
        margin: 0 0 0.5rem;
        font-size: var(--fs-lg);
    }

    p {
        margin: 0 0 0.8rem;
        color: var(--fg);
        line-height: 1.6;
    }

    p.dim {
        color: var(--mute);
    }

    dl {
        display: grid;
        grid-template-columns: auto 1fr;
        gap: 0.35rem 1.2rem;
        margin: 0;
        font-size: var(--fs-sm);
    }

    dt {
        color: var(--mute);
    }

    dd {
        margin: 0;
    }

    kbd {
        padding: 0 0.35rem;
        border: 1px solid var(--rule);
        font: inherit;
    }
</style>
