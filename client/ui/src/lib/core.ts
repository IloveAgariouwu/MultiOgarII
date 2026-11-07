import init, { Core, World } from "../pkg/ogar_web";
import wasmUrl from "../pkg/ogar_web_bg.wasm?url";
import { fromJson } from "./json";
import type { MacroDef } from "./game.svelte";
import type {
    CoreLimits,
    Groups,
    Hotkeys,
    HotkeyKind,
    MinimapLayout,
    Profile,
    ProfileChanges,
    ProfileField,
    ProfileSelection,
    ProfileState,
    SettingValue,
    SettingsMap,
    StoredProfileKeys,
    StoredSelection,
    ThemeBook,
    ThemeImport,
    UiSchema,
} from "./types";

export class CoreApi {
    private constructor(private core: Core) {}

    get wasm(): Core {
        return this.core;
    }

    static async load(): Promise<CoreApi> {
        await init({ module_or_path: wasmUrl });
        return new CoreApi(new Core());
    }

    loadStored(settings: string | null, profiles: StoredProfileKeys): void {
        if (settings) for (const issue of this.core.loadSettings(settings)) console.error(`stored settings: ${issue}`);
        this.loadProfiles(profiles);
    }

    loadProfiles(p: StoredProfileKeys): void {
        for (const issue of this.core.loadProfiles(p.entries, p.extra, p.selected, p.selectedExtra, p.selectedTab2))
            console.error(`stored profiles: ${issue}`);
    }

    limits(): CoreLimits {
        return fromJson(this.core.limits());
    }

    minimapLayout(): MinimapLayout {
        return fromJson(World.minimapLayout());
    }

    uiSchema(): UiSchema {
        return fromJson(this.core.uiSchema());
    }

    groups(): Groups {
        return fromJson(this.core.groups());
    }

    defaults(): SettingsMap {
        return fromJson(this.core.defaults());
    }

    settings(): SettingsMap {
        return fromJson(this.core.settings());
    }

    hotkeys(): Hotkeys {
        return fromJson(this.core.hotkeys());
    }

    setSetting(key: string, value: SettingValue): string[] {
        return fromJson(this.core.setSetting(key, JSON.stringify(value)));
    }

    needsReload(key: string): boolean {
        return this.core.needsReload(key);
    }

    setHotkey(kind: HotkeyKind, key: string, code: number): void {
        this.core.setHotkey(kind, key, code);
    }

    resetKeys(keys: string[]): void {
        this.core.resetKeys(keys);
    }

    resetHotkeys(): void {
        this.core.resetHotkeys();
    }

    resetColors(): void {
        this.core.resetColors();
    }

    clearMacros(): void {
        this.core.clearMacros();
    }

    storedSettings(): string {
        return this.core.storedSettings();
    }

    colors(): Record<string, number> {
        return fromJson(this.core.colors());
    }

    loadThemes(stored: string | null): void {
        for (const issue of this.core.loadThemes(stored)) console.error(`stored themes: ${issue}`);
    }

    themes(): ThemeBook {
        return fromJson(this.core.themes());
    }

    setColor(key: string, argb: number, forkName: string): string {
        return this.core.setColor(key, argb >>> 0, forkName);
    }

    saveThemeAs(name: string): string {
        return this.core.saveThemeAs(name);
    }

    selectTheme(name: string | null): string {
        return this.core.selectTheme(name);
    }

    renameTheme(name: string): string {
        return this.core.renameTheme(name);
    }

    deleteTheme(): string {
        return this.core.deleteTheme();
    }

    exportTheme(): string {
        return this.core.exportTheme();
    }

    importTheme(json: string): ThemeImport {
        return fromJson(this.core.importTheme(json));
    }

    storedThemes(): string {
        return this.core.storedThemes();
    }

    profiles(): ProfileState {
        return fromJson(this.core.profiles());
    }

    setProfileField(index: number, field: ProfileField, value: string): boolean {
        return this.core.setProfileField(index, field, value);
    }

    selectProfile(tab: number, index: number): void {
        this.core.selectProfile(tab, index);
    }

    setLinked(linked: boolean): void {
        this.core.setLinked(linked);
    }

    takeProfileChanges(): ProfileChanges {
        return fromJson(this.core.takeProfileChanges());
    }

    profile(index: number): Profile {
        return fromJson(this.core.profile(index));
    }

    profileSelection(): ProfileSelection {
        return fromJson(this.core.profileSelection());
    }

    storedProfileSelection(): StoredSelection {
        return fromJson(this.core.storedProfileSelection());
    }

    addProfile(): number {
        return this.core.addProfile();
    }

    moveProfile(from: number, to: number): void {
        this.core.moveProfile(from, to);
    }

    removeProfile(index: number): void {
        this.core.removeProfile(index);
    }

    loadMacros(stored: string | null): void {
        if (stored) for (const issue of this.core.loadMacros(stored)) console.error(`stored macros: ${issue}`);
    }

    macros(): MacroDef[] {
        return fromJson(this.core.macros());
    }

    macroActions(): string[] {
        return this.core.macroActions();
    }

    setMacro(index: number, macro: MacroDef): string {
        return this.core.setMacro(index, JSON.stringify(macro));
    }

    removeMacro(index: number): void {
        this.core.removeMacro(index);
    }

    moveMacro(from: number, to: number): string {
        return this.core.moveMacro(from, to);
    }

    storedMacros(): string {
        return this.core.storedMacros();
    }
}
