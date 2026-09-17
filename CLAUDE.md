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
