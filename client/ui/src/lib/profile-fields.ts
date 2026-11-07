import type { Profile, ProfileField, ProfileState } from "./types";

export interface TabFields {
    name: ProfileField;
    skin: [ProfileField, ProfileField];
    self: [ProfileField, ProfileField];
}

export const TAB_FIELDS: [TabFields, TabFields] = [
    { name: "name", skin: ["skinUrl", "skinUrl2"], self: ["selfSkinUrl", "selfSkinUrl2"] },
    { name: "tabName", skin: ["tabSkinUrl", "tabSkinUrl2"], self: ["selfTabSkinUrl", "selfTabSkinUrl2"] },
];

export function profileIndex(state: ProfileState, tab: number): number {
    return tab === 1 ? state.second : state.main;
}

export function unitSkins(p: Profile, tab: number, external: boolean): [string, string] {
    const f = TAB_FIELDS[tab];
    const pick = (u: 0 | 1) => (external && p[f.self[u]]) || p[f.skin[u]] || p[f.skin[0]];
    return [pick(0), pick(1)];
}
