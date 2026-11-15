import js from "@eslint/js";
import prettier from "eslint-config-prettier";
import svelte from "eslint-plugin-svelte";
import globals from "globals";
import ts from "typescript-eslint";
import svelteConfig from "./svelte.config.js";

export default ts.config(
    { ignores: ["src/pkg/", "dist/"] },
    js.configs.recommended,
    ts.configs.recommended,
    svelte.configs.recommended,
    prettier,
    svelte.configs.prettier,
    {
        languageOptions: { globals: { ...globals.browser } },
        rules: {
            "@typescript-eslint/consistent-type-assertions": ["error", { assertionStyle: "never" }],
            "@typescript-eslint/no-non-null-assertion": "error",
            "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_", ignoreRestSiblings: true }],
            "svelte/prefer-svelte-reactivity": "off",
        },
    },
    {
        files: ["**/*.ts", "**/*.svelte"],
        rules: { "no-undef": "off" },
    },
    {
        files: ["**/*.svelte", "**/*.svelte.ts"],
        languageOptions: { parserOptions: { parser: ts.parser, extraFileExtensions: [".svelte"], svelteConfig } },
    },
);
