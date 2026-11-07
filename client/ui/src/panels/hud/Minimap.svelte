<script lang="ts">
    import { onMount } from "svelte";
    import { useApp, useGame, useHudPlacement, usePreviewMapView, useSource } from "../../lib/context";
    import { dateTimeStamp } from "../../lib/format";
    import { loadGoogleFont } from "../../lib/google-fonts";
    import { MinimapFeed } from "../../lib/minimap-feed";
    import { Renderer } from "../../pkg/ogar_web";

    const CANVAS2D_RENDERER = 3;
    const app = useApp();
    const game = useGame();
    const source = useSource();
    const placement = useHudPlacement();
    const preview = usePreviewMapView();
    const feed = preview ? new MinimapFeed() : app.minimap;
    let map: HTMLDivElement;
    let radar: HTMLDivElement;
    let canvas: HTMLCanvasElement | undefined = $state();
    let size = $state(0);
    let deskSize = $state(parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--hud-box-w")));
    let now = $state(new Date());
    let text = $derived(gMobile ? 1 : (app.hud.texts.map ?? 1));
    let shrink = $derived(gMobile ? size / deskSize : 1);

    let family = $derived((gMobile ? undefined : app.hud.fonts.map) ?? "");
    let nameFamily = $derived(family || String(app.settings.UiFont));

    $effect(() => {
        if (nameFamily) loadGoogleFont(nameFamily);
    });

    $effect(() => feed.style(text, shrink, family, !game.playing, !app.replay.playback));

    $effect(() => feed.units(game.units));

    function measureFeed(): void {
        if (preview) {
            if (size) feed.place(0, 0, size);
            else feed.clear();
            return;
        }
        const r = radar.getBoundingClientRect();
        if (r.width > 0) feed.place(r.left, r.top, r.width);
        else feed.clear();
    }

    function radarMouse(e: MouseEvent, down: boolean) {
        if (gMobile || app.menuOpen || app.overlay || app.replay.playback) return;
        source.mouse(e.button, down);
    }

    onMount(() => {
        const clock = setInterval(() => (now = new Date()), 1000);
        const resize = new ResizeObserver((entries) => {
            const box = entries.find((e) => e.target === radar);
            if (box) size = box.contentRect.width;
            measureFeed();
        });
        resize.observe(radar);
        resize.observe(map);
        const moved = new MutationObserver(measureFeed);
        moved.observe(map, { attributes: true, attributeFilter: ["style", "class"] });
        window.addEventListener("resize", measureFeed);
        let raf = 0;
        let alive = true;
        let own: Renderer | null = null;
        if (preview && canvas) {
            feed.view(preview);
            Renderer.create(canvas, CANVAS2D_RENDERER).then(
                (r) => {
                    if (!alive) {
                        r.free();
                        return;
                    }
                    own = r;
                    r.setMinimapOnly(true);
                    r.setReadOnly(true);
                    const frame = (t: number) => {
                        raf = requestAnimationFrame(frame);
                        feed.cells(source.world.minimapCells());
                        feed.apply(r);
                        r.renderFrame(t, source.world, app.core.wasm);
                    };
                    raf = requestAnimationFrame(frame);
                },
                (e) => console.error("minimap preview renderer failed to start", e),
            );
        }
        return () => {
            alive = false;
            clearInterval(clock);
            resize.disconnect();
            moved.disconnect();
            window.removeEventListener("resize", measureFeed);
            cancelAnimationFrame(raf);
            own?.free();
            feed.clear();
        };
    });
</script>

<svelte:window
    onresize={() => (deskSize = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--hud-box-w")))}
/>

<div class="map" bind:this={map} {@attach placement("map")}>
    <div class="bar">
        <span class="time">{dateTimeStamp(now)}</span>
    </div>
    <div
        class="radar"
        bind:this={radar}
        role="presentation"
        onmousedown={(e) => radarMouse(e, true)}
        onmouseup={(e) => radarMouse(e, false)}
    >
        {#if preview}<canvas bind:this={canvas}></canvas>{/if}
    </div>
</div>

<style>
    .map {
        position: absolute;
        right: calc(2rem - var(--scrim-x));
        bottom: 1.2rem;
        width: var(--hud-box-w);
    }

    .bar {
        display: flex;
        margin-bottom: 0.3rem;
    }

    .time {
        color: var(--lb-head);
        font-size: var(--fs-sm);
        font-variant-numeric: tabular-nums;
    }

    .radar {
        position: relative;
        width: var(--hud-box-w);
        height: var(--hud-box-w);
    }

    canvas {
        position: absolute;
        inset: 0;
        display: block;
        width: 100%;
        height: 100%;
    }

    :global(html.mobile) .map {
        top: var(--mm-top);
        left: var(--hud-left);
        right: auto;
        bottom: auto;
        width: var(--mm-size);
    }

    :global(html.mobile) .radar {
        width: var(--mm-size);
        height: var(--mm-size);
    }

    :global(html.mobile) .bar {
        display: none;
    }
</style>
