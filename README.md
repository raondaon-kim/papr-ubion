<div align="center">

<img src="docs/logo.svg" alt="Papr" width="96" height="96" />

# Papr

**A fast, native RSS reader — and a CLI your AI agent can actually drive.**

<img src="docs/screenshot.webp" alt="Papr" width="820" />

</div>

Papr is two front-ends over **one local database**:

- **A desktop reader** — fast, native, offline-first. No account, no cloud.
- **`papr`, an agent-facing CLI** — so an autonomous agent (Claude Code, Codex,
  OpenCode…) can read, search and triage your feeds straight from the shell.
  [Jump to it ↓](#papr--your-feeds-handed-to-your-agent)

Both read the same database through the shared `papr-core` crate, so the app and
the CLI can never drift apart.

---

## The desktop app

- **Feeds & folders** — subscribe, organize, and import/export OPML.
- **Smart views** — All, Unread, Starred, and Read Later, with live counts.
- **Tags & rules** — color-coded tags and rules that tag new articles automatically.
- **Full-text** — fetch and clean the complete article when a feed ships only a summary.
- **AI** — summaries, ask-the-article Q&A, and digests. Bring your own API key.
- **Audio** — a built-in player that follows you from article to article.
- **FreshRSS sync** — keep read state in step with a FreshRSS server.
- **Local-first** — everything stays on your machine. No account, no cloud.
- **Localized** — English, Japanese, and Simplified Chinese.

### Install

| Platform | How |
| --- | --- |
| **macOS** | `brew install --cask l0ng-ai/papr/papr` — or grab the `.dmg` |
| **Windows** | Download the `.msi` installer |
| **Linux** | Download the `.AppImage` or `.deb` |

All packages live on the **[latest release](https://github.com/l0ng-ai/papr/releases/latest)**.
The macOS builds are Developer ID signed and notarized.

---

## `papr` — your feeds, handed to your agent

This is what makes Papr different. `papr` is a command-line companion built **for
autonomous agents** to drive over the shell. Point your agent at it and it can
work your feeds with no GUI:

- **Read** — `feeds`, `list`, `read`, full-text `search`
- **Triage** — `mark` read/star/later, `extract` full text, `refresh`
- **Manage** — subscriptions, folders, tags, rules, highlights, OPML
- **Sync** — FreshRSS / Miniflux

Run bare `papr` and it prints your unread dashboard *plus the next useful
commands*, so the agent orients with zero manual. Output is
[TOON](https://toonformat.dev) — ~40% fewer tokens than JSON — with definitive
counts and structured exit codes.

```console
$ papr
unread: 206   starred: 17   later: 0   feeds: 15
articles[10]{id,feed,title,flags,date}:
  3664,V2EX,[Java] 使用 kkRepo 搭建 Maven 私服,unread.star,"2026-06-25"
  ...
help[4]: Run `papr read <id>` to read an article's full text, ...
```

### Hand it to your agent — one line

The bundled **[`papr-rss` skill](skills/papr-rss/SKILL.md)** loads *on demand*
when an agent recognizes a feed-related task, so it costs nothing until you use
it. Install it with [`skills`](https://github.com/vercel-labs/skills):

```sh
npx skills add https://github.com/l0ng-ai/papr/tree/main/skills/papr-rss
```

Want the agent *proactively* aware of your feeds every conversation? `papr setup`
wires up an ambient SessionStart hook (Claude Code, Codex, OpenCode).

### Install the CLI

| Platform | How |
| --- | --- |
| **macOS / Linux** | `brew install l0ng-ai/papr/papr-cli` |
| **Windows** | Download `papr-x86_64-pc-windows-msvc.zip`, unzip, drop `papr.exe` on your `PATH` |
| **Any** | Prebuilt `papr-<target>.tar.gz` from the [latest release](https://github.com/l0ng-ai/papr/releases/latest), or `cargo build --release -p papr-cli` |

> **Full command reference, agent setup, and install options → [docs/cli.md](docs/cli.md)**

---

## Ubion 경쟁사 모니터링 (GPT OAuth 운영)

이 포크는 Papr를 **Ubion 경쟁사 뉴스 모니터링** 데이터 계층으로 쓴다. 분석은
OpenAI API 키 없이 **ChatGPT 계정 OAuth로 로그인한 Codex CLI**가 맡고, Papr에는
어떤 LLM 자격 증명도 저장하지 않는다.

- **[docs/gpt-oauth-workflow.md](docs/gpt-oauth-workflow.md)** — 운영 방식 규정(역할
  분리·인증·데이터 경계·설치 절차), Codex hook 스키마 대응, 데스크톱 앱 없이
  브라우저에서 실제 기사를 보는 미리보기 모드, 아키텍처 인사이트와 남은 과제
- **[REPOSITORY_ANALYSIS.md](REPOSITORY_ANALYSIS.md)** — 레포지토리 구조·동작·품질 분석
- **[skills/ubion-competitor-monitoring](skills/ubion-competitor-monitoring/SKILL.md)** —
  원티드랩·엘리스그룹·클라썸·프리윌린·플리토 경쟁 인텔리전스 브리프 스킬

```sh
pnpm dev          # http://localhost:1430 — Tauri 없이 브라우저에서 경쟁사 기사 미리보기
codex login       # ChatGPT OAuth — API 키 불필요
papr setup --app codex
```
