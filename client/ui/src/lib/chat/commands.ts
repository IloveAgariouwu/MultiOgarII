export type ChatCommand = { kind: "bench"; report: boolean } | { kind: "say"; text: string };

export function parseCommand(text: string): ChatCommand {
    const [head, arg = ""] = text.trim().split(/\s+/);
    return head === "/bench" ? { kind: "bench", report: arg === "report" } : { kind: "say", text };
}
