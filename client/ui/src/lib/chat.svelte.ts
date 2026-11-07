import type { ChatMessage } from "./game.svelte";
import { KEYS } from "./store";

export interface ChatSize {
    width: number;
    log: number;
}

const DEFAULT_SIZE: ChatSize = { width: 17, log: 20 };
const MIN_SIZE: ChatSize = { width: 16, log: 4 };

function loadSize(): ChatSize {
    try {
        const saved = localStorage.getItem(KEYS.chatSize);
        return saved === null ? DEFAULT_SIZE : JSON.parse(saved);
    } catch (e) {
        console.error(`${KEYS.chatSize} is not valid JSON`, e);
        return DEFAULT_SIZE;
    }
}

export class ChatUi {
    size = $state<ChatSize>(loadSize());
    open = $state(false);

    startResize(e: PointerEvent): void {
        e.preventDefault();
        e.stopPropagation();
        const rem = parseFloat(getComputedStyle(document.documentElement).fontSize);
        const start = this.size,
            x0 = e.clientX,
            y0 = e.clientY;
        const move = (ev: PointerEvent) => {
            this.size = {
                width: Math.max(MIN_SIZE.width, start.width + (ev.clientX - x0) / rem),
                log: Math.max(MIN_SIZE.log, start.log + (ev.clientY - y0) / rem),
            };
        };
        const up = () => {
            removeEventListener("pointermove", move);
            removeEventListener("pointerup", up);
            removeEventListener("pointercancel", up);
            localStorage.setItem(KEYS.chatSize, JSON.stringify(this.size));
        };
        addEventListener("pointermove", move);
        addEventListener("pointerup", up);
        addEventListener("pointercancel", up);
    }

    mentionsMe(m: ChatMessage, selves: string[]): boolean {
        if (m.system || selves.includes(m.name)) return false;
        const text = m.text.toLowerCase();
        return selves.some((name) => text.includes(`@${name.toLowerCase()}`));
    }
}
