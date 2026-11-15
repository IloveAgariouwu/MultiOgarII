import { profileIndex, TAB_FIELDS } from "../settings/profile-fields";
import type { ProfileState } from "../settings/types";

export function selfNames(profiles: ProfileState): string[] {
    return TAB_FIELDS.flatMap((f, tab) => {
        const name = profiles.entries[profileIndex(profiles, tab)][f.name];
        return name ? [name] : [];
    });
}
