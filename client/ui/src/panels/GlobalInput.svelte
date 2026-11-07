<script lang="ts">
    import { useApp, useGame, useSource } from "../lib/context";
    import { MOUSE_LEFT, MOUSE_RIGHT, NAV_BUTTONS, eventCode, mouseCode, typingTarget } from "../lib/keys";
    import { Controls } from "../lib/controls";

    const app = useApp();
    const game = useGame();
    const source = useSource();
    const r = app.replay;
    const held = new Set<string>();
    const controls = new Controls(app, game, source);

    function escape() {
        if (document.querySelector(":popover-open")) return;
        if (app.dismissTop()) return;
        app.menuOpen = !app.menuOpen;
    }

    function onkeydown(e: KeyboardEvent) {
        if (e.key === "Escape") return escape();
        if (typingTarget(e)) return;
        if (e.key === "Enter") {
            app.chat.open = !app.chat.open;
            return;
        }
        if (app.overlay) return;
        if (!app.menuOpen) {
            if (e.key === "Tab") e.preventDefault();
            if (document.activeElement instanceof HTMLElement && document.activeElement !== document.body)
                document.activeElement.blur();
        }
        const code = eventCode(e);
        const key = e.code;
        for (const [k, v] of Object.entries(app.hotkeys.toggle))
            if (v === code && !e.repeat) app.setSetting(k, !app.settings[k]);
        for (const [k, v] of Object.entries(app.hotkeys.hold))
            if (v === code && !held.has(`${key}:${k}`)) {
                held.add(`${key}:${k}`);
                app.setSetting(k, !app.settings[k]);
            }
        for (const [k, v] of Object.entries(app.hotkeys.control)) {
            if (v !== code || (e.repeat && !k.startsWith("hkReplaySeek"))) continue;
            e.preventDefault();
            controls.press(k, key);
        }
        for (const m of app.macros) {
            if (m.trigger !== code || e.repeat) continue;
            e.preventDefault();
            source.runMacro(m);
        }
    }

    function mouse(e: MouseEvent, down: boolean) {
        if (!down) controls.release(`mouse${e.button}`);
        if (NAV_BUTTONS.has(e.button)) e.preventDefault();
        if (gMobile || app.menuOpen || app.overlay) return;
        if (!(e.target instanceof HTMLCanvasElement)) return;
        if (r.playback) {
            if (down && e.button === MOUSE_LEFT) r.togglePlay();
            if (down && e.button === MOUSE_RIGHT) r.stop();
            return;
        }
        if (!e.target.classList.contains("world")) return;
        source.mouse(e.button, down);
        if (!down) return;
        const code = mouseCode(e);
        for (const [action, bound] of Object.entries(app.hotkeys.mouse))
            if (bound === code) controls.press(action, `mouse${e.button}`);
    }

    function onblur() {
        game.focused = false;
        for (const id of held) {
            const k = id.slice(id.indexOf(":") + 1);
            app.setSetting(k, !app.settings[k]);
        }
        held.clear();
    }

    function releaseKey(code: string) {
        controls.release(code);
        for (const id of [...held]) {
            if (!id.startsWith(`${code}:`)) continue;
            held.delete(id);
            const k = id.slice(code.length + 1);
            app.setSetting(k, !app.settings[k]);
        }
    }

    function onkeyup(e: KeyboardEvent) {
        if (!typingTarget(e)) releaseKey(e.code);
    }
</script>

<svelte:window
    {onkeydown}
    {onkeyup}
    onmousedown={(e) => mouse(e, true)}
    onmouseup={(e) => mouse(e, false)}
    onfocus={() => (game.focused = true)}
    {onblur}
    oncontextmenu={(e) => e.preventDefault()}
    onbeforeunload={(e) => game.playing && e.preventDefault()}
/>
