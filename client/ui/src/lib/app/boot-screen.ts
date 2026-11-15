function bootElement(id: string): HTMLElement {
    const el = document.getElementById(id);
    if (!el) throw new Error(`#${id} is missing from index.html`);
    return el;
}

const screen = bootElement("boot");
const title = bootElement("boot-title");
const bar = bootElement("boot-bar");
const status = bootElement("boot-status");

export function revealBootTitle(): void {
    title.classList.add("ready");
}

export function bootStage(percent: number, text: string): void {
    bar.style.width = `${percent}%`;
    status.textContent = text;
}

export function bootFailed(text: string): void {
    screen.classList.add("failed");
    status.textContent = text;
}

export async function bootDone(): Promise<void> {
    bootStage(100, "");
    await new Promise(requestAnimationFrame);
    await new Promise(requestAnimationFrame);
    screen.addEventListener("transitionend", () => screen.remove(), { once: true });
    screen.classList.add("done");
}
