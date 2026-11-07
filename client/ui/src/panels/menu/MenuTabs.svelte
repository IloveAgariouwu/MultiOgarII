<script lang="ts">
    import Icon from "../../components/Icon.svelte";
    import { useApp } from "../../lib/context";
    import type { MenuModel } from "../../lib/menu-model.svelte";

    let { m }: { m: MenuModel } = $props();
    const app = useApp();
    const t = app.t;
    let linkLabel = $derived(t(app.profiles.linked ? "menuLinked" : "menuUnlinked"));
</script>

{#each [0, 1] as n (n)}
    {#if n === 1}
        <button
            class="icon link"
            class:unlinked={!app.profiles.linked}
            title={linkLabel}
            aria-label={linkLabel}
            aria-pressed={app.profiles.linked}
            onclick={() => m.toggleLink()}
        >
            <Icon name={app.profiles.linked ? "link" : "unlink"} />
        </button>
    {/if}
    <button class="tabsw" aria-pressed={m.tab === n} onclick={() => (m.tab = n)}>
        {t("tabN").replace("%s", String(n + 1))}{#if !app.profiles.linked}<small
                >{(n ? app.profiles.second : app.profiles.main) + 1}</small
            >{/if}
    </button>
{/each}

<style>
    small {
        margin-left: 0.35rem;
        color: var(--accent);
        font-size: var(--fs-xs);
    }

    .link {
        align-self: center;
        width: var(--hit);
        height: var(--hit);
        margin: 0 -0.6rem;
    }

    .link :global(svg) {
        width: var(--icon-sm);
        height: var(--icon-sm);
    }

    .link.unlinked {
        color: var(--accent);
    }

    :global(html.mobile) .link {
        height: var(--row);
        margin: 0 -0.5rem;
    }

    :global(html.mobile) .tabsw {
        white-space: nowrap;
    }
</style>
