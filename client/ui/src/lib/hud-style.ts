import type { ChatSize } from "./chat.svelte";
import { argbCss } from "./format";

export function hudVars(colors: Record<string, number>): string {
    return `--lb-back:${argbCss(colors.clLeaderboardBack)};--lb-head:${argbCss(colors.clLeaderboardHeader)};--chat-text:${argbCss(colors.clChatMessage)}`;
}

export function chatVars(colors: Record<string, number>, size: ChatSize): string {
    return `--chat-back:${argbCss(colors.clChatBackground)};--chat-text:${argbCss(colors.clChatMessage)};--chat-time:${argbCss(colors.clChatTimeString)};--chat-name:${argbCss(colors.clChatSenderName)};--scrim:var(--chat-back);--chat-w:${size.width}rem;--log-h:${size.log}rem`;
}
