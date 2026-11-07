<script lang="ts">
    import { useApp, useGame } from "../../lib/context";
    import type { TextKey } from "../../lib/i18n";

    let { server: showServer, client }: { server: boolean; client: boolean } = $props();
    const app = useApp();
    const game = useGame();
    const t = app.t;
    const SERVER_LABELS: Record<string, TextKey> = {
        Load: "statServerLoad",
        Uptime: "statUptime",
    };
    const UNIT_LABELS: Record<string, TextKey> = { min: "statUnitMin", h: "statUnitHour", d: "statUnitDay" };
    const LINE = /^(.+?):\s*(-?[\d.]+)\s*(\S*)$/;
    let st = $derived(game.stats);
    let server = $derived(
        st.serverStatus
            .split("\n")
            .filter((l) => l.trim())
            .map((l) => {
                const m = LINE.exec(l.trim());
                return m
                    ? {
                          label: Object.hasOwn(SERVER_LABELS, m[1]) ? t(SERVER_LABELS[m[1]]) : m[1],
                          value: m[2],
                          unit: Object.hasOwn(UNIT_LABELS, m[3]) ? t(UNIT_LABELS[m[3]]) : m[3],
                      }
                    : { label: l.trim(), value: "", unit: "" };
            }),
    );
</script>

<dl>
    {#if showServer}
        {#each server as row, i (i)}
            <dt>{row.label}</dt>
            <dd>{row.value}</dd>
            <dd class="u">{row.unit}</dd>
        {/each}
        {#if server.length && client}<hr />{/if}
    {/if}
    {#if client}
        <dt>{t("statFps")}</dt>
        <dd>{st.fps}</dd>
        <dd class="u"></dd>
        <dt>{t("statRender")}</dt>
        <dd>{st.render.toFixed(2)}</dd>
        <dd class="u">ms</dd>
        <dt>{t("statLatency")}</dt>
        <dd>{st.latency}</dd>
        <dd class="u">ms</dd>
        {#if game.tab2Connected}<dt>{t("statLatency2")}</dt>
            <dd>{st.latency2}</dd>
            <dd class="u">ms</dd>{/if}
    {/if}
</dl>

<style>
    dl {
        display: grid;
        grid-template-columns: auto auto auto;
        justify-content: start;
        align-items: baseline;
        gap: 0.05rem 0.45rem;
        margin: 0;
        line-height: 1.3;
    }

    dt {
        color: var(--mute);
    }

    dd {
        margin: 0;
        text-align: right;
        font-weight: 600;
    }

    .u {
        min-width: 1.4rem;
        text-align: left;
        color: var(--mute);
        font-weight: 400;
        font-size: var(--fs-xs);
    }

    hr {
        grid-column: 1 / -1;
        width: 100%;
        margin: 0.2rem 0;
        border: 0;
        border-top: 1px solid var(--rule);
    }
</style>
