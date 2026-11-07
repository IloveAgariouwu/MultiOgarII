<script lang="ts">
    import { untrack } from "svelte";
    import Icon from "../components/Icon.svelte";
    import PickerPanel from "../components/PickerPanel.svelte";
    import { useApp } from "../lib/context";
    import { timeText, type ReplayEntry } from "../lib/replay.svelte";
    import ReplayStorage from "./ReplayStorage.svelte";
    import { saveFile } from "../lib/replay-store";
    import { bytes } from "../lib/format";

    const app = useApp();
    const t = app.t;
    const r = app.replay;
    const video = app.video;
    const thumbs = app.thumbs;

    $effect(() => {
        const missing = r.reels.filter((reel) => !reel.thumb);
        untrack(() => thumbs.render(missing));
    });

    function viewReel(reel: ReplayEntry) {
        r.open(reel.key);
        app.open(null);
    }

    async function playReel(reel: ReplayEntry) {
        const opened = r.open(reel.key);
        app.open(null);
        if (await opened) await r.togglePlay();
    }

    async function exportVideo(reel: ReplayEntry) {
        const blob = await video.record(reel);
        if (blob) saveFile(blob, "video", "mp4");
    }

    async function download(reel: ReplayEntry) {
        const data = await r.exportReel(reel);
        if (data) saveFile(new Blob([new Uint8Array(data)], { type: "application/octet-stream" }), "replay", "ogarr");
    }
</script>

<PickerPanel title={t("replayGallery")} closeLabel={t("pickClose")} onclose={() => app.open(null)}>
    {#snippet tools()}
        <ReplayStorage />
        {#if video.progress !== null}<button class="btn-line" onclick={() => video.cancel()} {@attach video.show}
                >{t("replayVideoStop")} {Math.round(video.progress * 100)}%</button
            >{/if}
    {/snippet}
    <div class="grid">
        {#each r.reels as reel, i (reel.key)}
            <div class="tile" class:current={r.playback && r.reel?.key === reel.key}>
                <span class="shot"
                    >{#if reel.thumb}<img src={reel.thumb.url} alt="" />{/if}</span
                >
                <button class="meta stretch" onclick={() => viewReel(reel)}>
                    <span class="k">{i + 1}</span>
                    <span class="dur">{timeText(reel.duration)}</span>
                    <span class="src">{t(reel.origin === "imported" ? "replayImported" : "replayLive")}</span>
                    {#if reel.size !== null}<span class="size">{bytes(reel.size)}</span>{/if}
                </button>
                <span class="acts">
                    <button title={t("replayPlay")} aria-label={t("replayPlay")} onclick={() => playReel(reel)}
                        ><Icon name="play" /></button
                    >
                    <button
                        title={video.label}
                        aria-label={video.label}
                        disabled={video.active}
                        onclick={() => exportVideo(reel)}><Icon name="film" /></button
                    >
                    <button title={t("replayExport")} aria-label={t("replayExport")} onclick={() => download(reel)}
                        ><Icon name="download" /></button
                    >
                    <button
                        title={t("replayDeleteReel")}
                        aria-label={t("replayDeleteReel")}
                        onclick={() => r.deleteReel(reel.key)}><Icon name="trash" /></button
                    >
                </span>
            </div>
        {:else}
            <div class="empty">{t("replayGalleryEmpty")}</div>
        {/each}
    </div>
</PickerPanel>

<style>
    .grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(16rem, 1fr));
        gap: 1rem;
    }

    .tile {
        position: relative;
        display: flex;
        flex-direction: column;
        gap: 0.4rem;
        padding: 0.5rem;
        cursor: pointer;
        box-shadow: inset 0 0 0 1px var(--rule);
        transition: box-shadow 0.15s;
    }

    .tile:hover,
    .tile.current {
        box-shadow: inset 0 0 0 1px var(--accent);
    }

    .shot {
        display: block;
        aspect-ratio: 16 / 9;
        background: var(--well);
    }

    img {
        display: block;
        width: 100%;
        height: 100%;
        object-fit: cover;
    }

    .meta {
        display: flex;
        align-items: baseline;
        gap: 0.6rem;
        font-variant-numeric: tabular-nums;
    }

    .k {
        color: var(--dim);
        font-size: var(--fs-xs);
    }

    .dur {
        font-weight: 600;
    }

    .src {
        margin-left: auto;
        color: var(--mute);
        font-size: var(--fs-xs);
    }

    .acts {
        display: flex;
        gap: 0.2rem;
    }

    .acts button {
        position: relative;
        display: grid;
        place-items: center;
        width: 1.9rem;
        height: 1.9rem;
        padding: 0;
        border: 0;
        background: none;
        color: var(--mute);
    }

    .acts button:hover:not(:disabled) {
        color: var(--fg);
    }

    .acts button:disabled {
        opacity: 0.35;
    }

    .acts :global(svg) {
        width: var(--icon-sm);
        height: var(--icon-sm);
        fill: none;
        stroke: currentColor;
        stroke-width: 1.8;
    }

    .empty {
        color: var(--dim);
    }

    .size {
        margin-left: auto;
        color: var(--dim);
    }
</style>
