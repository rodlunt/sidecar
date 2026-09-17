# Security

sidecar is a solo-maintained project. There's no bug bounty, but a genuine vulnerability report
is taken seriously and fixed.

## Reporting a vulnerability

Please don't open a public issue for anything that looks exploitable (a leaked-token path, an
auth bypass, a way to escape the app's intended file/process boundaries). Use one of:

- [Private security advisory](https://github.com/rodlunt/sidecar/security/advisories/new)
  (preferred: only the maintainer can see it)
- Email `rodneylunt79@gmail.com` directly

## Scope notes

The app spawns real shell processes by design (it's a terminal) and stores a GitHub OAuth token
in the OS keychain. Accepted, deliberate trade-offs rather than defects:

- No sandboxing of the terminal/pty itself: the whole point of a terminal on your own machine is
  running commands with your own privileges.
- Device-flow OAuth tokens aren't short-lived by default unless the linked GitHub OAuth App has
  token expiration enabled.

Genuine defects worth reporting: the OAuth token leaking somewhere it shouldn't (logs, error
messages, a file instead of the OS keychain), a path-traversal escape in the file tree, or
credentials surfacing anywhere they're not supposed to.
