const RECONNECT_MAX_MS = 60_000;

export function reconnectDelay(intervalSec: number, doubling: boolean, retries: number): number {
    const interval = intervalSec * 1000;
    return doubling ? Math.min(interval * 2 ** retries, RECONNECT_MAX_MS) : interval;
}
