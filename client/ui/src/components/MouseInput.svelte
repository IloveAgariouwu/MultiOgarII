<script lang="ts">
    import { CLEAR, MOUSE_LEFT, NAV_BUTTONS, mouseCode, mouseText } from "../lib/keys";

    let {
        code,
        label,
        placeholder = "",
        title = "",
        t,
        onset,
    }: {
        code: number;
        label: string;
        placeholder?: string;
        title?: string;
        t: (key: string) => string;
        onset: (code: number) => boolean;
    } = $props();
    let listening = $state(false);
    let bound = false;
    let rejected = $state(false);

    function stopListening() {
        listening = false;
    }

    function onpointerdown(e: PointerEvent) {
        if (!listening) return;
        e.preventDefault();
        e.stopPropagation();
        bound = true;
        rejected = !onset(mouseCode(e));
        listening = false;
    }

    function onpointerup(e: PointerEvent) {
        if (NAV_BUTTONS.has(e.button)) e.preventDefault();
        if (bound) {
            bound = false;
            return;
        }
        if (e.button === MOUSE_LEFT) listening = true;
    }

    function onkeydown(e: KeyboardEvent) {
        if (!listening) return;
        e.preventDefault();
        e.stopPropagation();
        if (e.key === "Backspace" || e.key === "Delete") rejected = !onset(CLEAR);
        listening = false;
    }
</script>

<button
    type="button"
    class="hk"
    class:listening
    class:rejected
    aria-label={label}
    {title}
    {onpointerdown}
    {onpointerup}
    {onkeydown}
    onblur={stopListening}
    oncontextmenu={(e) => e.preventDefault()}
    onauxclick={(e) => e.preventDefault()}
    onmouseup={(e) => NAV_BUTTONS.has(e.button) && e.preventDefault()}
    >{listening ? t("mouseListening") : mouseText(code, t) || placeholder}</button
>

<style>
    .hk {
        width: 5.4rem;
        padding: 0.2rem 0.4rem;
        border: 1px solid var(--rule);
        background: var(--field);
        color: var(--fg);
        text-align: center;
        font-size: var(--fs-sm);
        cursor: pointer;
    }

    .hk.listening {
        border-color: var(--accent);
        color: var(--accent);
    }

    .hk.rejected {
        border-color: var(--danger);
    }
</style>
