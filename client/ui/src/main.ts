import { bootFailed, bootStage, revealBootTitle } from "./lib/boot-screen";
import { chosenLocale, loadTexts, storedPacks } from "./lib/i18n";
import { KEYS } from "./lib/store";
import "./fonts/overpass.css";

document.fonts.load('800 48px "Overpass"').then(revealBootTitle, revealBootTitle);
const imported = storedPacks();
const locale = chosenLocale(localStorage.getItem(KEYS.language), imported);
document.documentElement.lang = locale;
const texts = await loadTexts(locale, imported);
bootStage(10, "");
await (await import("./boot")).boot(texts).catch((e: unknown) => {
    bootFailed(`${texts.bootFailed} ${e instanceof Error ? e.message : String(e)}`);
    throw e;
});
