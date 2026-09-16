# sidecar

A lightweight desktop companion to a terminal you already live in: terminal, file tree, GitHub
commit graph and Issues in a thin sidebar. Not a VS Code replacement.

## TL;DR

```
pnpm install
pnpm tauri dev
```

Currently implemented: a single pty-backed terminal pane (xterm.js frontend, `portable-pty`
Rust backend). File tree, commit graph, Issues panel and chat are not built yet. See
`~/.claude/plans/golden-floating-rose.md` for the full plan.

## Stack

| Layer | Tech |
|---|---|
| Shell | [Tauri 2](https://tauri.app) (Rust core + web frontend) |
| Frontend | TypeScript + Vite, [xterm.js](https://xtermjs.org) |
| Terminal backend | [`portable-pty`](https://crates.io/crates/portable-pty) |
| Package manager | pnpm (never npm) |

## Development setup

- Rust (stable, via [rustup](https://rustup.rs))
- Node/pnpm
- Linux build deps: `libwebkit2gtk-4.1-dev`, `libjavascriptcoregtk-4.1-dev`,
  `libayatana-appindicator3-dev`, `librsvg2-dev`, `libgtk-3-dev`, `build-essential`

```
pnpm install
pnpm tauri dev
```

## Tests

- `pnpm exec tsc --noEmit` — frontend type-check
- `cd src-tauri && cargo check` — Rust backend compile check

No automated test suite yet — this is pre-MVP scaffolding.
