export interface PipeEvents {
    open(): void;
    message(data: Uint8Array): void;
    closed(reason: string): void;
}

export class SocketPipe {
    private ws: WebSocket;

    constructor(url: string, on: PipeEvents) {
        this.ws = new WebSocket(url);
        this.ws.binaryType = "arraybuffer";
        this.ws.onopen = () => on.open();
        this.ws.onmessage = (e) => {
            if (e.data instanceof ArrayBuffer) on.message(new Uint8Array(e.data));
        };
        this.ws.onclose = (e) => on.closed(e.reason);
    }

    get ready(): boolean {
        return this.ws.readyState === WebSocket.OPEN;
    }

    send(data: Uint8Array): void {
        this.ws.send(data.slice());
    }

    close(): void {
        this.ws.onclose = null;
        this.ws.close();
    }
}

export const SOLO_WORKER_PREFIX = "solo-";

export class SoloPipe {
    private port: MessagePort | null;
    private opened = false;
    private readonly leave = () => this.close();

    /** Each gamemode gets its own shared worker, named after the mode it runs. */
    constructor(on: PipeEvents, mode: number) {
        const worker = new SharedWorker(new URL("./solo-worker.ts", import.meta.url), {
            type: "module",
            name: `${SOLO_WORKER_PREFIX}${mode}`,
        });
        const port = worker.port;
        this.port = port;
        port.onmessage = (e: MessageEvent) => {
            if (e.data === true) {
                this.opened = true;
                on.open();
            } else if (e.data instanceof ArrayBuffer) {
                on.message(new Uint8Array(e.data));
            } else if (typeof e.data === "string") {
                this.shut();
                on.closed(e.data);
            }
        };
        worker.onerror = () => {
            this.shut();
            on.closed("");
        };
        addEventListener("pagehide", this.leave);
    }

    get ready(): boolean {
        return this.opened;
    }

    send(data: Uint8Array): void {
        const copy = data.slice();
        this.port?.postMessage(copy.buffer, [copy.buffer]);
    }

    close(): void {
        this.port?.postMessage(null);
        this.shut();
    }

    private shut(): void {
        removeEventListener("pagehide", this.leave);
        this.opened = false;
        this.port?.close();
        this.port = null;
    }
}
