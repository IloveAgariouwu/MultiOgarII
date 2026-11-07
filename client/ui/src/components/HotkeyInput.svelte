<script lang="ts">
    import { captureCode, hotkeyText } from "../lib/keys";

    let {
        code,
        label,
        placeholder = "",
        title = "",
        onset,
    }: {
        code: number;
        label: string;
        placeholder?: string;
        title?: string;
        onset: (code: number) => boolean;
    } = $props();
    let rejected = $state(false);

    function onkeydown(e: KeyboardEvent & { currentTarget: HTMLInputElement }) {
        e.preventDefault();
        e.stopPropagation();
        if (e.key === "Escape") return e.currentTarget.blur();
        const next = captureCode(e);
        if (next === null) return;
        rejected = !onset(next);
        if (!rejected) e.currentTarget.blur();
    }
</script>

<input
    class="hk"
    class:rejected
    readonly
    value={hotkeyText(code)}
    aria-label={label}
    {placeholder}
    {title}
    {onkeydown}
    onblur={() => (rejected = false)}
/>

<style>
    .hk {
        width: 5.4rem;
        padding: 0.2rem 0.4rem;
        border: 1px solid var(--rule);
        background: var(--field);
        text-align: center;
        font-size: var(--fs-sm);
        caret-color: transparent;
        cursor: pointer;
    }

    .hk::placeholder {
        color: var(--dim);
    }

    .hk:focus {
        outline: 0;
        border-color: var(--accent);
    }

    .hk.rejected {
        border-color: var(--danger);
    }
</style>
