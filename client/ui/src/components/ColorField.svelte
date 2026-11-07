<script lang="ts">
    import type { Attachment } from "svelte/attachments";
    import { HUE_RANGE, argbHsva, hsvaArgb, hueCss, type Hsva } from "../lib/color";
    import { argbCss, argbHex, hexArgb } from "../lib/format";
    import { useApp } from "../lib/context";

    let { value, label, onchange }: { value: number; label: string; onchange: (argb: number) => void } = $props();
    const t = useApp().t;
    const GAP = 6;
    const id = $props.id();
    let trigger: HTMLButtonElement;
    let pop: HTMLDivElement;
    let open = $state(false);
    let hsva = $state<Hsva>(argbHsva(0));
    let base = $state(0);
    let rgb = $derived(argbCss(hsvaArgb({ ...hsva, a: 1 })));

    function applyHsva(next: Partial<Hsva>) {
        Object.assign(hsva, next);
        onchange(hsvaArgb(hsva));
    }

    function dragPicker(apply: (x: number, y: number) => void): Attachment<HTMLElement> {
        return (el) => {
            const at = (e: PointerEvent) => {
                const r = el.getBoundingClientRect();
                apply(
                    Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)),
                    Math.min(1, Math.max(0, (e.clientY - r.top) / r.height)),
                );
            };
            const down = (e: PointerEvent) => {
                at(e);
                el.setPointerCapture(e.pointerId);
            };
            const move = (e: PointerEvent) => el.hasPointerCapture(e.pointerId) && at(e);
            el.addEventListener("pointerdown", down);
            el.addEventListener("pointermove", move);
            return () => {
                el.removeEventListener("pointerdown", down);
                el.removeEventListener("pointermove", move);
            };
        };
    }

    function nudgeByKey(e: KeyboardEvent, axis: "s" | "v" | "h" | "a") {
        const dir = { ArrowRight: 1, ArrowUp: 1, ArrowLeft: -1, ArrowDown: -1 }[e.key];
        if (!dir) return;
        e.preventDefault();
        const span = axis === "h" ? HUE_RANGE : 1;
        applyHsva({ [axis]: Math.min(span, Math.max(0, hsva[axis] + dir * 0.02 * span)) });
    }

    function onBeforeToggle(e: ToggleEvent) {
        if (e.newState !== "open") return;
        base = value;
        hsva = argbHsva(value);
    }

    function onToggle(e: ToggleEvent) {
        open = e.newState === "open";
        if (!open) return;
        const r = trigger.getBoundingClientRect();
        const w = pop.offsetWidth,
            h = pop.offsetHeight;
        pop.style.left = `${Math.max(GAP, r.right - w)}px`;
        pop.style.top = `${innerHeight - r.bottom - GAP >= h ? r.bottom + GAP : r.top - GAP - h}px`;
    }

    function setHex(text: string) {
        const argb = hexArgb(text);
        if (argb === null) return;
        hsva = argbHsva(argb);
        onchange(argb);
    }
</script>

<svelte:window onresize={() => open && pop.hidePopover()} />
<svelte:document
    onscrollcapture={(e) => open && !pop.contains(e.target instanceof Node ? e.target : null) && pop.hidePopover()}
/>

<div class="field">
    <button
        class="swatch"
        class:open
        bind:this={trigger}
        style:--c={argbCss(value)}
        title={label}
        aria-label={label}
        popovertarget={id}
    ></button>
    <input
        class="f hex"
        value={argbHex(value)}
        spellcheck="false"
        aria-label={label}
        onchange={(e) => setHex(e.currentTarget.value)}
    />
</div>

<div
    class="pop"
    {id}
    popover="auto"
    style:--hue={hueCss(hsva.h)}
    style:--rgb={rgb}
    bind:this={pop}
    onbeforetoggle={onBeforeToggle}
    ontoggle={onToggle}
>
    <div
        class="sv"
        role="slider"
        tabindex="0"
        aria-label={label}
        aria-valuenow={Math.round(hsva.s * 100)}
        {@attach dragPicker((x, y) => applyHsva({ s: x, v: 1 - y }))}
        onkeydown={(e) => nudgeByKey(e, e.key === "ArrowUp" || e.key === "ArrowDown" ? "v" : "s")}
    >
        <i class="knob" style:left="{hsva.s * 100}%" style:top="{(1 - hsva.v) * 100}%"></i>
    </div>
    <div
        class="track hue"
        role="slider"
        tabindex="0"
        aria-label={t("colorHue")}
        aria-valuenow={Math.round(hsva.h)}
        {@attach dragPicker((x) => applyHsva({ h: x * HUE_RANGE }))}
        onkeydown={(e) => nudgeByKey(e, "h")}
    >
        <i class="thumb" style:left="{(hsva.h / HUE_RANGE) * 100}%"></i>
    </div>
    <div
        class="track alpha"
        role="slider"
        tabindex="0"
        aria-label={t("colorAlpha")}
        aria-valuenow={Math.round(hsva.a * 100)}
        {@attach dragPicker((x) => applyHsva({ a: x }))}
        onkeydown={(e) => nudgeByKey(e, "a")}
    >
        <i class="thumb" style:left="{hsva.a * 100}%"></i>
    </div>
    <div class="foot">
        <span class="cmp" style:--was={argbCss(base)} style:--now={argbCss(value)}><i></i><i></i></span>
        <span class="code">{argbHex(value)}</span>
        <span class="pct">{Math.round(hsva.a * 100)}%</span>
    </div>
</div>

<style>
    .field {
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }

    .swatch,
    .alpha,
    .cmp i {
        background-image:
            linear-gradient(45deg, #3a3a3a 25%, transparent 25%, transparent 75%, #3a3a3a 75%),
            linear-gradient(45deg, #3a3a3a 25%, #262626 25%, #262626 75%, #3a3a3a 75%);
        background-size: 8px 8px;
        background-position:
            0 0,
            4px 4px;
    }

    .swatch {
        position: relative;
        width: 2.2rem;
        height: 1.5rem;
        padding: 0;
        border: 0;
        transition: transform 0.15s var(--ease);
    }

    .swatch::after {
        content: "";
        position: absolute;
        inset: 0;
        background: var(--c);
        box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--fg) 18%, transparent);
    }

    .swatch:hover,
    .swatch.open {
        transform: scale(1.08);
    }

    .swatch.open::after {
        box-shadow: inset 0 0 0 1px var(--accent);
    }

    .hex {
        width: 7.5rem;
        padding: 0.2rem 0.4rem;
        font-variant-numeric: tabular-nums;
    }

    .pop {
        position: fixed;
        inset: auto;
        width: 15rem;
        margin: 0;
        padding: 0.8rem;
        border: 0;
        background: var(--bg);
        color: var(--fg);
        box-shadow: 0 0.8rem 2rem rgba(0, 0, 0, 0.5);
        opacity: 0;
        transform: translateY(-4px);
        transition:
            opacity var(--popover-time) var(--ease),
            transform var(--popover-time) var(--ease),
            overlay var(--popover-time) allow-discrete,
            display var(--popover-time) allow-discrete;
    }

    .pop:popover-open {
        opacity: 1;
        transform: none;
    }

    @starting-style {
        .pop:popover-open {
            opacity: 0;
            transform: translateY(-4px);
        }
    }

    .sv {
        position: relative;
        height: 9.5rem;
        background: linear-gradient(transparent, #000), linear-gradient(90deg, #fff, var(--hue));
        cursor: crosshair;
        touch-action: none;
    }

    .knob {
        position: absolute;
        width: 0.8rem;
        height: 0.8rem;
        margin: -0.4rem 0 0 -0.4rem;
        border-radius: 50%;
        background: var(--rgb);
        box-shadow:
            0 0 0 2px #fff,
            0 1px 4px rgba(0, 0, 0, 0.6);
        pointer-events: none;
    }

    .track {
        position: relative;
        height: 0.6rem;
        margin-top: 0.8rem;
        cursor: ew-resize;
        touch-action: none;
    }

    .hue {
        background: linear-gradient(90deg, #f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00);
    }

    .alpha::before {
        content: "";
        position: absolute;
        inset: 0;
        background: linear-gradient(90deg, transparent, var(--rgb));
    }

    .thumb {
        position: absolute;
        top: -0.2rem;
        bottom: -0.2rem;
        width: 3px;
        margin-left: -1.5px;
        background: #fff;
        box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.6);
        pointer-events: none;
    }

    .sv:focus-visible,
    .track:focus-visible {
        outline-offset: 2px;
    }

    .foot {
        display: flex;
        align-items: center;
        gap: 0.6rem;
        margin-top: 0.8rem;
        font-variant-numeric: tabular-nums;
    }

    .cmp {
        display: flex;
        width: 2.4rem;
        height: 1.2rem;
    }

    .cmp i {
        position: relative;
        flex: 1;
    }

    .cmp i::after {
        content: "";
        position: absolute;
        inset: 0;
        background: var(--was);
    }

    .cmp i + i::after {
        background: var(--now);
    }

    .code {
        flex: 1;
        color: var(--mute);
        font-size: var(--fs-sm);
    }

    .pct {
        color: var(--dim);
        font-size: var(--fs-sm);
    }
</style>
