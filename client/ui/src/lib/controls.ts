import type { GameSource, GameState } from "./game.svelte";
import { QUICK_CHAT_SLOTS, quickChatAction, quickChatSetting } from "./quick-chat.svelte";
import type { AppState } from "./state.svelte";

export class Controls {
    private readonly pressed = new Set<string>();
    private readonly seeking = new Map<string, number>();
    private readonly ui: Record<string, (key: string) => void>;

    constructor(
        private readonly app: AppState,
        private readonly game: GameState,
        private readonly source: GameSource,
    ) {
        const r = app.replay;
        this.ui = {
            hkQuickReplayCapture: () => r.capture(),
            hkToggleReplayRecording: () => r.toggleRecording(),
            hkPlaybackReplay: () => (r.playback ? r.stop() : r.togglePlay()),
            hkReplayPlayPause: () => r.playback && r.togglePlay(),
            hkReplaySeekLeft: (key) => this.startSeek(key, -1),
            hkReplaySeekRight: (key) => this.startSeek(key, 1),
        };
        for (let slot = 0; slot < QUICK_CHAT_SLOTS; slot++) this.ui[quickChatAction(slot)] = () => this.quickChat(slot);
        for (const action of app.ui.uiActions)
            if (!(action in this.ui)) throw new Error(`UI-only hotkey ${action} has no handler`);
        for (const action in this.ui)
            if (!app.ui.uiActions.includes(action))
                throw new Error(`${action} is handled by the UI but not listed in uiActions`);
    }

    press(action: string, key: string): void {
        const ui = this.ui[action];
        if (ui) return ui(key);
        if (action === "hkStartNewGame") {
            this.source.play();
            this.app.menuOpen = false;
            return;
        }
        this.pressed.add(`${key}:${action}`);
        this.app.recorder.record(action, "down");
        this.source.act(action);
    }

    release(key: string): void {
        for (const id of [...this.pressed]) {
            if (!id.startsWith(`${key}:`)) continue;
            this.pressed.delete(id);
            const action = id.slice(key.length + 1);
            this.app.recorder.record(action, "up");
            this.source.release(action);
        }
        const timer = this.seeking.get(key);
        if (timer === undefined) return;
        clearInterval(timer);
        this.seeking.delete(key);
    }

    releaseAll(): void {
        for (const id of this.pressed) {
            const action = id.slice(id.indexOf(":") + 1);
            this.app.recorder.record(action, "up");
            this.source.release(action);
        }
        this.pressed.clear();
        for (const timer of this.seeking.values()) clearInterval(timer);
        this.seeking.clear();
    }

    private startSeek(key: string, dir: number): void {
        if (this.seeking.has(key)) return;
        this.app.replay.holdSeek(dir);
        this.seeking.set(
            key,
            window.setInterval(() => this.app.replay.holdSeek(dir), 100),
        );
    }

    private quickChat(slot: number): void {
        const game = this.game;
        const sector = game.activeTab === 1 ? game.self.pos2 : game.self.pos1;
        this.app.quickChat.send(slot, String(this.app.settings[quickChatSetting(slot)]), sector, (text) =>
            this.source.say(text),
        );
    }
}
