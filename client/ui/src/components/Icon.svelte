<script lang="ts" module>
    export type IconName =
        | "up"
        | "down"
        | "grid"
        | "link"
        | "unlink"
        | "eye"
        | "eyeOff"
        | "gear"
        | "users"
        | "close"
        | "menu"
        | "chat"
        | "two"
        | "play"
        | "pause"
        | "stop"
        | "record"
        | "bolt"
        | "prev"
        | "next"
        | "stepBack"
        | "stepForward"
        | "trash"
        | "sync"
        | "loop"
        | "person"
        | "move"
        | "download"
        | "upload"
        | "film"
        | "markIn"
        | "markOut"
        | "split"
        | "feed"
        | "feedOne"
        | "double"
        | "x16"
        | "trophy"
        | "layers"
        | "swap"
        | "grip"
        | "left"
        | "right"
        | "edit";
</script>

<script lang="ts">
    let { name }: { name: IconName } = $props();
</script>

<svg viewBox="0 0 24 24" aria-hidden="true">
    {#if name === "up"}<path d="M6 15l6-6 6 6" />
    {:else if name === "down"}<path d="M6 9l6 6 6-6" />
    {:else if name === "left"}<path d="M15 6l-6 6 6 6" />
    {:else if name === "right"}<path d="M9 6l6 6-6 6" />
    {:else if name === "edit"}<path d="M4 20l1-4.5L16 4.5l3.5 3.5L8.5 19z" /><path d="M14 7l3.5 3.5" />
    {:else if name === "grid"}<rect x="4" y="4" width="6" height="6" /><rect x="14" y="4" width="6" height="6" /><rect
            x="4"
            y="14"
            width="6"
            height="6"
        /><rect x="14" y="14" width="6" height="6" />
    {:else if name === "link"}<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1" /><path
            d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"
        />
    {:else if name === "unlink"}<path d="M15.7 14l3-3a4 4 0 0 0-5.7-5.7l-1 1" /><path
            d="M8.3 10l-3 3a4 4 0 0 0 5.7 5.7l1-1"
        /><path d="M8 3v3M3 8h3M16 21v-3M21 16h-3" />
    {:else if name === "eye"}<path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z" /><circle
            cx="12"
            cy="12"
            r="3"
        />
    {:else if name === "eyeOff"}<path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z" /><circle
            cx="12"
            cy="12"
            r="3"
        /><path d="M4 4l16 16" />
    {:else if name === "gear"}<path
            d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"
        /><circle cx="12" cy="12" r="3" />
    {:else if name === "users"}<circle cx="9" cy="8" r="3.5" /><path
            d="M2.5 20c.8-3.5 3.4-5.5 6.5-5.5s5.7 2 6.5 5.5"
        /><path d="M16 4.6a3.5 3.5 0 0 1 0 6.8M18.5 14.8c1.6.8 2.6 2.6 3 5.2" />
    {:else if name === "close"}<path d="M6 6l12 12M18 6L6 18" />
    {:else if name === "menu"}<path d="M4 7h16M4 12h16M4 17h16" />
    {:else if name === "chat"}<path d="M4 5h16v11H9l-5 4z" />
    {:else if name === "two"}<rect x="3" y="6" width="9" height="12" /><path d="M15 9h3v9h-6" />
    {:else if name === "split"}<path d="M12 21v-8L6 5M12 13l6-8M6 5h4M6 5v4M18 5h-4M18 5v4" />
    {:else if name === "feed"}<circle cx="8.5" cy="12" r="5.5" /><circle cx="17" cy="12" r="1.4" /><circle
            cx="21"
            cy="12"
            r="0.9"
        />
    {:else if name === "feedOne"}<circle cx="9" cy="12" r="5.5" /><circle cx="18.5" cy="12" r="1.8" />
    {:else if name === "double"}<circle cx="8" cy="8" r="3" /><circle cx="16" cy="8" r="3" /><circle
            cx="8"
            cy="16"
            r="3"
        /><circle cx="16" cy="16" r="3" />
    {:else if name === "x16"}{#each [4.5, 9.5, 14.5, 19.5] as y (y)}{#each [4.5, 9.5, 14.5, 19.5] as x (x)}<circle
                    cx={x}
                    cy={y}
                    r="1"
                />{/each}{/each}
    {:else if name === "trophy"}<path
            d="M8 4h8v5a4 4 0 0 1-8 0zM8 6H5a3 3 0 0 0 3 4M16 6h3a3 3 0 0 1-3 4M12 13v4M8 20h8M10 17h4"
        />
    {:else if name === "layers"}<path d="M12 4l8 4-8 4-8-4zM4 12l8 4 8-4M4 16l8 4 8-4" />
    {:else if name === "swap"}<path d="M4 8h14l-4-4M20 16H6l4 4" />
    {:else if name === "grip"}<circle cx="9" cy="6" r="1" /><circle cx="15" cy="6" r="1" /><circle
            cx="9"
            cy="12"
            r="1"
        /><circle cx="15" cy="12" r="1" /><circle cx="9" cy="18" r="1" /><circle cx="15" cy="18" r="1" />
    {:else if name === "play"}<path d="M8 5l11 7-11 7z" />
    {:else if name === "pause"}<path d="M8 5v14M16 5v14" />
    {:else if name === "stop"}<rect x="6" y="6" width="12" height="12" />
    {:else if name === "record"}<circle cx="12" cy="12" r="6" />
    {:else if name === "bolt"}<path d="M13 3L5 13.5h6L10 21l8-10.5h-6z" />
    {:else if name === "prev"}<path d="M6 5v14M18 5l-9 7 9 7z" />
    {:else if name === "next"}<path d="M18 5v14M6 5l9 7-9 7z" />
    {:else if name === "stepBack"}<path d="M11 6l-6 6 6 6M19 6l-6 6 6 6" />
    {:else if name === "stepForward"}<path d="M13 6l6 6-6 6M5 6l6 6-6 6" />
    {:else if name === "download"}<path d="M12 4v11M7 10l5 5 5-5M5 20h14" />
    {:else if name === "upload"}<path d="M12 20V9M7 14l5-5 5 5M5 4h14" />
    {:else if name === "film"}<rect x="3" y="5" width="18" height="14" /><path
            d="M7 5v14M17 5v14M3 9h4M3 15h4M17 9h4M17 15h4"
        />
    {:else if name === "markIn"}<path d="M10 5H6v14h4M13 12h7M17 9l3 3-3 3" />
    {:else if name === "markOut"}<path d="M14 5h4v14h-4M11 12H4M7 9l-3 3 3 3" />
    {:else if name === "trash"}<path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13" />
    {:else if name === "sync"}<path d="M20 12a8 8 0 0 1-14 5.3M4 12a8 8 0 0 1 14-5.3" /><path
            d="M18 3v4h-4M6 21v-4h4"
        />
    {:else if name === "loop"}<path d="M4 6h11M4 11h11M4 16h6M15 14.5v6l5-3z" />
    {:else if name === "person"}<circle cx="12" cy="8" r="4" /><path d="M4 21c1-4.5 4-7 8-7s7 2.5 8 7" />
    {:else if name === "move"}<path
            d="M12 3v18M3 12h18M12 3l-3 3M12 3l3 3M12 21l-3-3M12 21l3-3M3 12l3-3M3 12l3 3M21 12l-3-3M21 12l-3 3"
        />
    {/if}
</svg>
