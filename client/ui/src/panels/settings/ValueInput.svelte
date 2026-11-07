<script lang="ts">
    import { autoselect } from "../../lib/focus";

    let {
        text,
        value,
        label,
        title,
        onset,
    }: { text: string; value: number; label: string; title: string; onset: (v: number) => void } = $props();
    let editing = $state(false);

    function finishEditing(el: HTMLInputElement, apply: boolean) {
        if (!editing) return;
        editing = false;
        if (!apply || el.value.trim() === "") return;
        const v = Number(el.value);
        if (Number.isFinite(v) && v !== value) onset(v);
    }

    function onkeydown(e: KeyboardEvent & { currentTarget: HTMLInputElement }) {
        e.stopPropagation();
        if (e.key === "Enter") finishEditing(e.currentTarget, true);
        if (e.key === "Escape") finishEditing(e.currentTarget, false);
    }
</script>

{#if editing}
    <input
        class="f val"
        type="number"
        step="any"
        {value}
        aria-label={label}
        {@attach autoselect}
        {onkeydown}
        onblur={(e) => finishEditing(e.currentTarget, true)}
    />
{:else}
    <button class="val" {title} aria-label={label} onclick={() => (editing = true)}>{text}</button>
{/if}

<style>
    .val {
        width: 100%;
        min-width: 0;
        padding: 0.1rem 0.3rem;
        text-align: right;
        font: inherit;
        font-variant-numeric: tabular-nums;
    }

    button.val {
        border: 0;
        background: none;
        color: inherit;
        cursor: text;
        text-shadow: var(--shadow);
    }

    button.val:hover {
        color: var(--fg);
        box-shadow: inset 0 -1px var(--rule-strong);
    }

    input.val {
        appearance: textfield;
    }

    input.val::-webkit-inner-spin-button,
    input.val::-webkit-outer-spin-button {
        -webkit-appearance: none;
        margin: 0;
    }
</style>
