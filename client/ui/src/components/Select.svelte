<script lang="ts" generics="T extends string">
    import Icon from "./Icon.svelte";

    let {
        options,
        value,
        label,
        onchange,
        progress,
    }: {
        options: { value: T; label: string }[];
        value: T;
        label: string;
        onchange: (value: T) => void;
        progress?: number;
    } = $props();
    const GAP = 4;
    const id = $props.id();
    let trigger: HTMLButtonElement;
    let list: HTMLDivElement;
    let open = $state(false);
    let active = $state(0);
    let current = $derived(options.find((o) => o.value === value));

    function placeList() {
        const r = trigger.getBoundingClientRect();
        const h = list.offsetHeight;
        const below = innerHeight - r.bottom - GAP >= h || r.top < h;
        list.style.left = `${r.left}px`;
        list.style.top = `${below ? r.bottom + GAP : r.top - GAP - h}px`;
        list.style.minWidth = `${r.width}px`;
        list.style.transformOrigin = below ? "top" : "bottom";
    }

    function hideList() {
        list.hidePopover();
        trigger.focus();
    }

    function pickOption(i: number) {
        hideList();
        if (options[i].value !== value) onchange(options[i].value);
    }

    function moveActive(step: number) {
        active = (active + step + options.length) % options.length;
        list.children[active].scrollIntoView({ block: "nearest" });
    }

    function onBeforeToggle(e: ToggleEvent) {
        if (e.newState === "open")
            active = Math.max(
                0,
                options.findIndex((o) => o.value === value),
            );
    }

    function onToggle(e: ToggleEvent) {
        open = e.newState === "open";
        if (!open) return;
        placeList();
        list.focus();
        list.children[active].scrollIntoView({ block: "nearest" });
    }

    function onTriggerKey(e: KeyboardEvent) {
        if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
        e.preventDefault();
        list.showPopover();
    }

    function onListKey(e: KeyboardEvent) {
        const keys: Record<string, () => void> = {
            ArrowDown: () => moveActive(1),
            ArrowUp: () => moveActive(-1),
            Home: () => moveActive(-active),
            End: () => moveActive(options.length - 1 - active),
            Enter: () => pickOption(active),
            " ": () => pickOption(active),
        };
        const run = keys[e.key];
        if (!run) return;
        e.preventDefault();
        run();
    }
</script>

<svelte:window onresize={() => open && list.hidePopover()} />
<svelte:document onscrollcapture={(e) => open && e.target !== list && list.hidePopover()} />

<button
    class="sel"
    class:open
    bind:this={trigger}
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-label={label}
    popovertarget={id}
    onkeydown={onTriggerKey}
>
    <span>{current?.label ?? value}</span><Icon name="down" />
    {#if progress !== undefined}<i class="load" style:width={`${progress * 100}%`}></i>{/if}
</button>
<div
    class="list"
    {id}
    popover="auto"
    role="listbox"
    tabindex="-1"
    aria-label={label}
    bind:this={list}
    onbeforetoggle={onBeforeToggle}
    ontoggle={onToggle}
    onkeydown={onListKey}
>
    {#each options as o, i (o.value)}
        <button
            role="option"
            tabindex="-1"
            aria-selected={o.value === value}
            class:active={i === active}
            onpointerenter={() => (active = i)}
            onclick={() => pickOption(i)}>{o.label}</button
        >
    {/each}
</div>

<style>
    .sel {
        position: relative;
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 0.6rem;
        width: 14rem;
        padding: 0.3rem 0.5rem 0.3rem 0.7rem;
        border: 0;
        border-bottom: 1px solid var(--rule);
        background: var(--field);
        text-align: left;
        text-shadow: var(--shadow);
        transition:
            border-color 0.2s,
            background-color 0.2s;
    }

    .sel:hover {
        border-bottom-color: var(--rule-strong);
    }

    .sel.open {
        border-bottom-color: var(--accent);
        background: var(--field-focus);
    }

    .sel span {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .sel :global(svg) {
        flex: none;
        width: var(--icon-sm);
        height: var(--icon-sm);
        fill: none;
        stroke: var(--mute);
        stroke-width: 2;
        transition: transform 0.25s var(--ease);
    }

    .sel.open :global(svg) {
        transform: rotate(180deg);
    }

    .load {
        position: absolute;
        left: 0;
        bottom: -1px;
        height: 2px;
        background: var(--fg);
        transition: width 0.3s;
    }

    .list {
        position: fixed;
        inset: auto;
        max-height: min(22rem, 60vh);
        margin: 0;
        padding: 0.3rem 0;
        overflow: auto;
        border: 0;
        background: var(--bg);
        color: var(--fg);
        box-shadow: 0 0.8rem 2rem rgba(0, 0, 0, 0.5);
        opacity: 0;
        transform: scaleY(0.92);
        transition:
            opacity var(--popover-time) var(--ease),
            transform var(--popover-time) var(--ease),
            overlay var(--popover-time) allow-discrete,
            display var(--popover-time) allow-discrete;
    }

    .list:popover-open {
        opacity: 1;
        transform: none;
    }

    @starting-style {
        .list:popover-open {
            opacity: 0;
            transform: scaleY(0.92);
        }
    }

    .list:focus {
        outline: 0;
    }

    [role="option"] {
        display: block;
        width: 100%;
        border: 0;
        background: none;
        text-align: left;
        padding: 0.35rem 0.9rem;
        color: var(--mute);
        cursor: pointer;
        white-space: nowrap;
    }

    [role="option"].active {
        background: color-mix(in srgb, var(--fg) 6%, transparent);
        color: var(--fg);
    }

    [role="option"][aria-selected="true"] {
        box-shadow: inset 2px 0 var(--accent);
        color: var(--fg);
    }
</style>
