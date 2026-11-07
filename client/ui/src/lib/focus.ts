import type { Attachment } from "svelte/attachments";

export const autofocus: Attachment<HTMLElement> = (el) => el.focus();

export const autoselect: Attachment<HTMLInputElement> = (el) => {
    el.focus();
    el.select();
};
