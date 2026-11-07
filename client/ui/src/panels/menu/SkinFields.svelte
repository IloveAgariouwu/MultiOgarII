<script lang="ts">
    import Icon from "../../components/Icon.svelte";
    import { useApp } from "../../lib/context";
    import type { MenuModel } from "../../lib/menu-model.svelte";

    const UNITS: (0 | 1)[] = [0, 1];

    let { m }: { m: MenuModel } = $props();
    const app = useApp();
    const t = app.t;
    let eyeLabel = $derived(t(m.external ? "menuEyeOn" : "menuEyeOff"));
</script>

<div class="skins">
    {#each UNITS as u (u)}
        <input
            class="f"
            spellcheck="false"
            value={m.external ? m.profile[m.fields.self[u]] : m.profile[m.fields.skin[u]]}
            placeholder={m.external ? t("menuExternalFallback") : t("menuUnitSkin").replace("%s", String(u + 1))}
            title={m.external ? t("menuExternalFallback") : t(u === 0 ? "menuSkin" : "menuSkin2")}
            oninput={(e) =>
                app.setProfileField(m.index, m.external ? m.fields.self[u] : m.fields.skin[u], e.currentTarget.value)}
        />
    {/each}
    <button
        class="icon eye"
        class:on={m.external}
        title={eyeLabel}
        aria-label={eyeLabel}
        aria-pressed={m.external}
        onclick={() => (m.external = !m.external)}
    >
        <Icon name={m.external ? "eyeOff" : "eye"} />
    </button>
</div>

<style>
    .skins {
        display: grid;
        grid-template-columns: 1fr 1fr 2.4rem;
        gap: 0.6rem;
        margin-top: 0.8rem;
    }

    .eye {
        border: 1px solid var(--rule);
        background: var(--field);
    }

    .eye.on {
        color: var(--accent);
        border-color: var(--accent);
    }

    :global(html.mobile) .skins {
        order: 4;
        grid-template-columns: 1fr 1fr var(--row);
        gap: 0.5rem;
        margin-top: 0;
    }

    :global(html.mobile) .f {
        height: var(--row);
    }

    :global(html.mobile) .eye {
        height: var(--row);
        border: 0;
    }
</style>
