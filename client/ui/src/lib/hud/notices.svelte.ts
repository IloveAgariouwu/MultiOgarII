export interface NoticePost {
    key: string;
    name: string;
    text: string;
    open: (() => void) | null;
}

export interface Notice extends NoticePost {
    id: number;
    feed: NoticeFeed;
}

export class NoticeFeed {
    constructor(
        private readonly notices: Notices,
        readonly lifeMs: number,
    ) {}

    post(n: NoticePost): void {
        this.notices.add(this, n);
    }
}

export class Notices {
    items = $state.raw<Notice[]>([]);
    latest = $derived(this.items.at(-1) ?? null);
    private nextId = 1;

    feed(lifeMs: number): NoticeFeed {
        return new NoticeFeed(this, lifeMs);
    }

    add(feed: NoticeFeed, post: NoticePost): void {
        const id = this.nextId++;
        this.items = [...this.items.filter((n) => n.feed !== feed || n.key !== post.key), { ...post, id, feed }];
        window.setTimeout(() => this.drop((n) => n.id === id), feed.lifeMs);
    }

    drop(match: (n: Notice) => boolean): void {
        if (this.items.some(match)) this.items = this.items.filter((n) => !match(n));
    }

    open(notice: Notice): void {
        this.drop((n) => n.id === notice.id);
        notice.open?.();
    }
}
