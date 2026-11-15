import english from "../locales/en.json";

export type Texts = Record<string, string>;
export type TextKey = keyof typeof english;

interface LocaleRule {
    code: string;
    matches: (lang: string, region: string) => boolean;
}

export interface Language {
    code: string;
    name: string;
    imported: boolean;
}

export interface LanguagePack {
    code: string;
    name: string;
    texts: Texts;
}

export interface PackError {
    key: string;
    args: string[];
}

export type PackRead =
    { ok: true; pack: LanguagePack; warnings: PackError[]; translated: number } | { ok: false; error: PackError };

const FALLBACK_LOCALE = "en";
const ENGLISH: Texts = english;
export const PACKS_KEY = "ogarLanguagePacks";
const PACK_FORMAT = "ogar-language";
const PACK_VERSION = 1;
const KB = 1024;
const FILE_MAX_KB = 256;
const STORE_MAX_KB = 1024;
const PACKS_MAX = 8;
const NAME_MAX = 32;
const TEXT_MAX = 1000;
const LISTED_MAX = 5;
const LISTED_KEY_MAX = 40;
const CODE = /^[a-z]{2,3}(-[A-Za-z0-9]{2,8}){0,3}$/;
const PLACEHOLDER = /%[sv]|\{[A-Za-z]+\}/g;

const LOCALES: LocaleRule[] = [
    { code: "ja", matches: (lang) => lang === "ja" },
    {
        code: "zh-CN",
        matches: (lang, region) => (lang === "zh" && (region === "CN" || region === "SG")) || lang === "ms",
    },
    { code: "zh-TW", matches: (lang) => lang === "zh" },
];

const BUILTIN_LANGUAGES = [
    { code: "ja", name: "日本語" },
    { code: "zh-TW", name: "繁體中文" },
    { code: "zh-CN", name: "简体中文" },
    { code: "en", name: "English" },
];

const packs = import.meta.glob<Texts>(["../locales/*.json", "!../locales/en.json"], { import: "default" });

function subtags(code: string): string[] {
    return code.toLowerCase().split("-");
}

function resolveLocale(language: string, imported: LanguagePack[]): string {
    const [lang, regionRaw = ""] = language.split("-");
    const region = regionRaw.toUpperCase();
    const asked = subtags(language);
    const [pack] = imported
        .map((p) => p.code)
        .filter((code) => subtags(code).every((tag, i) => asked[i] === tag))
        .sort((a, b) => subtags(b).length - subtags(a).length);
    const builtin = LOCALES.find((l) => l.matches(lang.toLowerCase(), region))?.code;
    if (pack && subtags(pack).length > 1) return pack;
    return builtin ?? pack ?? FALLBACK_LOCALE;
}

export function languages(imported: LanguagePack[]): Language[] {
    const builtin = (code: string) => BUILTIN_LANGUAGES.some((l) => l.code === code);
    return [
        ...BUILTIN_LANGUAGES.map((l) => {
            const pack = imported.find((p) => p.code === l.code);
            return { code: l.code, name: pack?.name ?? l.name, imported: pack !== undefined };
        }),
        ...imported.filter((p) => !builtin(p.code)).map((p) => ({ code: p.code, name: p.name, imported: true })),
    ];
}

export function chosenLocale(saved: string | null, imported: LanguagePack[]): string {
    return languages(imported).find((l) => l.code === saved)?.code ?? resolveLocale(navigator.language, imported);
}

async function ownTexts(code: string, imported: LanguagePack[]): Promise<Texts> {
    const file = packs[`../locales/${code}.json`];
    const base = code === FALLBACK_LOCALE ? ENGLISH : file ? await file() : {};
    return { ...base, ...imported.find((p) => p.code === code)?.texts };
}

export async function loadTexts(code: string, imported: LanguagePack[]): Promise<Texts> {
    return { ...ENGLISH, ...(await ownTexts(code, imported)) };
}

export async function translationTemplate(code: string, imported: LanguagePack[]): Promise<string> {
    const own = await ownTexts(code, imported);
    return JSON.stringify(
        {
            format: PACK_FORMAT,
            version: PACK_VERSION,
            code,
            name: languages(imported).find((l) => l.code === code)?.name ?? code,
            texts: Object.fromEntries(Object.keys(ENGLISH).map((key) => [key, own[key] ?? ""])),
        },
        null,
        2,
    );
}

function isRecord(value: unknown): value is Record<string, unknown> {
    return typeof value === "object" && value !== null && !Array.isArray(value);
}

function placeholders(text: string): string {
    return (text.match(PLACEHOLDER) ?? []).sort().join(" ");
}

function listedKeys(keys: string[]): string {
    const shown = keys
        .slice(0, LISTED_MAX)
        .map((k) => ([...k].length > LISTED_KEY_MAX ? `${[...k].slice(0, LISTED_KEY_MAX).join("")}…` : k))
        .join(", ");
    return keys.length > LISTED_MAX ? `${shown} … (+${keys.length - LISTED_MAX})` : shown;
}

function packError(key: string, ...args: string[]): PackRead {
    return { ok: false, error: { key, args } };
}

function readPack(data: unknown): PackRead {
    if (!isRecord(data) || data.format !== PACK_FORMAT || data.version !== PACK_VERSION || !isRecord(data.texts))
        return packError("langErrFormat");
    if (typeof data.code !== "string" || !CODE.test(data.code)) return packError("langErrCode");
    const name = typeof data.name === "string" ? data.name.trim() : "";
    if (!name || [...name].length > NAME_MAX) return packError("langErrName", String(NAME_MAX));
    const texts: Texts = {};
    const unknown: string[] = [];
    const nonText: string[] = [];
    const tooLong: string[] = [];
    const mismatched: string[] = [];
    for (const [key, value] of Object.entries(data.texts)) {
        if (typeof value !== "string") nonText.push(key);
        else if (!Object.hasOwn(ENGLISH, key)) unknown.push(key);
        else if (!value) continue;
        else if ([...value].length > TEXT_MAX) tooLong.push(key);
        else if (placeholders(value) !== placeholders(ENGLISH[key]))
            mismatched.push(`${key} [${placeholders(ENGLISH[key])}]`);
        else texts[key] = value;
    }
    if (nonText.length) return packError("langErrValue", listedKeys(nonText));
    if (tooLong.length) return packError("langErrLength", String(TEXT_MAX), listedKeys(tooLong));
    if (mismatched.length) return packError("langErrPlaceholders", listedKeys(mismatched));
    const translated = Object.keys(texts).length;
    if (!translated) return packError("langErrEmpty");
    const warnings = unknown.length ? [{ key: "langWarnUnknown", args: [listedKeys(unknown)] }] : [];
    return { ok: true, pack: { code: data.code, name, texts }, warnings, translated };
}

export function readPackFile(text: string): PackRead {
    if (text.length > FILE_MAX_KB * KB) return packError("langErrFileSize", `${FILE_MAX_KB} KB`);
    let data: unknown;
    try {
        data = JSON.parse(text);
    } catch (e) {
        console.error("language file is not JSON", e);
        return packError("langErrJson");
    }
    return readPack(data);
}

export function englishCount(): number {
    return Object.keys(ENGLISH).length;
}

export function storedPacks(): LanguagePack[] {
    const stored = localStorage.getItem(PACKS_KEY);
    if (stored === null) return [];
    let list: unknown;
    try {
        list = JSON.parse(stored);
    } catch (e) {
        console.error(`${PACKS_KEY} is not JSON`, e);
        return [];
    }
    if (!Array.isArray(list)) {
        console.error(`${PACKS_KEY} is not a list`);
        return [];
    }
    return list.flatMap((item: unknown) => {
        const read = readPack(item);
        if (read.ok) return [read.pack];
        console.error(`stored language pack rejected: ${read.error.key} ${read.error.args.join(" ")}`);
        return [];
    });
}

export function packMessage(error: PackError, t: (key: string) => string): string {
    return error.args.reduce((text, arg) => text.replace("%s", () => arg), t(error.key));
}

export function importPacks(stored: string): PackError | null {
    const list: unknown = JSON.parse(stored);
    if (!Array.isArray(list)) return { key: "langErrFormat", args: [] };
    const imported: LanguagePack[] = [];
    for (const item of list) {
        const read = readPack(item);
        if (!read.ok) return read.error;
        imported.push(read.pack);
    }
    return savePacks(imported);
}

export function savePacks(list: LanguagePack[]): PackError | null {
    if (list.length > PACKS_MAX) return { key: "langErrTooMany", args: [String(PACKS_MAX)] };
    const stored = JSON.stringify(
        list.map((p) => ({ format: PACK_FORMAT, version: PACK_VERSION, code: p.code, name: p.name, texts: p.texts })),
    );
    const full = { key: "langErrStorage", args: [`${STORE_MAX_KB} KB`] };
    if (stored.length > STORE_MAX_KB * KB) return full;
    try {
        if (list.length) localStorage.setItem(PACKS_KEY, stored);
        else localStorage.removeItem(PACKS_KEY);
        return null;
    } catch (e) {
        console.error("language packs do not fit in storage", e);
        return full;
    }
}
