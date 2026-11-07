<script lang="ts">
    import { useApp } from "../lib/context";
    import { bytes } from "../lib/format";
    import { STORE_LOW_MS } from "../lib/replay.svelte";

    const app = useApp();
    const t = app.t;
    const r = app.replay;
</script>

{#if r.usage}
    {@const quota = r.usage.quota}
    {@const reels = r.storedBytes}
    {@const other = Math.max(0, r.usage.used - reels)}
    {@const left = r.recordableMs}
    <div class="storage" class:low={left !== null && left < STORE_LOW_MS}>
        <div class="bar" role="meter" aria-valuemin={0} aria-valuemax={quota} aria-valuenow={r.usage.used}>
            <span class="reels" style:width={`${(reels / quota) * 100}%`}></span>
            <span class="other" style:width={`${(other / quota) * 100}%`}></span>
        </div>
        <div class="legend">
            <span><i class="reels"></i>{t("replayStorageReels").replace("%s", bytes(reels))}</span>
            <span><i class="other"></i>{t("replayStorageOther").replace("%s", bytes(other))}</span>
            <span>{t("replayStorageFree").replace("%s", bytes(quota - r.usage.used))}</span>
            {#if left !== null}<span class="time"
                    >{t("replayRecordable").replace(
                        "%s",
                        left < 7200000
                            ? t("replayMinutes").replace("%s", String(Math.ceil(left / 60000)))
                            : t("replayHours").replace("%s", String(Math.floor(left / 3600000))),
                    )}</span
                >{/if}
        </div>
        {#if r.persisted === false}<div class="note" title={t("replayNotPersistedHint")}>
                {t("replayNotPersisted")}
            </div>{/if}
    </div>
{/if}

<style>
    .storage {
        display: grid;
        gap: 0.3rem;
        min-width: 14rem;
        color: var(--mute);
        font-size: var(--fs-xs);
        font-variant-numeric: tabular-nums;
    }

    .bar {
        display: flex;
        height: 0.4rem;
        overflow: hidden;
        background: var(--well);
    }

    .reels {
        background: var(--accent);
    }

    .other {
        background: var(--dim);
    }

    .legend {
        display: flex;
        flex-wrap: wrap;
        gap: 0.2rem 0.8rem;
    }

    .legend i {
        display: inline-block;
        width: 0.5rem;
        height: 0.5rem;
        margin-right: 0.3rem;
    }

    .low .time {
        color: var(--danger);
    }

    .low .bar .reels {
        background: var(--danger);
    }

    .note {
        color: var(--dim);
    }
</style>
