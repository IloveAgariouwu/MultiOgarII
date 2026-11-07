import type { NoticeFeed, Notices } from "./notices.svelte";

export const QUICK_CHAT_SLOTS = 10;
const SECTOR = "%sector%";
const CANON = [
    "Feed me!",
    "Split into me!",
    `Need backup at ${SECTOR}!`,
    `Enemy spotted at ${SECTOR}!`,
    "Need a teammate!",
    "Tank the virus!",
    "Tricksplit!",
    "We need to run!",
];
const SECTOR_SHAPE = "[A-Z][0-9]+";
const KNOWN_SECTOR = new RegExp(`^${SECTOR_SHAPE}$`);
const PATTERNS = CANON.map(
    (line) => new RegExp(`^${line.replace(/[.*+?^${}()|[\]\\]/g, "\\$&").replace(SECTOR, `(${SECTOR_SHAPE})`)}$`),
);

export function quickChatAction(slot: number): string {
    return `hkQuickChat${slot + 1}`;
}

export function quickChatSetting(slot: number): string {
    return `QuickChat${slot + 1}`;
}

export function localizeQuickChat(text: string, t: (key: string) => string): string | null {
    for (const [i, pattern] of PATTERNS.entries()) {
        const match = pattern.exec(text);
        if (match) return t(`qcDefault${i + 1}`).replace(SECTOR, match[1]);
    }
    return null;
}

export class QuickChat {
    private last = -Infinity;
    private readonly notices: NoticeFeed;

    constructor(
        private readonly t: (key: string) => string,
        notices: Notices,
    ) {
        this.notices = notices.feed(1200);
    }

    send(slot: number, custom: string, sector: string, say: (text: string) => void): void {
        const line = custom || CANON[slot];
        if (!line) return;
        const now = performance.now();
        const wait = this.last + 1000 - now;
        if (wait > 0) return this.show(this.t("qcCooldown").replace("%s", (wait / 1000).toFixed(1)));
        if (line.includes(SECTOR) && !KNOWN_SECTOR.test(sector)) return this.show(this.t("qcNoSector"));
        this.last = now;
        say([...line.replaceAll(SECTOR, sector)].slice(0, 80).join(""));
    }

    private show(text: string): void {
        this.notices.post({ key: "", name: "", text, open: null });
    }
}
