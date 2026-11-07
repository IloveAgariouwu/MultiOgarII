<script lang="ts">
    import ColorField from "../../components/ColorField.svelte";
    import HotkeyInput from "../../components/HotkeyInput.svelte";
    import MouseInput from "../../components/MouseInput.svelte";
    import Select from "../../components/Select.svelte";
    import Toggle from "../../components/Toggle.svelte";
    import { useApp, useGame } from "../../lib/context";
    import { GOOGLE_FONTS, loadGoogleFont } from "../../lib/google-fonts";
    import { hotkeyText } from "../../lib/keys";
    import { isChoice, rangeParse, rangeShown, rangeText } from "../../lib/settings-format";
    import type { RowEntry } from "../../lib/settings-rows";
    import { HotkeyKind, type RangeSpec } from "../../lib/types";
    import ValueInput from "./ValueInput.svelte";

    let {
        entry,
        binding,
        selected,
        onlabel,
    }: { entry: RowEntry; binding: boolean; selected: boolean; onlabel: () => void } = $props();
    const SEGMENT_MAX = 4;
    const app = useApp();
    const game = useGame();
    const t = app.t;
    const lookup = app.lookup;
    let pendingFont = $state<{ value: string; progress: number } | null>(null);
    let fonts = $derived([
        { value: "", label: t("fontDefault") },
        ...GOOGLE_FONTS.map((f) => ({ value: f.name, label: f.name })),
    ]);
    let row = $derived(entry.row);
    let desc = $derived(gMobile && `desc_${entry.key}` in app.texts ? lookup(`desc_${entry.key}`) : null);
    let wide = $derived(
        gMobile &&
            (row.kind === "group" ||
                row.kind === "color" ||
                row.kind === "text" ||
                row.kind === "control" ||
                (row.kind === "range" && rangeWide(row.spec))),
    );

    function choiceOptions(spec: RangeSpec): { value: string; label: string }[] | null {
        if (!isChoice(spec)) return null;
        const out = [];
        for (let v = spec.min; v <= spec.max; v += spec.step)
            out.push({ value: String(v), label: rangeText(spec, v, app.ui, lookup) });
        return out;
    }

    function rangeWide(spec: RangeSpec): boolean {
        const options = choiceOptions(spec);
        return options === null || options.length <= SEGMENT_MAX;
    }
</script>

{#snippet name()}{entry.label}{#if app.core.needsReload(entry.key)}<span class="rl">*</span>{/if}{/snippet}

{#snippet plain()}
    <span class="lbl"
        ><span>{@render name()}</span>{#if desc !== null}<small class="desc">{desc}</small>{/if}</span
    >
{/snippet}

{#snippet bindLabel(keys: string[])}
    {#if gMobile}{@render plain()}{:else}<button class="lbl" aria-expanded={binding} onclick={onlabel}
            >{@render name()}{@render bound(keys)}</button
        >{/if}
{/snippet}

{#snippet bound(keys: string[])}
    <span class="bound">
        {#each keys as key (key)}
            {#if app.hotkeys.toggle[key] > 0}<kbd title={t("setKeyTap")}>{hotkeyText(app.hotkeys.toggle[key])}</kbd
                >{/if}
            {#if app.hotkeys.hold[key] > 0}<kbd class="hold" title={t("setKeyHold")}
                    >{hotkeyText(app.hotkeys.hold[key])}</kbd
                >{/if}
        {/each}
    </span>
{/snippet}

{#snippet keyInputs(key: string)}
    <HotkeyInput
        code={app.hotkeys.toggle[key]}
        label={t("setKeyTap")}
        placeholder={t("setKeyTap")}
        title={t("setKeyClearHint")}
        onset={(code) => app.setHotkey(HotkeyKind.Toggle, key, code)}
    />
    <HotkeyInput
        code={app.hotkeys.hold[key]}
        label={t("setKeyHold")}
        placeholder={t("setKeyHold")}
        title={t("setKeyClearHint")}
        onset={(code) => app.setHotkey(HotkeyKind.Hold, key, code)}
    />
{/snippet}

<div class="opt" class:binding class:selected class:m={gMobile} class:wide>
    {#if row.kind === "toggle"}
        {@render bindLabel([row.key])}
        <div class="ctl">
            {#if binding}{@render keyInputs(row.key)}{/if}
            <Toggle checked={app.flag(row.key)} label={entry.label} onchange={(v) => app.setSetting(row.key, v)} />
        </div>
    {:else if row.kind === "group"}
        {@const spec = row.spec}
        {@const keys = spec.key ? [spec.key, ...spec.keys] : spec.keys}
        {@render bindLabel(keys)}
        <div class="ctl">
            {#if !spec.key || app.flag(spec.key)}
                <span class="parts">
                    {#each spec.keys as key, i (key)}
                        <button
                            class="part"
                            aria-pressed={app.flag(key)}
                            title={lookup(key)}
                            onclick={() => app.setSetting(key, !app.flag(key))}
                            >{lookup(spec.labels?.[i] ?? key)}</button
                        >
                    {/each}
                </span>
            {/if}
            {#if spec.key}{@const parent = spec.key}<Toggle
                    checked={app.flag(parent)}
                    label={entry.label}
                    onchange={(v) => app.setSetting(parent, v)}
                />{/if}
        </div>
        {#if binding}
            <div class="keys">
                {#each keys as key (key)}<span>{lookup(key)}</span><span class="ctl">{@render keyInputs(key)}</span
                    >{/each}
            </div>
        {/if}
    {:else if row.kind === "range"}
        {@const spec = row.spec}
        {@const options = choiceOptions(spec)}
        {@render plain()}
        {#if options && options.length <= SEGMENT_MAX}
            {@const picked = options.findIndex((o) => Number(o.value) === Number(app.settings[spec.key]))}
            <span class="parts seg" style:--n={options.length} style:--i={picked}>
                <i class="thumb"></i>
                {#each options as o, i (o.value)}<button
                        class="part"
                        aria-pressed={i === picked}
                        onclick={() => app.setSetting(spec.key, o.value)}>{o.label}</button
                    >{/each}
            </span>
        {:else if options}
            <Select
                {options}
                value={String(Number(app.settings[spec.key]))}
                label={entry.label}
                onchange={(v) => app.setSetting(spec.key, v)}
            />
        {:else}
            <div class="rng">
                <input
                    class="slider"
                    type="range"
                    min={spec.min}
                    max={spec.max}
                    step={spec.step}
                    value={app.settings[spec.key]}
                    style:--p={`${((Number(app.settings[spec.key]) - spec.min) / (spec.max - spec.min)) * 100}%`}
                    aria-label={entry.label}
                    oninput={(e) => app.setSetting(spec.key, e.currentTarget.value)}
                />
                <output
                    ><ValueInput
                        text={rangeText(spec, app.settings[spec.key], app.ui, lookup)}
                        value={rangeShown(spec, Number(app.settings[spec.key]))}
                        label={entry.label}
                        title={t("setValueEditHint")}
                        onset={(v) => app.setSetting(spec.key, rangeParse(spec, v))}
                    /></output
                >
            </div>
        {/if}
    {:else if row.kind === "color"}
        {@render plain()}
        <ColorField value={app.colors[row.key]} label={entry.label} onchange={(argb) => app.setColor(row.key, argb)} />
    {:else if row.kind === "control"}
        {@render plain()}
        <span class="binds">
            <HotkeyInput
                code={app.hotkeys.control[row.key]}
                label={entry.label}
                onset={(code) => app.setHotkey(HotkeyKind.Control, row.key, code)}
            />
            <MouseInput
                code={app.hotkeys.mouse[row.key]}
                label={`${entry.label} ${t("setKeyMouse")}`}
                placeholder={t("setKeyMouse")}
                title={t("setMouseClearHint")}
                t={lookup}
                onset={(code) => app.setHotkey(HotkeyKind.Mouse, row.key, code)}
            />
        </span>
    {:else if row.kind === "slot"}
        {@render plain()}
        <Select
            options={[
                { value: "", label: t("lbtActNone") },
                ...app.groups.control.map((k) => ({ value: k, label: lookup(k) })),
            ]}
            value={String(app.settings[row.key])}
            label={entry.label}
            onchange={(v) => app.setSetting(row.key, v)}
        />
    {:else if row.kind === "font"}
        {@render plain()}
        <Select
            options={fonts}
            value={pendingFont?.value ?? String(app.settings[row.key])}
            label={entry.label}
            progress={pendingFont?.progress}
            onchange={async (v) => {
                pendingFont = { value: v, progress: 0 };
                const pick = pendingFont;
                const names = game.units.map((u) => u.name).join("");
                const shown = row.key === "UiFont" ? document.body.innerText : "";
                const ok = await loadGoogleFont(
                    v,
                    `${shown}${names} `,
                    (done, total) => (pick.progress = done / total),
                );
                if (pendingFont !== pick) return;
                if (ok) app.setSetting(row.key, v);
                pendingFont = null;
            }}
        />
    {:else if row.kind === "flag"}
        {@render plain()}
        <Toggle checked={app.flag(row.key)} label={entry.label} onchange={(v) => app.setSetting(row.key, v)} />
    {:else if row.kind === "text"}
        {@render plain()}
        <input
            class="f txt"
            value={app.settings[row.spec.key]}
            placeholder={row.spec.placeholder ? lookup(row.spec.placeholder) : ""}
            maxlength={row.spec.maxLength}
            title={row.spec.hint ? lookup(row.spec.hint) : ""}
            spellcheck="false"
            oninput={(e) => app.setSetting(row.spec.key, e.currentTarget.value)}
        />
    {/if}
</div>

<style>
    .opt {
        display: grid;
        grid-template-columns: 1fr auto;
        grid-template-rows: minmax(2.8rem, auto);
        align-items: center;
        gap: 1rem;
        min-height: 2.8rem;
        padding: 0 0.6rem;
        border-bottom: 1px solid color-mix(in srgb, var(--fg) 6%, transparent);
        transition:
            background-color 0.15s,
            box-shadow 0.15s;
    }

    .opt:hover {
        background: var(--row-hover);
    }

    .opt.selected,
    .opt.binding {
        background: var(--row-hover);
        box-shadow: inset 2px 0 var(--accent);
    }

    .lbl {
        padding: 0;
        border: 0;
        background: none;
        text-align: left;
        text-shadow: var(--shadow);
    }

    .rl {
        margin-left: 0.3rem;
        color: var(--accent);
    }

    .ctl {
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }

    .parts {
        display: flex;
        gap: 2px;
    }

    .part {
        position: relative;
        isolation: isolate;
        padding: 0.2rem 0.7rem;
        border: 1px solid var(--rule);
        background: none;
        color: var(--mute);
        font-size: var(--fs-sm);
        transition:
            color var(--switch-time) ease-out,
            border-color var(--switch-time) ease-out;
    }

    .part::before {
        content: "";
        position: absolute;
        inset: -1px;
        z-index: -1;
        background: var(--seg-on);
        opacity: 0;
        transition: opacity var(--switch-time) ease-out;
    }

    .part:hover {
        color: var(--fg);
    }

    .part:active {
        transform: translateY(1px);
    }

    .part[aria-pressed="true"] {
        border-color: var(--on-line);
        color: var(--on-seg);
    }

    .part[aria-pressed="true"]::before {
        opacity: 1;
    }

    .seg {
        position: relative;
        display: grid;
        grid-auto-columns: 1fr;
        grid-auto-flow: column;
        gap: 0;
        box-shadow: inset 0 0 0 1px var(--rule);
    }

    .seg .part {
        border: 0;
    }

    .seg .part::before {
        content: none;
    }

    .thumb {
        position: absolute;
        top: 0;
        bottom: 0;
        left: 0;
        width: calc(100% / var(--n));
        background: var(--seg-on);
        box-shadow: inset 0 0 0 1px var(--on-line);
        transform: translateX(calc(var(--i) * 100%));
        transition: transform var(--switch-time) var(--ease);
    }

    .bound {
        display: inline-flex;
        gap: 0.3rem;
        margin-left: 0.6rem;
        vertical-align: 0.1em;
    }

    kbd {
        padding: 0 0.35rem;
        box-shadow: inset 0 0 0 1px var(--rule);
        color: var(--mute);
        font: inherit;
        font-size: var(--fs-xs);
        line-height: 1.4;
        text-shadow: none;
    }

    kbd.hold {
        box-shadow:
            inset 0 -2px var(--rule-strong),
            inset 0 0 0 1px var(--rule);
    }

    .lbl:hover kbd,
    .binding kbd {
        color: var(--fg);
    }

    .binds {
        display: flex;
        gap: 0.4rem;
    }

    .keys {
        grid-column: 1 / -1;
        display: grid;
        grid-template-columns: 1fr auto;
        gap: 0.3rem 1rem;
        align-items: center;
        padding: 0 0 0.6rem 1rem;
        color: var(--mute);
        font-size: var(--fs-sm);
    }

    .rng {
        display: grid;
        grid-template-columns: 11rem 6.5rem;
        gap: 0.7rem;
        align-items: center;
    }

    .rng input {
        width: 100%;
    }

    output {
        text-align: right;
        color: var(--mute);
        font-variant-numeric: tabular-nums;
    }

    .txt {
        width: 22rem;
        padding: 0.25rem 0.5rem;
    }

    .opt.m {
        grid-template-columns: minmax(0, 1fr) auto;
        grid-template-rows: auto;
        gap: 0.4rem 1rem;
        min-height: calc(var(--hit) * 1.5);
        padding: 0.5rem 0;
        border-bottom-color: var(--rule);
    }

    .opt.m:hover,
    .opt.m.selected {
        background: none;
        box-shadow: none;
    }

    .opt.m.wide {
        grid-template-columns: minmax(0, 1fr);
    }

    .m .lbl {
        display: flex;
        flex-direction: column;
        min-width: 0;
    }

    .desc {
        margin-top: 0.15rem;
        color: var(--mute);
        font-size: var(--fs-xs);
        line-height: 1.4;
    }

    .m .part {
        min-height: var(--hit);
        padding: 0 0.8rem;
    }

    .m.wide .parts {
        flex-wrap: wrap;
    }

    .m :global(.sel) {
        width: 11rem;
    }

    .m .rng {
        grid-template-columns: minmax(0, 1fr) 5.5rem;
    }

    .m .rng input {
        height: var(--hit);
    }

    .m .rng input::-webkit-slider-thumb {
        width: 0.9rem;
        height: 1.5rem;
        margin-top: -0.625rem;
    }

    .m .rng input::-moz-range-thumb {
        width: 0.9rem;
        height: 1.5rem;
    }

    .m .txt {
        box-sizing: border-box;
        width: 100%;
        min-height: var(--hit);
    }
</style>
