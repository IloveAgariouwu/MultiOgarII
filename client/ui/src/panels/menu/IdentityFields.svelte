<script lang="ts">
    import { useApp } from "../../lib/context";
    import type { MenuModel } from "../../lib/menu-model.svelte";

    let { m }: { m: MenuModel } = $props();
    const app = useApp();
    const t = app.t;
    const limits = app.limits;
</script>

<input
    class="f"
    maxlength={limits.profileNameMax}
    placeholder={t("menuNickname")}
    spellcheck="false"
    value={m.profile[m.fields.name]}
    oninput={(e) => app.setProfileField(m.index, m.fields.name, e.currentTarget.value)}
/>

<style>
    .f {
        display: block;
        margin-top: 0.6rem;
    }

    :global(html.mobile) .f {
        order: 5;
        height: var(--row);
        margin-top: 0;
    }
</style>
