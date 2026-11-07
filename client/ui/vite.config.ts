import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
    base: "./",
    plugins: [svelte()],
    build: {
        target: "es2022",
    },
    worker: {
        format: "es",
    },
    server: {
        port: 3000,
    },
    preview: {
        port: 3000,
    },
});
