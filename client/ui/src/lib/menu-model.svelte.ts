import { livePopulation } from "./format";
import type { GameSource, GameState } from "./game.svelte";
import { TAB_FIELDS, profileIndex, type TabFields } from "./profile-fields";
import type { AppState } from "./state.svelte";
import type { Population, Profile } from "./types";

export class MenuModel {
    tab = $state(0);
    external = $state(false);

    constructor(
        private readonly app: AppState,
        private readonly game: GameState,
        private readonly source: GameSource,
    ) {}

    get index(): number {
        return profileIndex(this.app.profiles, this.tab);
    }

    get profile(): Profile {
        return this.app.profiles.entries[this.index];
    }

    get fields(): TabFields {
        return TAB_FIELDS[this.tab];
    }

    get last(): number {
        return this.app.profiles.entries.length - 1;
    }

    get pop(): Population | undefined {
        return livePopulation(this.game.population);
    }

    go(i: number): void {
        const next = Math.max(0, Math.min(this.last, i));
        if (next === this.index) return;
        if (this.app.profiles.linked) this.tab = 0;
        this.app.selectProfile(this.app.profiles.linked ? 0 : this.tab, next);
    }

    toggleLink(): void {
        this.app.setLinked(!this.app.profiles.linked);
        if (this.app.profiles.linked) this.tab = 0;
    }

    openProfiles(): void {
        this.app.pickerTab = this.tab;
        this.app.open("profiles");
    }

    play(): void {
        this.source.play(this.tab);
        this.app.menuOpen = false;
    }

    spectate(): void {
        this.source.spectate();
        this.app.menuOpen = false;
    }
}
