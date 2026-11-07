<script lang="ts">
    import WorldCanvas from "../components/WorldCanvas.svelte";

    const VIEW = { zoom: 0.55, camera: "drift" } as const;
</script>

<WorldCanvas view={VIEW} class="world" />

<style>
    :global(canvas.world) {
        position: fixed;
        inset: 0;
        width: 100%;
        height: 100%;
        display: block;
    }
</style>
