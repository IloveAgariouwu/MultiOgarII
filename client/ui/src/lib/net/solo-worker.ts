import init, { Solo } from "../../pkg/ogar_web";
import { SOLO_WORKER_PREFIX } from "./pipes";

const TICK_MS = 40;
const IDLE_MS = 120000;

interface Link {
    id: number;
    seen: number;
}

const links = new Map<MessagePort, Link>();
const mode = Number(self.name.slice(SOLO_WORKER_PREFIX.length)) || 0;
const ready = init().then(() => {
    const solo = new Solo(Date.now(), mode);
    setInterval(() => tick(solo), TICK_MS);
    return solo;
});

function drop(solo: Solo, port: MessagePort, link: Link): void {
    links.delete(port);
    solo.close(link.id);
}

function deliver(solo: Solo, port: MessagePort, link: Link): void {
    for (const frame of solo.drain(link.id)) port.postMessage(frame.buffer, [frame.buffer]);
    const reason = solo.closed(link.id);
    if (reason === undefined) return;
    drop(solo, port, link);
    port.postMessage(reason);
}

function tick(solo: Solo): void {
    const now = Date.now();
    solo.tick(now);
    for (const [port, link] of links) {
        if (now - link.seen > IDLE_MS) drop(solo, port, link);
        else deliver(solo, port, link);
    }
}

function receive(solo: Solo, port: MessagePort, data: unknown): void {
    const link = links.get(port);
    if (!link) return;
    link.seen = Date.now();
    if (data === null) {
        drop(solo, port, link);
        port.close();
    } else if (data instanceof ArrayBuffer) {
        solo.send(link.id, new Uint8Array(data), link.seen);
        deliver(solo, port, link);
    }
}

addEventListener("connect", (e) => {
    if (!(e instanceof MessageEvent)) return;
    const port = e.ports[0];
    void ready.then((solo) => {
        links.set(port, { id: solo.open(navigator.userAgent, Date.now()), seen: Date.now() });
        port.addEventListener("message", (m) => receive(solo, port, m.data));
        port.start();
        port.postMessage(true);
    });
});
