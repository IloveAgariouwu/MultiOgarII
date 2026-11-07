<script lang="ts">
    import HotkeyInput from "../../components/HotkeyInput.svelte";
    import Icon from "../../components/Icon.svelte";
    import Select from "../../components/Select.svelte";
    import { useApp } from "../../lib/context";
    import type { MacroDef, MacroPhase } from "../../lib/game.svelte";
    import { MOUSE_LEFT } from "../../lib/keys";

    const UNBOUND = -1;
    const PHASES: MacroPhase[] = ["tap", "down", "up"];
    const app = useApp();
    const t = app.t;
    const lookup = app.lookup;
    const rec = app.recorder;
    const limits = app.limits;
    const actions = app.core.macroActions().map((a) => ({ value: a, label: lookup(a) }));
    const phases = PHASES.map((p) => ({ value: p, label: t(`macroPhase_${p}`) }));
    let error = $state("");
    let drag = $state<{ from: number; gap: number; startY: number; dy: number; mids: number[] } | null>(null);
    const rows: HTMLElement[] = [];
    const target = $derived(drag && drag.gap !== drag.from && drag.gap !== drag.from + 1 ? drag.gap : -1);

    function grabMacro(e: PointerEvent & { currentTarget: HTMLButtonElement }, from: number) {
        if (e.button !== MOUSE_LEFT) return;
        e.currentTarget.setPointerCapture(e.pointerId);
        const mids = rows.slice(0, app.macros.length).map((row) => {
            const r = row.getBoundingClientRect();
            return r.top + r.height / 2;
        });
        drag = { from, gap: from, startY: e.clientY, dy: 0, mids };
    }

    function followDrag(e: PointerEvent) {
        if (!drag) return;
        const y = e.clientY;
        drag.dy = y - drag.startY;
        drag.gap = drag.mids.filter((m) => m < y).length;
    }

    function dropMacro() {
        if (!drag) return;
        const { from, gap } = drag;
        drag = null;
        const to = gap > from ? gap - 1 : gap;
        if (to !== from) app.moveMacro(from, to);
    }

    function blank(): MacroDef {
        return {
            name: `${t("macroDefaultName")} ${app.macros.length + 1}`,
            trigger: UNBOUND,
            repeat: 1,
            steps: [{ action: actions[0].value, phase: "tap", delayMs: 0 }],
        };
    }

    function saveDraft(draft: MacroDef) {
        error = app.saveMacro(rec.index, draft);
        if (!error) rec.close();
    }

    function setTrigger(index: number, code: number): boolean {
        return !app.saveMacro(index, { ...app.macros[index], trigger: code });
    }

    function startRecording() {
        rec.start();
        app.open(null);
    }
</script>

<section class="macros">
    <header>
        <h3>{t("macroTitle")}</h3>
        <button class="btn-line" disabled={!!rec.draft} onclick={() => rec.edit(-1, blank())}>{t("macroAdd")}</button>
    </header>

    {#each app.macros as m, i (i)}
        <div
            class="item"
            bind:this={rows[i]}
            class:lifted={drag?.from === i}
            class:drop-before={target === i}
            class:drop-after={target === app.macros.length && i === app.macros.length - 1}
            style:transform={drag?.from === i ? `translateY(${drag.dy}px)` : null}
        >
            <button
                class="icon grip"
                title={t("macroReorder")}
                aria-label={t("macroReorder")}
                disabled={!!rec.draft}
                onpointerdown={(e) => grabMacro(e, i)}
                onpointermove={followDrag}
                onpointerup={dropMacro}
                onpointercancel={() => (drag = null)}><Icon name="grip" /></button
            >
            <span class="name">{m.name}</span>
            <span class="sum"
                >{m.steps.length}
                {t("macroSteps")}{#if m.repeat > 1}
                    × {m.repeat}{/if}</span
            >
            <HotkeyInput code={m.trigger} label={m.name} onset={(code) => setTrigger(i, code)} />
            <button class="btn-line" disabled={!!rec.draft} onclick={() => rec.edit(i, m)}>{t("macroEdit")}</button>
            <button
                class="icon"
                title={t("macroRemove")}
                aria-label={t("macroRemove")}
                onclick={() => app.removeMacro(i)}><Icon name="trash" /></button
            >
        </div>
    {:else}
        {#if !rec.draft}<p class="empty">{t("macroEmpty")}</p>{/if}
    {/each}

    {#if rec.draft}
        {@const draft = rec.draft}
        <div class="editor">
            <div class="meta">
                <label
                    >{t("macroName")}<input
                        class="f"
                        maxlength={limits.macroNameMax}
                        bind:value={draft.name}
                        onkeydown={(e) => e.stopPropagation()}
                    /></label
                >
                <label
                    >{t("macroRepeat")}<input
                        class="f num"
                        type="number"
                        min="1"
                        max={limits.macroMaxRepeat}
                        bind:value={draft.repeat}
                    /></label
                >
                <span class="trig"
                    >{t("macroTrigger")}<HotkeyInput
                        code={draft.trigger}
                        label={t("macroTrigger")}
                        onset={(code) => ((draft.trigger = code), true)}
                    /></span
                >
            </div>
            <ol>
                {#each draft.steps as step, i (i)}
                    <li>
                        <span class="no">{i + 1}</span>
                        <Select
                            options={actions}
                            value={step.action}
                            label={t("macroAction")}
                            onchange={(v) => (step.action = v)}
                        />
                        <Select
                            options={phases}
                            value={step.phase}
                            label={t("macroPhase")}
                            onchange={(v) => (step.phase = v)}
                        />
                        <label class="delay"
                            ><input
                                class="f num"
                                type="number"
                                min="0"
                                max={limits.macroMaxDelayMs}
                                step={10}
                                bind:value={step.delayMs}
                            />ms</label
                        >
                        <button
                            class="icon"
                            title={t("macroRemoveStep")}
                            aria-label={t("macroRemoveStep")}
                            onclick={() => draft.steps.splice(i, 1)}><Icon name="close" /></button
                        >
                    </li>
                {/each}
            </ol>
            <div class="bar">
                <button
                    class="btn-line"
                    onclick={() => draft.steps.push({ action: actions[0].value, phase: "tap", delayMs: 50 })}
                    >{t("macroAddStep")}</button
                >
                <button class="btn-line" onclick={startRecording}>{t("macroRecord")}</button>
                <span class="sp"></span>
                {#if error}<span class="err">{error}</span>{/if}
                <button class="btn-line" onclick={() => ((error = ""), rec.close())}>{t("macroCancel")}</button>
                <button class="btn-accent" onclick={() => saveDraft(draft)}>{t("macroSave")}</button>
            </div>
        </div>
    {/if}
</section>

<style>
    .macros {
        margin-bottom: 1.2rem;
        padding-bottom: 0.8rem;
        box-shadow: inset 0 -1px var(--rule);
    }

    header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        margin-bottom: 0.4rem;
    }

    h3 {
        margin: 0;
        font-size: var(--fs-lg);
    }

    .item {
        display: grid;
        position: relative;
        grid-template-columns: auto minmax(0, 1fr) auto auto auto auto;
        gap: 0.7rem;
        align-items: center;
        min-height: 2.6rem;
        padding: 0 0.6rem;
        border-bottom: 1px solid color-mix(in srgb, var(--fg) 6%, transparent);
    }

    .item.lifted {
        z-index: 1;
        background: var(--well);
    }

    .item.drop-before::before,
    .item.drop-after::after {
        content: "";
        position: absolute;
        z-index: 2;
        left: 0;
        right: 0;
        height: 2px;
        background: var(--fg);
        pointer-events: none;
    }

    .item.drop-before::before {
        top: -1px;
    }

    .item.drop-after::after {
        bottom: -1px;
    }

    .grip {
        cursor: grab;
        touch-action: none;
    }

    .lifted .grip {
        cursor: grabbing;
    }

    .name {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .trig {
        display: flex;
        align-items: center;
        gap: 0.4rem;
    }

    .sum,
    .empty,
    .trig {
        color: var(--mute);
        font-size: var(--fs-sm);
    }

    .icon {
        width: 1.8rem;
        height: 1.8rem;
    }

    .editor {
        margin-top: 0.6rem;
        padding: 0.8rem;
        box-shadow: inset 0 0 0 1px var(--rule);
    }

    .meta {
        display: flex;
        align-items: center;
        gap: 1.2rem;
        margin-bottom: 0.6rem;
    }

    .meta label,
    .delay {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        color: var(--mute);
        font-size: var(--fs-sm);
    }

    .num {
        width: 5rem;
    }

    ol {
        display: flex;
        flex-direction: column;
        gap: 0.3rem;
        margin: 0;
        padding: 0;
        list-style: none;
    }

    li {
        display: grid;
        grid-template-columns: 1.6rem minmax(0, 1fr) 9rem auto auto;
        gap: 0.6rem;
        align-items: center;
    }

    .no {
        color: var(--dim);
        font-size: var(--fs-sm);
        text-align: right;
    }

    .bar {
        display: flex;
        align-items: center;
        gap: 0.6rem;
        margin-top: 0.8rem;
    }

    .sp {
        flex: 1;
    }

    .err {
        color: var(--danger);
        font-size: var(--fs-xs);
    }
</style>
