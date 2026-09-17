# sidecar

Lightweight desktop companion to a terminal: file tree, git graph, GitHub Issues. Not a VS Code
replacement. Chat (a future phase) isn't built yet.

## Stack

Tauri 2 (Rust core, TypeScript/Vite frontend), pnpm. See `README.md` for dev setup.

## Issue Discipline

Every issue carries an **Acceptance criteria** field: two or three testable criteria. "Done"
means the criteria pass, not "finished typing". Epics get split into person-week-sized issues
before work starts.

## Conventions

- Feature branch + PR for anything non-trivial; conventional commit prefixes; never squash-merge.
- Australian English, no em or en dashes.
- Never `npm`, `pnpm` only.

## Gotchas

1. Form controls (`button`, `input`, `textarea`, `select`) don't inherit `font-family`/`font-size`
   from the page by default; the browser's own stylesheet sets them. Without an explicit
   `font-family: inherit; font-size: inherit;` reset, buttons render in the OS default font
   instead of the app's bundled fonts.
2. Running `cargo check`/`cargo test` directly while `pnpm tauri dev`'s file watcher is mid-rebuild
   causes a build-lock collision that crashes the live app. Check the dev log is idle first, or
   trust the dev server's own build output and CI instead of a standalone cargo run.
3. `pnpm tauri dev` produces a raw dev binary with no `.desktop` file, so GNOME can't resolve an
   icon for it and shows a generic default. A dev-mode dock icon needs a manually installed
   `.desktop` entry plus an icon-theme install (`~/.local/share/icons/hicolor/*/apps/`), machine
   local, not part of the repo.
