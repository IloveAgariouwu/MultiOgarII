import { PreviewWorld, World } from "../pkg/ogar_web";
import { GameState, type LeaderEntry, type UnitEntry } from "./game.svelte";
import { fromJson } from "./json";
import type { MinimapLayout } from "./types";

interface Person {
    name: string;
}

interface PreviewPlayer extends UnitEntry {
    rank: number;
    self: boolean;
}

interface Group {
    owner: number;
    x: number;
    y: number;
    area: number;
    biggest: number;
    rgb: number;
    flags: number;
}

const SEED = 7;
const ROWS = ["A", "B", "C", "D", "E"];
const GRID = 5;
const MASS_PER_AREA = 0.01;
const LEADERS = 10;
const SERVER_MAX = 120;
const MAP_SPOTS: Record<number, [number, number]> = {
    4: [0.3, 0.92],
    6: [0.86, 0.12],
    10: [0.76, 0.56],
    12: [0.1, 0.12],
    16: [0.25, 0.3],
    22: [0.92, 0.42],
    28: [0.3, 0.68],
    30: [0.1, 0.42],
    36: [0.1, 0.86],
    40: [0.86, 0.74],
    46: [0.6, 0.68],
    52: [0.55, 0.9],
    60: [0.74, 0.92],
    64: [0.9, 0.88],
    68: [0.42, 0.82],
    70: [0.08, 0.64],
};

class SpreadWorld extends World {
    constructor(private readonly layout: MinimapLayout) {
        super();
    }

    minimapCells(): Float64Array {
        const cells = super.minimapCells();
        const { stride, owner, x, y, radius } = this.layout;
        const sums = new Map<number, [number, number, number]>();
        for (let i = 0; i < cells.length; i += stride) {
            const player = cells[i + owner] & ~1;
            const area = cells[i + radius] ** 2;
            const sum = sums.get(player) ?? [0, 0, 0];
            sum[0] += cells[i + x] * area;
            sum[1] += cells[i + y] * area;
            sum[2] += area;
            sums.set(player, sum);
        }
        const shifts = new Map<number, [number, number]>();
        for (const [player, [sx, sy, area]] of sums) {
            const to = MAP_SPOTS[player];
            if (to) shifts.set(player, [to[0] - sx / area, to[1] - sy / area]);
        }
        for (let i = 0; i < cells.length; i += stride) {
            const shift = shifts.get(cells[i + owner] & ~1);
            if (!shift) continue;
            cells[i + x] += shift[0];
            cells[i + y] += shift[1];
        }
        return cells;
    }
}

export class PreviewStage {
    readonly world: World;
    readonly game = new GameState();
    roster: PreviewPlayer[] = [];
    private readonly scene: PreviewWorld;

    constructor(
        private readonly layout: MinimapLayout,
        private readonly selfName: string,
    ) {
        this.world = new SpreadWorld(layout);
        this.scene = new PreviewWorld(this.world, SEED);
        this.refresh();
    }

    step(now: number): void {
        this.scene.updateFrame(this.world, now);
        this.refresh();
    }

    private refresh(): void {
        const players = previewPlayers(this.world, this.layout, this.selfName);
        const game = this.game;
        this.roster = players;
        game.units = players.filter((p) => p.self);
        game.leaderboard = players
            .slice(0, LEADERS)
            .map((p): LeaderEntry => ({ rank: p.rank, name: p.name, self: p.self }));
        game.population = { total: players.length, max: SERVER_MAX, players: players.length, spectators: 0 };
        const me = players.find((p) => p.self);
        const cells = this.world.minimapCells();
        let own = 0;
        for (let i = 0; i < cells.length; i += this.layout.stride)
            if (cells[i + this.layout.flags] & this.layout.own) own++;
        if (me) game.self = { ...game.self, pos1: me.cell, score: me.mass, max: me.mass, split1: own };
    }

    free(): void {
        this.scene.free();
        this.world.free();
    }
}

export function previewChat(game: GameState, roster: PreviewPlayer[], lines: string[]): void {
    const talkers = roster.filter((p) => !p.self);
    const time = new Date();
    lines.forEach((line, i) => {
        const p = talkers[i % talkers.length];
        game.pushChat({ name: p.name, text: line, color: p.color, time });
    });
}

function previewPlayers(world: World, layout: MinimapLayout, selfName: string): PreviewPlayer[] {
    const cells = world.minimapCells();
    const [left, , right] = world.border();
    const width = right - left;
    const groups = new Map<number, Group>();
    for (let i = 0; i < cells.length; i += layout.stride) {
        const owner = cells[i + layout.owner];
        const r = cells[i + layout.radius];
        const area = r * r;
        const g = groups.get(owner) ?? {
            owner,
            x: 0,
            y: 0,
            area: 0,
            biggest: 0,
            rgb: 0,
            flags: cells[i + layout.flags],
        };
        g.x += cells[i + layout.x] * area;
        g.y += cells[i + layout.y] * area;
        g.area += area;
        if (area > g.biggest) {
            g.biggest = area;
            g.rgb = cells[i + layout.rgb];
        }
        groups.set(owner, g);
    }
    const order = [...new Set([...groups.keys()].map((owner) => owner & ~1))].sort((a, b) => a - b);
    const people: Person[] = fromJson(PreviewWorld.identities(SEED, order.length));
    const ranked = [...groups.values()].sort((a, b) => b.area - a.area);
    return ranked.map((g, i): PreviewPlayer => {
        const person = people[order.indexOf(g.owner & ~1)];
        const self = (g.flags & layout.own) !== 0;
        const spot = { x: g.x / g.area, y: g.y / g.area, r: Math.sqrt(g.area) };
        return {
            id: g.owner,
            rank: i + 1,
            name: self ? selfName : person.name,
            mass: Math.round(g.area * width * width * MASS_PER_AREA),
            color: `#${(g.rgb >>> 0).toString(16).padStart(6, "0")}`,
            cell: gridCell(spot.x, spot.y),
            spot,
            self,
        };
    });
}

function gridCell(x: number, y: number): string {
    const at = (v: number) => Math.min(GRID - 1, Math.max(0, Math.floor(v * GRID)));
    return `${ROWS[at(y)]}${at(x) + 1}`;
}
