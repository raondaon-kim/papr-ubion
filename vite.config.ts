import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// A browser-only preview has no Tauri database or RSS worker. This local
// endpoint lets the Vite server retrieve a small, read-only set of the
// competitors' current Google News RSS results so the UI can be reviewed with
// meaningful data before the desktop application is started.
const competitorFeeds = [
  { id: 1, title: "원티드랩", query: "원티드랩" },
  { id: 2, title: "엘리스그룹", query: "엘리스그룹" },
  { id: 3, title: "클라썸", query: "클라썸" },
  { id: 4, title: "프리윌린", query: "프리윌린" },
  { id: 5, title: "플리토", query: "플리토" },
];

let cachedNews: unknown;
let cachedAt = 0;
const CACHE_MS = 5 * 60 * 1000;

function decodeEntities(value: string): string {
  return value
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&");
}

// Google News escapes the HTML inside <description> as XML entities, so the
// entities must be decoded *before* the tags can be stripped, and a second
// decode turns the inner `&amp;` into readable text.
function xmlText(value: string | undefined): string {
  if (!value) return "";
  const raw = value.replace(/^<!\[CDATA\[([\s\S]*)\]\]>$/, "$1");
  return decodeEntities(decodeEntities(raw).replace(/<[^>]+>/g, " "))
    .replace(/\s+/g, " ")
    .trim();
}

function itemField(item: string, name: string): string {
  const match = item.match(new RegExp(`<${name}(?:\\s[^>]*)?>([\\s\\S]*?)</${name}>`, "i"));
  return xmlText(match?.[1]);
}

function parseNewsFeed(xml: string, feed: (typeof competitorFeeds)[number]) {
  const items = xml.match(/<item>[\s\S]*?<\/item>/gi) ?? [];
  return items.slice(0, 20).flatMap((item, index) => {
    const rawTitle = itemField(item, "title");
    const url = itemField(item, "link");
    if (!rawTitle || !url) return [];
    const source = itemField(item, "source");
    // Google News appends " - <source>" to every headline; the source already
    // appears in the author column, so keep the headline itself.
    const title =
      source && rawTitle.endsWith(` - ${source}`)
        ? rawTitle.slice(0, -(source.length + 3)).trim()
        : rawTitle;
    // The description is just the linked headline plus the source name, which
    // would duplicate the title in the list — only keep it when it says more.
    const description = itemField(item, "description");
    const snippet =
      description && !description.startsWith(rawTitle) && !description.startsWith(title)
        ? description.slice(0, 360)
        : null;
    const published = Date.parse(itemField(item, "pubDate"));
    return [{
      id: feed.id * 1_000 + index,
      feedId: feed.id,
      feedTitle: feed.title,
      sourceType: "rss",
      title,
      author: source || "Google News",
      snippet,
      imageUrl: null,
      url,
      publishedAt: Number.isNaN(published) ? null : new Date(published).toISOString(),
      isRead: false,
      isStarred: false,
      readLater: false,
    }];
  });
}

async function loadCompetitorNews(force = false) {
  if (!force && cachedNews && Date.now() - cachedAt < CACHE_MS) return cachedNews;

  const results = await Promise.all(
    competitorFeeds.map(async (feed) => {
      const params = new URLSearchParams({
        q: feed.query,
        hl: "ko",
        gl: "KR",
        ceid: "KR:ko",
      });
      const response = await fetch(`https://news.google.com/rss/search?${params}`, {
        headers: { "User-Agent": "Papr-local-preview/0.15" },
      });
      if (!response.ok) throw new Error(`${feed.title} news request failed: ${response.status}`);
      return parseNewsFeed(await response.text(), feed);
    }),
  );

  const feeds = competitorFeeds.map((feed) => ({
    id: feed.id,
    feedUrl: `https://news.google.com/rss/search?q=${encodeURIComponent(feed.query)}&hl=ko&gl=KR&ceid=KR:ko`,
    siteUrl: "https://news.google.com/",
    title: feed.title,
    description: `${feed.title} 관련 Google News RSS 미리보기`,
    faviconUrl: null,
    folderId: null,
    sourceType: "rss",
    lastFetchedAt: new Date().toISOString(),
    fetchError: null,
    unreadCount: results[feed.id - 1].length,
    refreshIntervalMin: null,
    autoTranslate: false,
    openMode: null,
  }));
  const articles = results.flat().sort((a, b) =>
    (b.publishedAt ?? "").localeCompare(a.publishedAt ?? ""),
  );
  cachedNews = { feeds, articles, refreshedAt: new Date().toISOString() };
  cachedAt = Date.now();
  return cachedNews;
}

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [
    react(),
    {
      name: "competitor-news-preview",
      configureServer(server) {
        server.middlewares.use("/api/competitor-news", async (req, res) => {
          try {
            const data = await loadCompetitorNews(req.url?.includes("refresh=1"));
            res.statusCode = 200;
            res.setHeader("Content-Type", "application/json; charset=utf-8");
            res.end(JSON.stringify(data));
          } catch (error) {
            res.statusCode = 502;
            res.setHeader("Content-Type", "application/json; charset=utf-8");
            res.end(JSON.stringify({ error: String(error) }));
          }
        });
      },
    },
  ],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1430,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1431,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
