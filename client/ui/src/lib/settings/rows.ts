import type { CategorySpec, GroupSpec, Groups, RangeSpec, SettingItem, TextSpec } from "./types";

export type SettingRow =
    | { kind: "toggle"; key: string }
    | { kind: "range"; spec: RangeSpec }
    | { kind: "color"; key: string }
    | { kind: "control"; key: string }
    | { kind: "slot"; key: string; index: number }
    | { kind: "text"; spec: TextSpec }
    | { kind: "flag"; key: string; label: string }
    | { kind: "font"; key: string; label: string }
    | { kind: "group"; spec: GroupSpec };

export interface RowEntry {
    id: string;
    key: string;
    row: SettingRow;
    label: string;
}

function itemRow(item: SettingItem, slotIndex: number): SettingRow {
    if (typeof item === "string") return { kind: "toggle", key: item };
    switch (item.kind) {
        case "range":
            return { kind: "range", spec: item };
        case "text":
            return { kind: "text", spec: item };
        case "slot":
            return { kind: "slot", key: item.key, index: slotIndex };
        case "flag":
        case "font":
            return item;
        case "group":
            return { kind: "group", spec: item };
    }
}

export function categoryRows(
    cat: CategorySpec,
    groups: Groups,
    colorKeys: string[],
    t: (k: string) => string,
): RowEntry[] {
    let slots = 0;
    const rows = cat.items.map((item) => itemRow(item, typeof item !== "string" && item.kind === "slot" ? slots++ : 0));
    if (cat.id === "colors") for (const key of colorKeys) rows.push({ kind: "color", key });
    if (cat.id === "hotkeys") for (const key of groups.control) rows.push({ kind: "control", key });
    return rows.map((row) => ({ id: rowId(row), key: rowKey(row), row, label: rowLabel(row, t) }));
}

function rowKey(row: SettingRow): string {
    switch (row.kind) {
        case "range":
        case "text":
            return row.spec.key;
        case "group":
            return row.spec.key ?? row.spec.keys[0];
        default:
            return row.key;
    }
}

function rowId(row: SettingRow): string {
    return `${row.kind}:${rowKey(row)}`;
}

function rowLabel(row: SettingRow, t: (k: string) => string): string {
    switch (row.kind) {
        case "range":
        case "text":
            return t(row.spec.label);
        case "flag":
        case "font":
            return t(row.label);
        case "slot":
            return `${t("mTouchSlot")} ${row.index + 1}`;
        case "group":
            return t(row.spec.label ?? row.spec.key ?? row.spec.keys[0]);
        default:
            return t(row.key);
    }
}

export function settingKeys(row: SettingRow): string[] {
    if (row.kind === "group") return row.spec.key ? [row.spec.key, ...row.spec.keys] : row.spec.keys;
    return [rowKey(row)];
}

export function bindable(row: SettingRow): boolean {
    return row.kind === "toggle" || row.kind === "group";
}
