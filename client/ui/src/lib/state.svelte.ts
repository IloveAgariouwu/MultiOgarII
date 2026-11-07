import type { Attachment } from "svelte/attachments";
import { ChatUi } from "./chat.svelte";
import { HudEditor } from "./hud-layout.svelte";
import { ReplayState } from "./replay.svelte";
import { ReplayThumbs, VideoExport } from "./video-export.svelte";
import type { CoreApi } from "./core";
import type { TextKey, Texts } from "./i18n";
import type { Store } from "./store";
import { MacroRecorder } from "./macro-recorder.svelte";
import { MinimapFeed } from "./minimap-feed";
import { Notices } from "./notices.svelte";
import { QuickChat } from "./quick-chat.svelte";
import { saveSelectedServer } from "./servers";
import type { MacroDef } from "./game.svelte";
import type {
    CoreLimits,
    Groups,
    Hotkeys,
    HotkeyKind,
    ProfileField,
    ProfileState,
    ServerChoice,
    SettingValue,
    SettingsMap,
    ThemeBook,
    UiSchema,
} from "./types";

export type Overlay = "profiles" | "servers" | "settings" | "replays" | null;

interface ResetScope {
    keys: string[];
    hotkeys: boolean;
    colors: boolean;
    macros: boolean;
}

export class AppState {
    menuOpen = $state(true);
    overlay = $state<Overlay>(null);
    settings = $state<SettingsMap>({});
    hotkeys = $state<Hotkeys>({ control: {}, toggle: {}, hold: {}, mouse: {} });
    colors = $state<Record<string, number>>({});
    themes = $state<ThemeBook>({ names: [], selected: null });
    themeError = $state("");
    profileError = $state("");
    profiles = $state.raw<ProfileState>({ entries: [], main: 0, second: 0, linked: true });
    server: ServerChoice;
    pickerTab = $state(0);
    touchAim: { x: number; y: number } | null = null;
    pinchZoom = 1;
    readonly minimap = new MinimapFeed();
    private menuBeforeReplay = false;
    private layers: (() => void)[] = [];
    readonly notices = new Notices();
    readonly chat = new ChatUi();
    private readonly replayNotices = this.notices.feed(8000);
    readonly replay = new ReplayState(
        (on) => this.replayMode(on),
        () =>
            this.replayNotices.post({
                key: "replayStoreFull",
                name: this.t("chatSenderReplay"),
                text: this.t("replayStoreFull"),
                open: () => this.open("replays"),
            }),
        (recordableMs) =>
            this.replayNotices.post({
                key: "replayStoreLow",
                name: this.t("chatSenderReplay"),
                text: this.t("replayStoreLow").replace("%s", String(Math.ceil(recordableMs / 60000))),
                open: () => this.open("replays"),
            }),
    );
    readonly video = new VideoExport(this);
    readonly thumbs = new ReplayThumbs(this);
    readonly hud = new HudEditor(this);
    readonly recorder: MacroRecorder;
    readonly quickChat = new QuickChat((key) => this.lookup(key), this.notices);
    macros = $state<MacroDef[]>([]);
    settingsCategory = $state<string | null>(null);
    readonly limits: CoreLimits;
    readonly ui: UiSchema;
    readonly defaults: SettingsMap;
    readonly groups: Groups;
    private readonly rivals = new Map<string, readonly string[]>();

    constructor(
        readonly core: CoreApi,
        readonly store: Store,
        readonly texts: Texts,
        server: ServerChoice,
    ) {
        this.server = $state(server);
        this.limits = core.limits();
        this.recorder = new MacroRecorder(this.limits.macroMaxDelayMs);
        this.ui = core.uiSchema();
        this.defaults = core.defaults();
        this.groups = core.groups();
        for (const cat of this.ui.categories)
            for (const item of cat.items)
                if (typeof item !== "string" && item.kind === "group" && item.exclusive)
                    for (const key of item.exclusive) this.rivals.set(key, item.exclusive);
        this.settings = core.settings();
        this.hotkeys = core.hotkeys();
        this.macros = core.macros();
        this.colors = core.colors();
        this.themes = core.themes();
        this.profiles = core.profiles();
    }

    t = (key: TextKey): string => this.texts[key];
    lookup = (key: string): string => this.texts[key] ?? key;

    limitText(key: string): string {
        return Object.entries(this.limits).reduce(
            (text, [name, value]) => text.replaceAll(`{${name}}`, String(value)),
            this.lookup(key),
        );
    }

    layer(close: () => void): Attachment<HTMLElement> {
        return () => {
            this.layers.push(close);
            return () => {
                this.layers = this.layers.filter((c) => c !== close);
            };
        };
    }

    dismissTop(): boolean {
        const close = this.layers.at(-1);
        close?.();
        return close !== undefined;
    }

    flag(key: string): boolean {
        return this.settings[key] === true;
    }

    num(key: string): number {
        return Number(this.settings[key]);
    }

    open(overlay: Overlay): void {
        this.overlay = overlay;
    }

    setSetting(key: string, value: SettingValue): void {
        if (value === true)
            for (const other of this.rivals.get(key) ?? [])
                if (other !== key && this.flag(other)) this.setSetting(other, false);
        const changed = this.core.setSetting(key, value);
        if (!changed.length) return;
        const fresh = this.core.settings();
        for (const k of changed) this.settings[k] = fresh[k];
        this.store.saveSettingsSoon();
    }

    setHotkey(kind: HotkeyKind, key: string, code: number): boolean {
        this.core.setHotkey(kind, key, code);
        this.hotkeys = this.core.hotkeys();
        this.store.saveSettings();
        this.macros = this.core.macros();
        this.store.saveMacros();
        return true;
    }

    saveMacro(index: number, macro: MacroDef): string {
        const error = this.core.setMacro(index, macro);
        if (error) return this.limitText(error);
        this.macros = this.core.macros();
        this.store.saveMacros();
        this.hotkeys = this.core.hotkeys();
        this.store.saveSettings();
        return "";
    }

    finishRecording(): void {
        this.recorder.stop();
        this.settingsCategory = "hotkeys";
        this.open("settings");
    }

    removeMacro(index: number): void {
        this.core.removeMacro(index);
        this.macros = this.core.macros();
        this.store.saveMacros();
    }

    moveMacro(from: number, to: number): void {
        const error = this.core.moveMacro(from, to);
        if (error) {
            console.error(`move macro ${from} -> ${to}: ${error}`);
            return;
        }
        this.macros = this.core.macros();
        this.store.saveMacros();
    }

    reset(scope: ResetScope): void {
        if (scope.keys.length) this.core.resetKeys(scope.keys);
        if (scope.hotkeys) this.core.resetHotkeys();
        if (scope.colors) {
            this.core.resetColors();
            this.pullThemes();
        }
        if (scope.macros) {
            this.core.clearMacros();
            this.macros = this.core.macros();
            this.store.saveMacros();
        }
        this.settings = this.core.settings();
        this.hotkeys = this.core.hotkeys();
        this.store.saveSettings();
    }

    setColor(key: string, argb: number): void {
        this.themeResult(this.core.setColor(key, argb, this.t("themeMine")));
    }

    saveThemeAs(name: string): boolean {
        return this.themeResult(this.core.saveThemeAs(name));
    }

    selectTheme(name: string | null): void {
        this.themeResult(this.core.selectTheme(name));
    }

    renameTheme(name: string): boolean {
        return this.themeResult(this.core.renameTheme(name));
    }

    deleteTheme(): void {
        this.themeResult(this.core.deleteTheme());
    }

    importTheme(json: string): void {
        const result = this.core.importTheme(json);
        for (const issue of result.issues) console.error(`imported theme: ${issue}`);
        this.themeResult(result.error);
    }

    private themeResult(error: string): boolean {
        this.themeError = error && this.limitText(error);
        if (!error) this.pullThemes();
        return !error;
    }

    private pullThemes(): void {
        this.colors = this.core.colors();
        this.themes = this.core.themes();
        this.store.saveThemes();
    }

    private profilesSaved = (saved: boolean): void => {
        this.profileError = saved ? "" : this.t("profilesStorageFull");
        if (!saved) this.profiles = this.core.profiles();
    };

    private reshapeProfiles(): boolean {
        this.profiles = this.core.profiles();
        const saved = this.store.saveProfiles();
        this.profilesSaved(saved);
        return saved;
    }

    private pullSelection(): void {
        this.profiles = { ...this.profiles, ...this.core.profileSelection() };
        this.store.saveProfilesSoon(this.profilesSaved);
    }

    setProfileField(index: number, field: ProfileField, value: string): void {
        const clean = ["skinUrl", "skinUrl2", "tabSkinUrl", "tabSkinUrl2"].includes(field)
            ? value.replace(/[^\w.:/-]/g, "")
            : value;
        if (!this.core.setProfileField(index, field, clean)) return;
        const fresh = this.core.profile(index);
        this.profiles = { ...this.profiles, entries: this.profiles.entries.map((p, i) => (i === index ? fresh : p)) };
        this.store.saveProfilesSoon(this.profilesSaved);
    }

    selectProfile(tab: number, index: number): void {
        this.core.selectProfile(tab, index);
        this.pullSelection();
    }

    setLinked(linked: boolean): void {
        this.core.setLinked(linked);
        this.pullSelection();
    }

    addProfile(): boolean {
        this.core.addProfile();
        return this.reshapeProfiles();
    }

    moveProfile(from: number, to: number): void {
        this.core.moveProfile(from, to);
        this.reshapeProfiles();
    }

    removeProfile(index: number): void {
        this.core.removeProfile(index);
        this.reshapeProfiles();
    }

    setServer(choice: ServerChoice): void {
        if (this.server.id !== choice.id && this.settings.ResetReplayWhenChangeServer) this.replay.reset();
        this.server = choice;
        saveSelectedServer(choice);
    }

    private replayMode(on: boolean): void {
        if (on) {
            this.menuBeforeReplay = this.menuOpen;
            this.menuOpen = false;
        } else this.menuOpen = this.menuBeforeReplay;
    }
}
