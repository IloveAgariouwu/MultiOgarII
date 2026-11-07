import type { CoreApi } from "./core";
import { PACKS_KEY, importPacks, type PackError } from "./i18n";
import type { StoredProfileKeys } from "./types";

export const KEYS = {
    settings: "ogarSettings",
    themes: "ogarThemes",
    entries: "ogarProfiles",
    selected: "ogarProfileIndex",
    extraEntries: "ogarProfilesExtra",
    selectedTab2: "ogarProfileIndexTab2",
    selectedExtra: "ogarProfileSelected",
    macros: "ogarMacros",
    language: "ogarLanguage",
    chatSize: "ogarChatSize",
} as const;

export class Store {
    private pending = 0;
    private profilesPending = 0;
    private profilesDone: ((saved: boolean) => void) | null = null;

    constructor(private core: CoreApi) {
        addEventListener("pagehide", () => this.flush());
    }

    flush(): void {
        this.flushSettings();
        this.flushProfiles();
    }

    load(): void {
        const get = (k: string) => localStorage.getItem(k);
        this.core.loadStored(get(KEYS.settings), this.readProfiles());
        if (this.core.storedProfileSelection().selectedExtra === null) localStorage.removeItem(KEYS.selectedExtra);
        this.core.loadThemes(get(KEYS.themes));
        this.core.loadMacros(get(KEYS.macros));
        this.saveSettings();
        if (get(KEYS.themes) === null) this.saveThemes();
    }

    saveSettingsSoon(): void {
        clearTimeout(this.pending);
        this.pending = window.setTimeout(() => this.flushSettings(), 300);
    }

    private flushSettings(): void {
        if (!this.pending) return;
        this.saveSettings();
    }

    saveSettings(): void {
        clearTimeout(this.pending);
        this.pending = 0;
        localStorage.setItem(KEYS.settings, this.core.storedSettings());
    }

    saveThemes(): void {
        localStorage.setItem(KEYS.themes, this.core.storedThemes());
    }

    saveMacros(): void {
        localStorage.setItem(KEYS.macros, this.core.storedMacros());
    }

    private readProfiles(): StoredProfileKeys {
        const get = (k: string) => localStorage.getItem(k);
        return {
            entries: get(KEYS.entries),
            extra: get(KEYS.extraEntries),
            selected: get(KEYS.selected),
            selectedExtra: get(KEYS.selectedExtra),
            selectedTab2: get(KEYS.selectedTab2),
        };
    }

    saveProfilesSoon(done: (saved: boolean) => void): void {
        clearTimeout(this.profilesPending);
        this.profilesDone = done;
        this.profilesPending = window.setTimeout(() => this.flushProfiles(), 300);
    }

    private flushProfiles(): void {
        if (!this.profilesPending) return;
        const saved = this.saveProfiles();
        this.profilesDone?.(saved);
    }

    saveProfiles(): boolean {
        clearTimeout(this.profilesPending);
        this.profilesPending = 0;
        const s = this.core.takeProfileChanges();
        const writes: [string, string | null | undefined][] = [
            [KEYS.entries, s.entries],
            [KEYS.extraEntries, s.extra],
            [KEYS.selected, s.selected],
            [KEYS.selectedExtra, s.selectedExtra],
            [KEYS.selectedTab2, s.selectedTab2],
        ];
        try {
            for (const [k, v] of writes) {
                if (v === null) localStorage.removeItem(k);
                else if (v !== undefined) localStorage.setItem(k, v);
            }
            return true;
        } catch (e) {
            if (!(e instanceof DOMException && e.name === "QuotaExceededError")) throw e;
            console.error("profiles not saved", e);
            this.core.loadProfiles(this.readProfiles());
            return false;
        }
    }
}

export function exportConfig(flags: string[]): string {
    const keys = [...flags, ...(flags.includes(KEYS.entries) ? [KEYS.extraEntries] : [])];
    return JSON.stringify(Object.fromEntries(keys.map((k) => [k, localStorage.getItem(k) ?? ""])));
}

function isJson(text: string): boolean {
    try {
        JSON.parse(text);
        return true;
    } catch (e) {
        console.error("config file is not JSON", e);
        return false;
    }
}

interface ConfigImport {
    sections: number;
    error: PackError | null;
}

export function importConfig(text: string, flags: string[]): ConfigImport {
    if (!isJson(text)) return { sections: 0, error: null };
    const data: unknown = JSON.parse(text);
    if (!data || typeof data !== "object") return { sections: 0, error: null };
    const accepted = [...flags, ...(flags.includes(KEYS.entries) ? [KEYS.extraEntries] : [])];
    const entries = Object.entries(data).filter(
        ([k, v]) => accepted.includes(k) && typeof v === "string" && v && isJson(v),
    );
    const packs = entries.find(([k]) => k === PACKS_KEY);
    const error = packs ? importPacks(String(packs[1])) : null;
    if (error) return { sections: 0, error };
    for (const [k, v] of entries) if (k !== PACKS_KEY) localStorage.setItem(k, String(v));
    const keys = entries.map(([k]) => k);
    if (keys.includes(KEYS.entries) && KEYS.extraEntries in data && !keys.includes(KEYS.extraEntries))
        localStorage.removeItem(KEYS.extraEntries);
    return { sections: entries.length, error: null };
}
