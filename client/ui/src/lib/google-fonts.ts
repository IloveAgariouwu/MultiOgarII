export interface GoogleFont {
    name: string;
    weights: readonly number[];
}

const ALL = [100, 200, 300, 400, 500, 600, 700, 800, 900];
const from = (min: number, max: number) => ALL.filter((w) => w >= min && w <= max);

export const GOOGLE_FONTS: readonly GoogleFont[] = [
    { name: "Noto Sans TC", weights: ALL },
    { name: "Noto Serif TC", weights: from(200, 900) },
    { name: "LXGW WenKai TC", weights: [300, 400, 700] },
    { name: "Noto Sans SC", weights: ALL },
    { name: "Noto Serif SC", weights: from(200, 900) },
    { name: "ZCOOL KuaiLe", weights: [400] },
    { name: "Noto Sans JP", weights: ALL },
    { name: "M PLUS Rounded 1c", weights: [100, 300, 400, 500, 700, 800, 900] },
    { name: "Zen Maru Gothic", weights: [300, 400, 500, 700, 900] },
    { name: "DotGothic16", weights: [400] },
    { name: "Roboto", weights: [100, 300, 400, 500, 700, 900] },
    { name: "Inter", weights: ALL },
    { name: "Open Sans", weights: from(300, 800) },
    { name: "Montserrat", weights: ALL },
    { name: "Poppins", weights: ALL },
    { name: "Nunito", weights: from(200, 900) },
    { name: "Ubuntu", weights: [300, 400, 500, 700] },
    { name: "Rubik", weights: from(300, 900) },
    { name: "Fredoka", weights: from(300, 700) },
    { name: "Comfortaa", weights: from(300, 700) },
    { name: "Exo 2", weights: ALL },
    { name: "Rajdhani", weights: from(300, 700) },
    { name: "Orbitron", weights: from(400, 900) },
    { name: "Press Start 2P", weights: [400] },
    { name: "Silkscreen", weights: [400, 700] },
];

const WANTED_WEIGHTS = [400, 500, 600, 700];
const requested = new Map<string, Promise<boolean>>();

function weightRank(want: number, have: number): number {
    const distance = Math.abs(have - want);
    if (want >= 400 && want <= 500) return (have >= want && have <= 500 ? 0 : have < want ? 1 : 2) * 1000 + distance;
    return ((want > 500 ? have >= want : have <= want) ? 0 : 1) * 1000 + distance;
}

export function servedWeights(font: GoogleFont): number[] {
    const picked = WANTED_WEIGHTS.map((want) =>
        font.weights.reduce((best, w) => (weightRank(want, w) < weightRank(want, best) ? w : best)),
    );
    return [...new Set(picked)].sort((a, b) => a - b);
}

export function googleFontUrl(font: GoogleFont): string {
    return `https://fonts.googleapis.com/css2?family=${font.name.replaceAll(" ", "+")}:wght@${servedWeights(font).join(";")}&display=swap`;
}

function rangeCovers(range: string, codes: number[]): boolean {
    return range.split(",").some((part) => {
        const [lo, hi = lo] = part
            .trim()
            .slice(2)
            .split("-")
            .map((h) => parseInt(h, 16));
        let [a, b] = [0, codes.length];
        while (a < b) {
            const mid = (a + b) >> 1;
            if (codes[mid] < lo) a = mid + 1;
            else b = mid;
        }
        return a < codes.length && codes[a] <= hi;
    });
}

export async function loadGoogleFont(
    name: string,
    text = " ",
    onProgress?: (done: number, total: number) => void,
): Promise<boolean> {
    const font = GOOGLE_FONTS.find((f) => f.name === name);
    if (!font) return true;
    const url = googleFontUrl(font);
    const sheet =
        requested.get(name) ??
        fetch(url)
            .then((r) => (r.ok ? r.text() : Promise.reject(new Error(`HTTP ${r.status}`))))
            .then((css) => {
                const style = document.createElement("style");
                style.textContent = css;
                document.head.append(style);
                return true;
            })
            .catch((e) => {
                requested.delete(name);
                console.error(`google font ${name} failed to load from ${url}`, e);
                return false;
            });
    requested.set(name, sheet);
    onProgress?.(0, 1);
    if (!(await sheet)) return false;
    const weights = servedWeights(font);
    const codes = [...new Set(text)].map((c) => c.codePointAt(0) ?? 0).sort((x, y) => x - y);
    const faces = [...document.fonts].filter(
        (f) =>
            f.family.replace(/^['"]|['"]$/g, "") === name &&
            weights.includes(Number(f.weight)) &&
            rangeCovers(f.unicodeRange, codes),
    );
    let done = 1;
    const total = faces.length + 1;
    onProgress?.(done, total);
    return Promise.all(faces.map((f) => f.load().then(() => onProgress?.(++done, total)))).then(
        () => true,
        (e) => {
            console.error(`google font ${name} failed to load`, e);
            return false;
        },
    );
}
