<script lang="ts">
    import WorldCanvas, { type WorldView } from "../../components/WorldCanvas.svelte";
    import PreviewHud from "./PreviewHud.svelte";
    import { useApp } from "../../lib/context";
    import { cursorImageUri } from "../../lib/settings-format";
    import {
        previewKind,
        stageFocus,
        type HudPart,
        type HudPreview,
        type SettingsMap,
        type StageFocus,
    } from "../../lib/types";

    const CELLS_ONLY = [
        { key: "ShowCoord", json: "false" },
        { key: "fieldBorderSize", json: "0" },
        { key: "ShowfieldBackImage", json: "false" },
    ];

    const WHOLE_FIELD: StageFocus[] = ["border", "grid", "field"];
    const HUD_PARTS: Record<HudPreview, HudPart[]> = {
        hud: ["chat", "rail", "self", "status", "map"],
        map: ["map"],
        rail: ["rail"],
        self: ["self"],
        client: ["client"],
        server: ["server"],
        chat: ["chat"],
        mention: ["mention"],
        replay: ["replay"],
    };

    const CURSOR_CROSS = 1;
    const CURSOR_IMAGE = 2;
    const CURSOR_MAX_PX = 128;
    const app = useApp();

    let { preview, overrides }: { preview: string; overrides: SettingsMap } = $props();
    let kind = $derived(previewKind(preview));
    let stage = $derived(kind !== "none" && stageFocus(kind) ? kind : null);
    let parts = $derived(kind === "none" || stageFocus(kind) ? [] : HUD_PARTS[kind]);
    let staged = $derived(stage !== null);
    let view = $derived<WorldView>({
        zoom: 1,
        camera: "stage",
        focus: stage ?? "scene",
        active: staged,
        overrides: [
            ...(stage !== null && WHOLE_FIELD.includes(stage) ? [] : CELLS_ONLY),
            ...Object.entries(overrides).map(([key, value]) => ({ key, json: JSON.stringify(value) })),
        ],
        onaim: (x, y) => {
            if (!aim || aim[0] !== x || aim[1] !== y) aim = [x, y];
        },
    });
    let aim = $state<[number, number] | null>(null);
    let cursorStyle = $derived(app.num("CursorStyle"));
    let cursorImage = $derived(cursorImageUri(String(app.settings.CursorImageUri)));
    let imageReady = $state(false);

    $effect(() => {
        const uri = cursorImage;
        imageReady = false;
        if (cursorStyle !== CURSOR_IMAGE || !uri) return;
        const img = new Image();
        img.onload = () => (imageReady = img.naturalWidth <= CURSOR_MAX_PX && img.naturalHeight <= CURSOR_MAX_PX);
        img.src = uri;
        return () => (img.onload = null);
    });
    let wanted = $state(false);

    $effect(() => {
        if (staged) wanted = true;
    });
</script>

<div class="pv" class:off={!staged} class:split={staged && parts.length > 0}>
    {#if wanted}<WorldCanvas {view} class="pvcanvas" />{/if}
    {#if stage !== null && aim && !WHOLE_FIELD.includes(stage)}<div
            class="aimcursor"
            style:left="{aim[0]}px"
            style:top="{aim[1]}px"
        >
            {#if cursorStyle === CURSOR_IMAGE && imageReady}<img class="centered" src={cursorImage} alt="" />
            {:else if cursorStyle === CURSOR_CROSS || cursorStyle === CURSOR_IMAGE}<svg
                    class="centered"
                    width="23"
                    height="23"
                    viewBox="0 0 23 23"
                    aria-hidden="true"
                    ><path d="M11.5 1v21M1 11.5h21" stroke="#fff" stroke-width="3" /><path
                        d="M11.5 1v21M1 11.5h21"
                        stroke="#000"
                        stroke-width="1"
                    /></svg
                >
            {:else}<svg width="14" height="21" viewBox="0 0 14 21" aria-hidden="true"
                    ><path
                        d="M0.5 0.5v17l4.2-4.1 2.9 6.6 2.4-1-2.9-6.5h5.9z"
                        fill="#000"
                        stroke="#fff"
                        stroke-linejoin="round"
                    /></svg
                >{/if}
        </div>{/if}
    {#if parts.length}<div class="partlist">
            <PreviewHud {parts} />
        </div>{/if}
</div>

<style>
    .pv {
        position: relative;
        min-height: 0;
        overflow: hidden;
        background: var(--box-back);
        background-image: var(--sheen);
        box-shadow: var(--box-line);
    }

    .pv :global(.pvcanvas) {
        position: absolute;
        inset: 0;
        width: 100%;
        height: 100%;
        transition: opacity 0.25s;
    }

    .pv.off :global(.pvcanvas) {
        opacity: 0;
    }

    .aimcursor {
        position: absolute;
        pointer-events: none;
    }

    .aimcursor > * {
        display: block;
    }

    .centered {
        transform: translate(-50%, -50%);
    }

    .partlist {
        position: absolute;
        inset: 0;
    }

    .pv.split :global(.pvcanvas) {
        height: 50%;
    }

    .pv.split .partlist {
        top: 50%;
    }
</style>
