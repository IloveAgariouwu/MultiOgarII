<script lang="ts">
    import Icon from "../components/Icon.svelte";
    import { useApp, useGame, useHudPlacement, useReplay } from "../lib/context";
    import { argbCss } from "../lib/format";
    import { saveFile } from "../lib/replay-store";
    import { timeText } from "../lib/replay.svelte";

    const app = useApp();
    const game = useGame();
    const placement = useHudPlacement();
    const t = app.t;
    const r = useReplay();
    const video = app.video;
    let gauge: HTMLDivElement;
    let fileInput: HTMLInputElement;
    let style = $derived(
        `--bar:${argbCss(app.colors.clReplayBar)};--sym:${argbCss(app.colors.clUiSymbols)};--active:${argbCss(app.colors.clUiButtonActive)};--scrim:${argbCss(app.colors.clLeaderboardBack)}`,
    );

    async function exportReel() {
        const bytes = await r.exportReel();
        if (!bytes) return;
        saveFile(new Blob([new Uint8Array(bytes)], { type: "application/octet-stream" }), "replay", "ogarr");
    }

    async function exportVideo() {
        if (!r.reel) return;
        const blob = await video.record(r.reel);
        if (blob) saveFile(blob, "video", "mp4");
    }

    function clearAll() {
        if (confirm(t("replayResetConfirm").replace("%s", String(r.reels.length)))) r.reset();
    }

    function confirmKeepClip() {
        const clip = r.reel?.clip;
        if (!clip) return;
        const range = `${timeText(clip.from)} – ${timeText(clip.to)}`;
        if (confirm(t("replayKeepClipConfirm").replace("%s", () => range))) r.keepClip();
    }

    async function importReel(e: Event & { currentTarget: HTMLInputElement }) {
        const file = e.currentTarget.files?.[0];
        e.currentTarget.value = "";
        if (!file) return;
        const error = r.importFile(new Uint8Array(await file.arrayBuffer()));
        if (!error) return;
        console.error(`replay import failed: ${error}`);
        game.system(t("replayImportFailed"), t("chatSenderReplay"));
    }

    function dragGauge(e: PointerEvent) {
        gauge.setPointerCapture(e.pointerId);
        const move = (ev: PointerEvent) => {
            const rect = gauge.getBoundingClientRect();
            r.seekTo((ev.clientX - rect.left) / rect.width);
        };
        move(e);
        gauge.onpointermove = move;
        gauge.onpointerup = () => (gauge.onpointermove = null);
    }
</script>

<div class="vars" {style}>
    <div
        class="replay scrim"
        class:disabled={game.bench}
        role="toolbar"
        tabindex="-1"
        onmousedown={(e) => e.target !== gauge && e.preventDefault()}
        {@attach placement("replay")}
    >
        <div class="grp" class:busy={video.active}>
            <button
                class="b"
                class:flash={r.captured}
                title={t("replayCapture")}
                aria-label={t("replayCapture")}
                onclick={() => r.capture()}><Icon name="bolt" /></button
            >
            <button
                class="b rec"
                class:on={r.recording}
                title={t("replayRecord")}
                aria-label={t("replayRecord")}
                aria-pressed={r.recording}
                onclick={() => r.toggleRecording()}><Icon name="record" /></button
            >
            <button class="b" title={t("replayStop")} aria-label={t("replayStop")} onclick={() => r.stop()}
                ><Icon name="stop" /></button
            >
            <button
                class="b"
                class:on={r.playing}
                title={t("replayPlay")}
                aria-label={t("replayPlay")}
                onclick={() => r.togglePlay()}><Icon name={r.playing ? "pause" : "play"} /></button
            >
        </div>

        <div class="grp" class:busy={video.active}>
            <button class="b" aria-label={t("replayBack")} title={t("replayBack")} onclick={() => r.step(-1)}
                ><Icon name="stepBack" /></button
            >
            <button class="b" aria-label={t("replayForward")} title={t("replayForward")} onclick={() => r.step(1)}
                ><Icon name="stepForward" /></button
            >
            <span class="speed">
                <button class="t" aria-label={t("replaySlower")} onclick={() => r.changeSpeed(-1)}>−</button>
                <span>×{r.speed.toFixed(2)}</span>
                <button class="t" aria-label={t("replayFaster")} onclick={() => r.changeSpeed(1)}>+</button>
            </span>
            <button class="b" title={t("replayMarkIn")} aria-label={t("replayMarkIn")} onclick={() => r.markIn()}
                ><Icon name="markIn" /></button
            >
            <button class="b" title={t("replayMarkOut")} aria-label={t("replayMarkOut")} onclick={() => r.markOut()}
                ><Icon name="markOut" /></button
            >
            <button
                class="b"
                class:on={r.cont}
                title={t("replayContinue")}
                aria-label={t("replayContinue")}
                aria-pressed={r.cont}
                onclick={() => (r.cont = !r.cont)}><Icon name="loop" /></button
            >
        </div>

        <div class="track" class:busy={video.active}>
            <span class="time">{timeText(r.position)}</span>
            <div
                class="gauge"
                bind:this={gauge}
                role="slider"
                tabindex="0"
                aria-label={t("replaySeek")}
                aria-valuenow={Math.round(r.progress * 100)}
                onpointerdown={dragGauge}
                onkeydown={(e) =>
                    (e.key === "ArrowLeft" || e.key === "ArrowRight") && r.step(e.key === "ArrowLeft" ? -1 : 1)}
            >
                <span class="rail"
                    >{#if r.reel?.clip}<span
                            class="clip"
                            style:left="{(r.reel.clip.from / r.reel.duration) * 100}%"
                            style:width="{((r.reel.clip.to - r.reel.clip.from) / r.reel.duration) * 100}%"
                        ></span>{/if}<span class="fill" style:width="{r.progress * 100}%"></span></span
                >
            </div>
            <span class="time dim">{timeText(r.reel?.duration ?? 0)}</span>
            {#if r.captured}<span class="captured">{t("replayCaptured")}</span>{/if}
        </div>

        <div class="grp reels">
            <span class="grp" class:busy={video.active}>
                <button
                    class="b"
                    aria-label={t("replayPrevReel")}
                    title={t("replayPrevReel")}
                    onclick={() => r.selectReel(-1)}><Icon name="prev" /></button
                >
                <span class="count">{r.reels.length ? r.current + 1 : 0}<i>/</i>{r.reels.length}</span>
                <button
                    class="b"
                    aria-label={t("replayNextReel")}
                    title={t("replayNextReel")}
                    onclick={() => r.selectReel(1)}><Icon name="next" /></button
                >
                <button
                    class="b"
                    title={t("replayDeleteReel")}
                    aria-label={t("replayDeleteReel")}
                    onclick={() => r.deleteReel()}><Icon name="trash" /></button
                >
                <button
                    class="b"
                    title={t("replayGallery")}
                    aria-label={t("replayGallery")}
                    onclick={() => app.open("replays")}><Icon name="grid" /></button
                >
                <button class="b" title={t("replayReset")} aria-label={t("replayReset")} onclick={clearAll}
                    ><Icon name="sync" /></button
                >
            </span>
            {#if video.progress !== null}
                <button
                    class="exporting"
                    title={t("replayVideoStop")}
                    onclick={() => video.cancel()}
                    {@attach video.show}>{Math.round(video.progress * 100)}%</button
                >
            {:else}
                <button
                    class="b anchor"
                    title={t("replayMore")}
                    aria-label={t("replayMore")}
                    popovertarget="replay-more"><Icon name="menu" /></button
                >
            {/if}
            <div id="replay-more" class="more" popover="auto">
                <button disabled={!r.reel} popovertarget="replay-more" popovertargetaction="hide" onclick={exportVideo}
                    ><Icon name="film" />{video.label}</button
                >
                <button disabled={!r.reel} popovertarget="replay-more" popovertargetaction="hide" onclick={exportReel}
                    ><Icon name="download" />{t("replayExport")}</button
                >
                <button
                    disabled={!r.reel?.clip}
                    popovertarget="replay-more"
                    popovertargetaction="hide"
                    onclick={() => r.clearClip()}><Icon name="markOut" />{t("replayClearClip")}</button
                >
                <button
                    disabled={!r.reel?.clip || video.active}
                    popovertarget="replay-more"
                    popovertargetaction="hide"
                    onclick={confirmKeepClip}><Icon name="markIn" />{t("replayKeepClip")}</button
                >
                <button popovertarget="replay-more" popovertargetaction="hide" onclick={() => fileInput.click()}
                    ><Icon name="upload" />{t("replayImport")}</button
                >
            </div>
            <input type="file" accept=".ogarr" hidden bind:this={fileInput} onchange={importReel} />
        </div>
    </div>
</div>

<style>
    .vars {
        display: contents;
    }

    .replay {
        position: fixed;
        left: 50%;
        top: 0;
        transform: translateX(-50%);
        display: flex;
        align-items: center;
        gap: 1.1rem;
        padding: 0.35rem 0.9rem;
        color: var(--sym);
        text-shadow: var(--shadow);
        z-index: var(--z-hud);
    }

    .busy {
        opacity: 0.35;
        pointer-events: none;
    }

    .replay.disabled {
        opacity: 0.4;
        pointer-events: none;
    }

    .grp {
        display: flex;
        align-items: center;
        gap: 0.15rem;
    }

    .reels {
        padding-left: 0.9rem;
        border-left: 1px solid var(--rule);
    }

    .b {
        display: grid;
        place-items: center;
        width: var(--hit);
        height: var(--hit);
        padding: 0;
        border: 0;
        background: none;
        color: var(--sym);
        opacity: 0.82;
    }

    .b:hover {
        opacity: 1;
    }

    .b :global(svg) {
        width: var(--icon);
        height: var(--icon);
        fill: none;
        stroke: currentColor;
        stroke-width: 1.8;
        stroke-linecap: round;
        stroke-linejoin: round;
    }

    .b.on {
        color: var(--active);
        opacity: 1;
    }

    .rec.on :global(svg) {
        fill: currentColor;
    }

    .b.flash {
        color: var(--accent);
        opacity: 1;
    }

    .track {
        position: relative;
        display: flex;
        align-items: center;
        gap: 0.6rem;
        width: 15rem;
    }

    .time,
    .speed,
    .count {
        font-size: var(--fs-sm);
        font-variant-numeric: tabular-nums;
        white-space: nowrap;
    }

    .time {
        min-width: 3.4rem;
        text-align: right;
    }

    .time.dim {
        text-align: left;
    }

    .dim {
        opacity: 0.6;
    }

    .gauge {
        flex: 1;
        display: flex;
        align-items: center;
        height: 1.4rem;
        cursor: pointer;
    }

    .rail {
        position: relative;
        width: 100%;
        height: 3px;
        background: color-mix(in srgb, var(--fg) 16%, transparent);
    }

    .fill {
        position: absolute;
        left: 0;
        top: 0;
        bottom: 0;
        background: var(--bar);
    }

    .clip {
        position: absolute;
        top: -3px;
        bottom: -3px;
        background: color-mix(in srgb, var(--active) 45%, transparent);
    }

    .fill::after {
        content: "";
        position: absolute;
        right: -1px;
        top: -4px;
        width: 2px;
        height: 11px;
        background: var(--sym);
    }

    .gauge:hover .rail {
        height: 5px;
    }

    .speed {
        display: flex;
        align-items: center;
        gap: 0.1rem;
        margin: 0 0.3rem;
    }

    .t {
        width: 1.3rem;
        padding: 0;
        border: 0;
        background: none;
        color: var(--sym);
        opacity: 0.7;
        font-size: var(--fs-md);
    }

    .t:hover {
        opacity: 1;
    }

    .count {
        min-width: 2.6rem;
        text-align: center;
    }

    .count i {
        margin: 0 0.15rem;
        font-style: normal;
        opacity: 0.5;
    }

    .captured {
        position: absolute;
        left: 50%;
        top: 100%;
        transform: translateX(-50%);
        margin-top: 0.5rem;
        color: var(--accent);
        font-size: var(--fs-sm);
    }
    .anchor {
        anchor-name: --replay-more;
    }

    .more {
        position: fixed;
        position-anchor: --replay-more;
        inset: auto;
        top: anchor(bottom);
        right: anchor(right);
        margin: 0.3rem 0 0;
        padding: 0.3rem 0;
        border: 0;
        background: var(--bg);
        color: var(--fg);
        box-shadow: 0 0.8rem 2rem rgba(0, 0, 0, 0.5);
    }

    .more button {
        display: flex;
        align-items: center;
        gap: 0.6rem;
        width: 100%;
        padding: 0.4rem 0.9rem;
        border: 0;
        background: none;
        color: inherit;
        text-align: left;
        white-space: nowrap;
    }

    .more button:hover:not(:disabled) {
        background: color-mix(in srgb, var(--fg) 8%, transparent);
    }

    .more button:disabled {
        opacity: 0.4;
    }

    .more :global(svg) {
        width: var(--icon-sm);
        height: var(--icon-sm);
        fill: none;
        stroke: currentColor;
        stroke-width: 1.8;
    }

    .exporting {
        min-width: 3.4rem;
        padding: 0.1rem 0.4rem;
        border: 1px solid var(--rule-strong);
        background: none;
        color: var(--fg);
        font-size: var(--fs-sm);
        font-variant-numeric: tabular-nums;
    }
</style>
