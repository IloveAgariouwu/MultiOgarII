<script lang="ts">
    import type { Attachment } from "svelte/attachments";
    import Icon from "../../components/Icon.svelte";
    import Select from "../../components/Select.svelte";
    import { useApp, useGame } from "../../lib/context";
    import { GOOGLE_FONTS, loadGoogleFont } from "../../lib/google-fonts";
    import { HUD_ZOOM, snapZoom, type HudSlot } from "../../lib/hud-layout.svelte";
    import { PERCENT } from "../../lib/settings-format";
    import ValueInput from "../settings/ValueInput.svelte";

    const app = useApp();
    const game = useGame();
    const t = app.t;
    let pendingFont = $state<{ slot: HudSlot; value: string; progress: number } | null>(null);
    const fonts = [
        { value: "", label: t("hudFontGlobal") },
        ...GOOGLE_FONTS.map((f) => ({ value: f.name, label: f.name })),
    ];

    const outside: Attachment<HTMLElement> = (node) => {
        const down = (e: PointerEvent) => {
            if (e.target instanceof Element && (node.contains(e.target) || e.target.closest(".hud-edit"))) return;
            app.hud.selected = null;
        };
        addEventListener("pointerdown", down);
        return () => removeEventListener("pointerdown", down);
    };
</script>

<div class="edit">
    <div class="row" {@attach app.layer(() => (app.hud.editing = false))}>
        <Icon name="move" />
        <span>{t("hudEditHint")}</span>
        <button class="btn-line" disabled={!app.hud.customized} onclick={() => app.hud.reset()}
            >{t("hudEditReset")}</button
        >
        <button class="btn-line done" onclick={() => (app.hud.editing = false)}>{t("hudEditDone")}</button>
    </div>
    {#if app.hud.selected}
        {@const slot = app.hud.selected}
        {@const size = Math.round((app.hud.scales[slot] ?? 1) * PERCENT)}
        {@const text = Math.round((app.hud.texts[slot] ?? 1) * PERCENT)}
        <div class="row pick" {@attach app.layer(() => (app.hud.selected = null))} {@attach outside}>
            <b>{t(`hudSlot_${slot}`)}</b>
            <span class="lbl">{t("hudSize")}</span>
            <input
                class="slider"
                type="range"
                min={HUD_ZOOM.min}
                max={HUD_ZOOM.max}
                step={HUD_ZOOM.step}
                value={size}
                style:--p={`${((size - HUD_ZOOM.min) / (HUD_ZOOM.max - HUD_ZOOM.min)) * 100}%`}
                aria-label={t("hudSize")}
                oninput={(e) => app.hud.setScale(slot, snapZoom(Number(e.currentTarget.value)))}
            />
            <output
                ><ValueInput
                    text={`${size}%`}
                    value={size}
                    label={t("hudSize")}
                    title={t("setValueEditHint")}
                    onset={(v) => app.hud.setScale(slot, snapZoom(v))}
                /></output
            >
            <span class="lbl">{t("hudTextSize")}</span>
            <input
                class="slider"
                type="range"
                min={HUD_ZOOM.min}
                max={HUD_ZOOM.max}
                step={HUD_ZOOM.step}
                value={text}
                style:--p={`${((text - HUD_ZOOM.min) / (HUD_ZOOM.max - HUD_ZOOM.min)) * 100}%`}
                aria-label={t("hudTextSize")}
                oninput={(e) => app.hud.setText(slot, snapZoom(Number(e.currentTarget.value)))}
            />
            <output
                ><ValueInput
                    text={`${text}%`}
                    value={text}
                    label={t("hudTextSize")}
                    title={t("setValueEditHint")}
                    onset={(v) => app.hud.setText(slot, snapZoom(v))}
                /></output
            >
            <span class="lbl">{t("hudFont")}</span>
            <Select
                options={fonts}
                value={pendingFont?.slot === slot ? pendingFont.value : (app.hud.fonts[slot] ?? "")}
                label={t("hudFont")}
                progress={pendingFont?.slot === slot ? pendingFont.progress : undefined}
                onchange={async (v) => {
                    pendingFont = { slot, value: v, progress: 0 };
                    const pick = pendingFont;
                    const shown = slot === "map" ? game.units.map((u) => u.name).join("") : document.body.innerText;
                    const ok = await loadGoogleFont(v, `${shown} `, (done, total) => (pick.progress = done / total));
                    if (pendingFont !== pick) return;
                    if (ok) app.hud.setFont(slot, v);
                    pendingFont = null;
                }}
            />
            <button class="btn-line" onclick={() => app.hud.resetSlot(slot)}>{t("hudSlotReset")}</button>
        </div>
    {/if}
</div>

<style>
    .edit {
        position: fixed;
        left: 50%;
        top: 50%;
        transform: translate(-50%, -50%);
        display: flex;
        flex-direction: column;
        gap: 0.8rem;
        padding: 0.8rem 1.1rem;
        background: var(--panel);
        z-index: var(--z-bar);
    }

    .row {
        display: flex;
        align-items: center;
        gap: 1rem;
    }

    .pick {
        padding-top: 0.8rem;
        border-top: 1px solid var(--rule);
    }

    .lbl {
        color: var(--mute);
    }

    .slider {
        width: 11rem;
    }

    output {
        width: 4rem;
        color: var(--mute);
    }

    .row > :global(svg) {
        width: var(--icon);
        height: var(--icon);
        fill: none;
        stroke: var(--accent);
        stroke-width: 1.7;
    }

    .done {
        border-color: var(--accent);
        color: var(--accent);
    }
</style>
