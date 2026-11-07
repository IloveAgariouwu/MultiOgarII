import { getContext, setContext } from "svelte";
import type { Attachment } from "svelte/attachments";
import type { GameSource, GameState } from "./game.svelte";
import type { HudSlot } from "./hud-layout.svelte";
import type { MapView } from "./minimap-feed";
import type { ReplayState } from "./replay.svelte";
import type { AppState } from "./state.svelte";

export type HudPlacement = (slot: HudSlot) => Attachment<HTMLElement>;

const APP = Symbol("app");
const GAME = Symbol("game");
const SOURCE = Symbol("source");
const PLACEMENT = Symbol("placement");
const MAP_VIEW = Symbol("mapView");
const REPLAY = Symbol("replay");

export function contextMap(
    app: AppState,
    game: GameState,
    source: GameSource,
): Map<symbol, AppState | GameState | GameSource> {
    return new Map<symbol, AppState | GameState | GameSource>([
        [APP, app],
        [GAME, game],
        [SOURCE, source],
    ]);
}

export function useApp(): AppState {
    return getContext(APP);
}

export function useGame(): GameState {
    return getContext(GAME);
}

export function useSource(): GameSource {
    return getContext(SOURCE);
}

export function provideGame(game: GameState, source: GameSource): void {
    setContext(GAME, game);
    setContext(SOURCE, source);
}

export function useHudPlacement(): HudPlacement {
    const app = useApp();
    return getContext<HudPlacement | undefined>(PLACEMENT) ?? ((slot) => app.hud.slot(slot));
}

export function usePreviewMapView(): MapView | null {
    return getContext<MapView | undefined>(MAP_VIEW) ?? null;
}

export function provideHud(placement: HudPlacement, view: MapView): void {
    setContext(PLACEMENT, placement);
    setContext(MAP_VIEW, view);
}

export function useReplay(): ReplayState {
    return getContext<ReplayState | undefined>(REPLAY) ?? useApp().replay;
}

export function provideReplay(replay: ReplayState): void {
    setContext(REPLAY, replay);
}
