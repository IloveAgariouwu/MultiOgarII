import { mount } from "svelte";
import App from "./App.svelte";
import { bootDone, bootStage } from "./lib/boot-screen";
import { CoreApi } from "./lib/core";
import { contextMap } from "./lib/context";
import { loadGoogleFont } from "./lib/google-fonts";
import { NetSource } from "./lib/net-source";
import { GameState } from "./lib/game.svelte";
import type { Texts } from "./lib/i18n";
import { savedServer } from "./lib/servers";
import { AppState } from "./lib/state.svelte";
import { Store } from "./lib/store";
import "./app.css";

const FONT_WAIT_MS = 5000;

export async function boot(texts: Texts): Promise<void> {
    bootStage(15, texts.bootCore);
    const core = await CoreApi.load();
    bootStage(60, texts.bootSettings);
    const store = new Store(core);
    store.load();
    const app = new AppState(core, store, texts, savedServer());
    const game = new GameState();
    const net = new NetSource(app, game);
    app.replay.bind(net.replays);
    net.start();
    net.connect(app.server);
    bootStage(70, texts.bootFonts);
    await Promise.race([
        Promise.all([
            document.fonts.load('500 16px "Overpass"'),
            document.fonts.load('700 16px "Overpass"'),
            loadGoogleFont(String(app.settings.UiFont), Object.values(texts).join("")),
            loadGoogleFont(String(app.settings.CellFont)),
            ...Object.values(app.hud.fonts).map((font) => loadGoogleFont(font, Object.values(texts).join(""))),
        ]),
        new Promise((resolve) => setTimeout(resolve, FONT_WAIT_MS)),
    ]);
    mount(App, { target: document.body, context: contextMap(app, game, net) });
    await bootDone();
}
