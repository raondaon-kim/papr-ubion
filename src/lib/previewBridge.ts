// Browser-preview stand-in for the Tauri command surface.
//
// `pnpm dev` can render the UI in an ordinary browser, where there is no
// SQLite database, RSS worker or IPC bridge. The Vite dev server exposes a
// read-only `/api/competitor-news` endpoint (see vite.config.ts) with the
// competitors' current Google News results; this module answers the same
// command names `src/api.ts` sends to Tauri using that data, so the shell can
// be reviewed with real articles. Read / star / read-later flags live in
// memory only and reset on reload. Anything needing the native backend
// (extraction, AI, sync, OPML …) rejects with a clear message.

import type {
  ArticleDetail,
  ArticleQuery,
  ArticleSummary,
  Feed,
  SmartCounts,
} from "../types";

interface PreviewPayload {
  feeds: Feed[];
  articles: ArticleSummary[];
  refreshedAt: string;
}

let payload: Promise<PreviewPayload> | null = null;
const settings = new Map<string, string>();

async function fetchPayload(refresh = false): Promise<PreviewPayload> {
  const res = await fetch(`/api/competitor-news${refresh ? "?refresh=1" : ""}`);
  if (!res.ok) {
    let detail = `${res.status}`;
    try {
      detail = ((await res.json()) as { error?: string }).error ?? detail;
    } catch {
      /* body was not JSON */
    }
    throw new Error(`competitor news preview unavailable: ${detail}`);
  }
  return (await res.json()) as PreviewPayload;
}

function load(): Promise<PreviewPayload> {
  if (!payload) {
    payload = fetchPayload().catch((e) => {
      payload = null; // let the next query retry instead of caching the failure
      throw e;
    });
  }
  return payload;
}

const key = (a: ArticleSummary) => a.url ?? String(a.id);

/** Re-fetch, keeping per-article flags the reviewer toggled in this session. */
async function refresh(): Promise<number> {
  const previous = payload ? await payload.catch(() => null) : null;
  const next = await fetchPayload(true);
  if (previous) {
    const flags = new Map(previous.articles.map((a) => [key(a), a]));
    for (const a of next.articles) {
      const old = flags.get(key(a));
      if (old) {
        a.isRead = old.isRead;
        a.isStarred = old.isStarred;
        a.readLater = old.readLater;
      }
    }
  }
  payload = Promise.resolve(next);
  const seen = new Set(previous?.articles.map(key));
  return next.articles.filter((a) => !seen.has(key(a))).length;
}

function matches(a: ArticleSummary, q: ArticleQuery): boolean {
  switch (q.kind) {
    case "all":
      return true;
    case "unread":
      return !a.isRead;
    case "starred":
      return a.isStarred;
    case "readLater":
      return a.readLater;
    case "feed":
      return a.feedId === q.value;
    case "folder":
    case "tag":
      return false;
  }
}

function select(
  data: PreviewPayload,
  q: ArticleQuery,
  unreadOnly: boolean,
  search: string | null,
  oldestFirst: boolean,
): ArticleSummary[] {
  const needle = search?.trim().toLowerCase() ?? "";
  const rows = data.articles.filter(
    (a) =>
      matches(a, q) &&
      (!unreadOnly || !a.isRead) &&
      (!needle ||
        a.title.toLowerCase().includes(needle) ||
        (a.snippet ?? "").toLowerCase().includes(needle) ||
        (a.author ?? "").toLowerCase().includes(needle)),
  );
  // The server already sorts newest-first.
  return oldestFirst ? [...rows].reverse() : rows;
}

function feedsWithCounts(data: PreviewPayload): Feed[] {
  return data.feeds.map((f) => ({
    ...f,
    unreadCount: data.articles.filter((a) => a.feedId === f.id && !a.isRead).length,
  }));
}

function detail(a: ArticleSummary): ArticleDetail {
  const esc = (s: string) =>
    s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
  const body = [
    a.snippet ? `<p>${esc(a.snippet)}</p>` : "",
    a.url
      ? `<p><a href="${esc(a.url)}" target="_blank" rel="noreferrer">원문 보기 (Google News)</a></p>`
      : "",
    `<p><em>브라우저 미리보기 모드 — 본문 추출, AI 요약, 번역은 데스크톱 앱에서만 동작합니다.</em></p>`,
  ].join("");
  return {
    id: a.id,
    feedId: a.feedId,
    feedTitle: a.feedTitle,
    sourceType: a.sourceType,
    title: a.title,
    author: a.author,
    url: a.url,
    contentHtml: body,
    extractedHtml: null,
    imageUrl: a.imageUrl,
    publishedAt: a.publishedAt,
    isRead: a.isRead,
    isStarred: a.isStarred,
    readLater: a.readLater,
    aiSummary: null,
    translatedHtml: null,
    translatedLang: null,
    enclosures: [],
    tags: [],
  };
}

type Args = Record<string, unknown>;

const unsupported = (cmd: string) =>
  Promise.reject(
    new Error(`"${cmd}" is not available in the browser preview — run the desktop app.`),
  );

/** Commands whose effect we don't track: succeed silently so UI flows continue. */
const NOOP = new Set([
  "refresh_tray",
  "apply_network_settings",
  "set_native_backing",
  "open_page_view",
  "set_page_view_bounds",
  "set_page_view_visible",
  "close_page_view",
  "set_feed_refresh_interval",
  "set_feed_auto_translate",
  "set_feed_open_mode",
  "rename_feed",
  "move_feed",
]);

export async function previewInvoke<T>(cmd: string, args: Args = {}): Promise<T> {
  const r = (v: unknown) => v as T;
  switch (cmd) {
    // ── read side ──
    case "list_folders":
    case "list_tags":
    case "list_rules":
    case "list_all_highlights":
    case "list_highlights":
    case "list_newsletter_sources":
      return r([]);
    case "list_feeds":
      return r(feedsWithCounts(await load()));
    case "list_articles": {
      const data = await load();
      const rows = select(
        data,
        args.query as ArticleQuery,
        Boolean(args.unreadOnly),
        (args.search as string | null) ?? null,
        Boolean(args.oldestFirst),
      );
      const offset = Number(args.offset ?? 0);
      const limit = Number(args.limit ?? rows.length);
      return r(rows.slice(offset, offset + limit));
    }
    case "article_index": {
      const data = await load();
      const rows = select(
        data,
        args.query as ArticleQuery,
        Boolean(args.unreadOnly),
        null,
        Boolean(args.oldestFirst),
      );
      const i = rows.findIndex((a) => a.id === args.articleId);
      return r(i < 0 ? null : i);
    }
    case "get_article": {
      const data = await load();
      const a = data.articles.find((x) => x.id === args.id);
      if (!a) throw new Error(`article ${String(args.id)} not found`);
      return r(detail(a));
    }
    case "smart_counts": {
      const data = await load();
      const counts: SmartCounts = {
        unread: data.articles.filter((a) => !a.isRead).length,
        starred: data.articles.filter((a) => a.isStarred).length,
        readLater: data.articles.filter((a) => a.readLater).length,
      };
      return r(counts);
    }
    case "get_setting":
      return r(settings.get(String(args.key)) ?? null);
    case "storage_stats": {
      const data = await load();
      return r({ dbBytes: 0, articleCount: data.articles.length, feedCount: data.feeds.length });
    }
    case "freshrss_status":
      return r({ connected: false, url: null, provider: "freshrss" });
    case "take_pending_deep_link":
      return r(null);

    // ── in-memory mutations ──
    case "mark_read":
    case "mark_starred":
    case "mark_read_later": {
      const data = await load();
      const a = data.articles.find((x) => x.id === args.id);
      if (a) {
        if (cmd === "mark_read") a.isRead = Boolean(args.read);
        if (cmd === "mark_starred") a.isStarred = Boolean(args.starred);
        if (cmd === "mark_read_later") a.readLater = Boolean(args.value);
      }
      return r(undefined);
    }
    case "mark_all_read": {
      const data = await load();
      let n = 0;
      for (const a of data.articles) {
        if (matches(a, args.query as ArticleQuery) && !a.isRead) {
          a.isRead = true;
          n++;
        }
      }
      return r(n);
    }
    case "set_setting":
      settings.set(String(args.key), String(args.value));
      return r(undefined);
    case "reset_settings":
      settings.clear();
      return r(undefined);
    case "refresh_feeds":
      return r(await refresh());

    default:
      if (NOOP.has(cmd)) return r(undefined);
      return unsupported(cmd);
  }
}
