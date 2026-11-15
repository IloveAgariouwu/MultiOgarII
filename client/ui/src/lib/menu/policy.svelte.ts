import type { GameState } from "../app/game.svelte";
import type { AppState } from "../app/state.svelte";

export function menuPolicy(app: AppState, game: GameState): void {
    let seenDeath = 0;
    let enteredTabs = [false, false];

    $effect(() => {
        const death = game.death;
        if (!death || death.id === seenDeath) return;
        seenDeath = death.id;
        if (game.tabs.intent !== "play" && !app.settings.HideMenuAfterDeath) app.menuOpen = true;
    });

    $effect(() => {
        const entered = game.tabs.tabs.map((v) => v.phase === "joining" || v.phase === "alive");
        if (entered.some((on, i) => on && !enteredTabs[i])) app.menuOpen = false;
        enteredTabs = entered;
    });
}
