# sidecar

A lightweight desktop companion to a terminal you already live in: terminal, file tree, GitHub
commit graph and Issues in a thin sidebar. Not a VS Code replacement.

## TL;DR

```
pnpm install
pnpm tauri dev
```

Implemented: a pty-backed terminal pane that saves its transcript to disk by default (toggle
next to the terminal), a live-updating file tree, a git commit graph with inline PR labels, and
a GitHub Issues panel (device-flow sign-in, keychain token storage). Sidebar sections are an
independent, resizable, reorderable accordion. Chat (Phase 2) isn't built yet.

## Stack

| Layer | Tech |
|---|---|
| Shell | [Tauri 2](https://tauri.app) (Rust core + web frontend) |
| Frontend | TypeScript + Vite, [xterm.js](https://xtermjs.org) |
| Terminal backend | [`portable-pty`](https://crates.io/crates/portable-pty) |
| Git | [`git2`](https://crates.io/crates/git2) |
| GitHub API | [`octocrab`](https://crates.io/crates/octocrab) (device-flow OAuth), [`keyring`](https://crates.io/crates/keyring) (OS keychain token storage) |
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

- `pnpm exec tsc --noEmit`: frontend type-check
- `cd src-tauri && cargo check && cargo test`: Rust backend compile check and unit tests

Every command above is what CI runs (see `.github/workflows/ci.yml`), split into independent
jobs so an audit failure never masks a lint or test failure.

## Docs

| Doc | What's in it |
|---|---|
| [`CLAUDE.md`](CLAUDE.md) | Stack summary, issue discipline, repo conventions |
| [`SECURITY.md`](SECURITY.md) | How to report a security issue |
