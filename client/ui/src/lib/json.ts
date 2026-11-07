export function fromJson<T>(text: string): T {
    return JSON.parse(text);
}
