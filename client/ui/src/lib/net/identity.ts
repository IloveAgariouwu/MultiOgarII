import { TAB_FIELDS, profileIndex } from "../profile-fields";
import type { AppState } from "../state.svelte";
import type { TabIndex } from "./tabs";

export function tabIdentity(app: AppState, tab: TabIndex): string {
    const state = app.profiles;
    const entry = state.entries[profileIndex(state, tab)];
    const f = TAB_FIELDS[tab];
    return JSON.stringify({
        name: entry[f.name],
        skin: entry[f.skin[0]],
        skin2: entry[f.skin[1]],
        externalSkin: entry[f.self[0]],
        externalSkin2: entry[f.self[1]],
    });
}
