<script lang="ts">
    import type { Component } from "svelte";
    import { useApp, useGame } from "./lib/context";
    import { argbCss } from "./lib/format";
    import { loadGoogleFont } from "./lib/google-fonts";
    import { menuPolicy } from "./lib/menu-policy.svelte";
    import { FROST_IN, PANEL_OUT, frost, frostFilter } from "./lib/transitions";
    import type { Overlay } from "./lib/state.svelte";
    import Chat from "./panels/chat/Chat.svelte";
    import Hud from "./panels/hud/Hud.svelte";
    import HudEditBar from "./panels/hud/HudEditBar.svelte";
    import GlobalInput from "./panels/GlobalInput.svelte";
    import MacroRecordBar from "./panels/MacroRecordBar.svelte";
    import Menu from "./panels/Menu.svelte";
    import MobileMenu from "./panels/mobile/MobileMenu.svelte";
    import ProfilePicker from "./panels/ProfilePicker.svelte";
    import ReplayGallery from "./panels/ReplayGallery.svelte";
    import ServerPicker from "./panels/ServerPicker.svelte";
    import Settings from "./panels/settings/Settings.svelte";
    import Touch from "./panels/Touch.svelte";
    import VideoExportBar from "./panels/VideoExportBar.svelte";
    import LiveWorld from "./panels/LiveWorld.svelte";

    const OVERLAYS: Record<NonNullable<Overlay>, Component> = {
        profiles: ProfilePicker,
        servers: ServerPicker,
        settings: Settings,
        replays: ReplayGallery,
    };

    const app = useApp();
    const game = useGame();
    let inGame = $derived(!app.menuOpen || app.hud.editing);
    let frosted = $derived((app.menuOpen && !app.hud.editing) || app.overlay !== null);

    $effect(() => {
        const root = document.documentElement.style;
        root.setProperty("--bg", argbCss(app.colors.clThemeBase));
        root.setProperty("--fg", argbCss(app.colors.clThemeText));
        root.setProperty("--accent", argbCss(app.colors.clThemeAccent));
    });

    $effect(() => {
        const font = String(app.settings.UiFont);
        loadGoogleFont(font, document.body.innerText).then(() => {
            if (String(app.settings.UiFont) !== font) return;
            if (font) document.documentElement.style.setProperty("--font", `${JSON.stringify(font)}, var(--font-base)`);
            else document.documentElement.style.removeProperty("--font");
        });
    });

    $effect(() => {
        loadGoogleFont(String(app.settings.CellFont));
    });

    menuPolicy(app, game);
</script>

<GlobalInput />
<LiveWorld />
{#if frosted}<div
        class="frost"
        style:backdrop-filter={frostFilter(1)}
        in:frost={FROST_IN}
        out:frost={PANEL_OUT}
    ></div>{/if}

{#if inGame && !app.overlay}
    <Hud />
    {#if gMobile}<Touch />{/if}
    {#if game.bench}<div class="bench">{app.t("benchOverlay")} · {game.stats.fps} fps</div>{/if}
{/if}

<Chat />

{#if app.menuOpen && app.overlay === null && !app.hud.editing}
    {#if gMobile}<MobileMenu />{:else}<Menu />{/if}
{/if}
{#if app.hud.editing && !app.overlay}<HudEditBar />{/if}
{#if app.recorder.recording}<MacroRecordBar macro={app.recorder.recording} />{/if}
{#if app.video.progress !== null && !app.video.shown}<VideoExportBar progress={app.video.progress} />{/if}
{#if app.overlay}
    {@const View = OVERLAYS[app.overlay]}
    <View />
{/if}

<style>
    .frost {
        position: fixed;
        inset: 0;
        background: var(--backdrop);
    }

    .bench {
        position: fixed;
        left: 50%;
        top: 40%;
        transform: translateX(-50%);
        color: var(--accent);
        font-size: var(--fs-lg);
        text-shadow: var(--shadow);
    }
</style>
