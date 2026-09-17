# Contributing

Thanks for wanting to improve sidecar. This is a solo-maintained project, so the process is
deliberately light; the few rules below exist so contributions land smoothly rather than
stalling in back and forth.

## Where things go

- **Bugs**: [open an issue](https://github.com/rodlunt/sidecar/issues/new/choose) using the bug
  report form. Platform, what you did, and what happened instead are the three things that get
  a bug fixed fast.
- **Questions**: [Q&A Discussions](https://github.com/rodlunt/sidecar/discussions/categories/q-a).
- **Ideas and feature requests**: [Ideas Discussions](https://github.com/rodlunt/sidecar/discussions/categories/ideas).
  Raising the idea before writing the code is strongly recommended; it protects you from
  building something that won't merge.
- **Security problems**: never publicly. See [SECURITY.md](SECURITY.md).

## Development setup

The project runs on [pnpm](https://pnpm.io) for the frontend and [Rust](https://rustup.rs)
(stable) for the backend:

```sh
git clone https://github.com/rodlunt/sidecar
cd sidecar
pnpm install
pnpm tauri dev
```

CI gates every pull request on four checks, so run the relevant ones locally before pushing:

```sh
pnpm exec tsc --noEmit
pnpm exec vite build
cd src-tauri && cargo check && cargo test
```

## Pull requests

- Feature branch, conventional-commit prefixes (`feat:`, `fix:`, `chore:`, `docs:`), never
  squash-merge.
- Tests for behaviour changes where the codebase already has a pattern to follow (see
  `src-tauri/src/git.rs` and `src-tauri/src/github.rs` for the Rust unit-test conventions).
- Australian English, no em or en dashes.
