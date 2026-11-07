<script lang="ts" module>
    import type { ProfileState, StageFocus } from "../lib/types";

    export interface WorldView {
        zoom: number;
        camera: "drift" | "stage";
        focus?: StageFocus;
        active?: boolean;
        overrides?: { key: string; json: string }[];
        onaim?: (x: number, y: number) => void;
    }
</script>

<script lang="ts">
    import { onMount } from "svelte";
    import { useApp, useGame, useSource } from "../lib/context";
    import { PreviewWorld, Renderer, World } from "../pkg/ogar_web";
    import { skinUrl } from "../lib/skin-images";
    import { TAB_FIELDS } from "../lib/profile-fields";
    import { cursorImageUri } from "../lib/settings-format";

    let { view, class: cls = "" }: { view: WorldView; class?: string } = $props();
    const app = useApp();
    const game = useGame();
    const source = useSource();
    let canvas: HTMLCanvasElement;
    const FRAME_RATES = [10, 15, 20, 30, 60];
    const WHEEL_DELTA_PER_NOTCH = 100;
    const WHEEL_LINES_PER_NOTCH = 3;
    const CURSOR_CROSS = 1;
    const CURSOR_IMAGE = 2;
    let cursorHotspot = $state<[number, number] | null>(null);

    $effect(() => {
        const uri = cursorImageUri(String(app.settings.CursorImageUri));
        cursorHotspot = null;
        if (app.num("CursorStyle") !== CURSOR_IMAGE || !uri) return;
        const img = new Image();
        img.onload = () => (cursorHotspot = [Math.round(img.naturalWidth / 2), Math.round(img.naturalHeight / 2)]);
        img.onerror = () => console.error(`cursor image failed to load: ${uri}`);
        img.src = uri;
        return () => {
            img.onload = null;
            img.onerror = null;
        };
    });

    let canvasCursor = $derived.by(() => {
        const style = app.num("CursorStyle");
        if (style === CURSOR_IMAGE && cursorHotspot)
            return `url(${JSON.stringify(cursorImageUri(String(app.settings.CursorImageUri)))}) ${cursorHotspot[0]} ${cursorHotspot[1]}, crosshair`;
        return style === CURSOR_CROSS || style === CURSOR_IMAGE ? "crosshair" : undefined;
    });

    $effect(() => {
        if (view.camera === "drift" && app.menuOpen) source.pointer(canvas.clientWidth / 2, canvas.clientHeight / 2);
    });

    onMount(() => {
        const live = view.camera === "drift";
        const seed = Date.now();
        let zoom = view.zoom;
        const previewWorld = new World();
        let preview: PreviewWorld | null = null;
        let applied = "";
        let profilesShown: ProfileState | null = null;
        let touchShown: { x: number; y: number } | null = null;
        const listeners = new AbortController();
        let renderer: Renderer | null = null;
        let raf = 0;
        let alive = true;
        const start = (r: Renderer) => {
            renderer = r;
            r.setZoom(view.zoom);
            const toWorld = (x: number, y: number) => r.screenToWorld(x, y);
            let followed: World | null = null;
            let frameIndex = 0;
            let renderMs = 0;
            let renderFrames = 0;
            let renderSince = performance.now();
            const fail = (e: unknown) => {
                cancelAnimationFrame(raf);
                console.error("renderer stopped", e);
                game.system(app.t("rendererStopped"));
            };
            const frame = (t: number) => {
                raf = requestAnimationFrame(frame);
                const begun = performance.now();
                if (live && app.pinchZoom !== 1) {
                    r.onPinch(app.pinchZoom);
                    app.pinchZoom = 1;
                }
                if (live && app.touchAim !== touchShown) {
                    touchShown = app.touchAim;
                    if (touchShown && !app.menuOpen) aim(touchShown.x, touchShown.y);
                }
                frameIndex++;
                if (zoom !== view.zoom) {
                    zoom = view.zoom;
                    r.setZoom(zoom);
                }
                const rate = FRAME_RATES[Number(app.settings.FrameRateOption)];
                if (frameIndex % (FRAME_RATES[FRAME_RATES.length - 1] / rate) !== 0) return;
                if (live) {
                    const world = source.world;
                    if (world !== followed) {
                        followed = world;
                        r.resetCamera();
                    }
                    if (app.menuOpen || app.replay.playback) r.clearPointer();
                    const shot = app.replay.playback ? app.replay.camera() : null;
                    if (shot)
                        r.setReplayCamera(
                            shot.eyeX,
                            shot.eyeY,
                            app.flag("AffectZoomingOnReplay") ? shot.scale : undefined,
                        );
                    else r.clearReplayCamera();
                    app.minimap.apply(r);
                    r.prepareFrame(begun, world, app.core.wasm);
                    source.frame(t, toWorld);
                    try {
                        r.draw(world);
                    } catch (e) {
                        fail(e);
                        return;
                    }
                    if (!app.replay.playback) {
                        const [x, y, scale] = r.cameraState();
                        source.recordCamera(x, y, scale);
                    }
                    const done = performance.now();
                    renderMs += done - begun;
                    renderFrames++;
                    if (done - renderSince >= 500) {
                        game.stats = { ...game.stats, render: renderMs / renderFrames };
                        renderMs = 0;
                        renderFrames = 0;
                        renderSince = done;
                    }
                } else {
                    if (view.active === false) return;
                    if (!preview) {
                        preview = new PreviewWorld(previewWorld, seed);
                        profilesShown = null;
                        r.resetCamera();
                        r.setReadOnly(true);
                    }
                    const overrides = view.overrides ?? [];
                    const want = overrides.map((o) => `${o.key}=${o.json}`).join("&");
                    if (want !== applied) {
                        r.clearOverrides();
                        for (const o of overrides) r.setOverride(o.key, o.json);
                        applied = want;
                    }
                    if (app.profiles !== profilesShown) {
                        profilesShown = app.profiles;
                        const me = profilesShown.entries[profilesShown.main];
                        const f = TAB_FIELDS[0];
                        preview.setSelfIdentity(
                            previewWorld,
                            me[f.name],
                            skinUrl(me[f.skin[0]]),
                            skinUrl(me[f.skin[1]] || me[f.skin[0]]),
                            skinUrl(me[f.self[0]]),
                            skinUrl(me[f.self[1]]),
                        );
                    }
                    preview.updateFrame(previewWorld, t);
                    const [x, y, span, ax, ay] = preview.focusCamera(view.focus ?? "scene", t);
                    r.setStage(x, y, (Math.min(canvas.clientWidth, canvas.clientHeight) / span) * view.zoom);
                    const [, , scale] = r.cameraState();
                    const px = canvas.clientWidth / 2 + (ax - x) * scale,
                        py = canvas.clientHeight / 2 + (ay - y) * scale;
                    r.setPointer(px, py);
                    view.onaim?.(px, py);
                    try {
                        r.renderFrame(t, previewWorld, app.core.wasm);
                    } catch (e) {
                        fail(e);
                    }
                }
            };
            raf = requestAnimationFrame(frame);
            const notches = (e: WheelEvent) =>
                -e.deltaY / (e.deltaMode === WheelEvent.DOM_DELTA_LINE ? WHEEL_LINES_PER_NOTCH : WHEEL_DELTA_PER_NOTCH);
            if (!live) {
                canvas.addEventListener(
                    "wheel",
                    (e) => {
                        e.preventDefault();
                        r.onWheel(notches(e));
                    },
                    { passive: false, signal: listeners.signal },
                );
                return;
            }
            const aim = (clientX: number, clientY: number) => {
                const rect = canvas.getBoundingClientRect();
                const x = clientX - rect.left,
                    y = clientY - rect.top;
                source.pointer(x, y);
                if (!app.replay.playback) r.setPointer(x, y);
            };
            window.addEventListener(
                "pointermove",
                (e) => {
                    if (app.menuOpen || gMobile) return;
                    aim(e.clientX, e.clientY);
                },
                { signal: listeners.signal },
            );
            window.addEventListener(
                "wheel",
                (e) => {
                    if (e.target === canvas && !app.menuOpen && !app.overlay) r.onWheel(notches(e));
                },
                { passive: true, signal: listeners.signal },
            );
        };
        Renderer.create(canvas, Number(app.settings.GpuCellRenderer)).then(
            (r) => (alive ? start(r) : r.free()),
            (e) => {
                console.error("renderer failed to start", e);
                game.system(app.t("rendererFailed"));
            },
        );
        return () => {
            alive = false;
            listeners.abort();
            cancelAnimationFrame(raf);
            renderer?.free();
            preview?.free();
            previewWorld.free();
        };
    });
</script>

<canvas bind:this={canvas} class={cls} style:cursor={canvasCursor}></canvas>
