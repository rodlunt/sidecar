# Security

This is a private, solo-maintained project. There is no public bug bounty or disclosure process.

## Reporting a vulnerability

Email rodneylunt79@gmail.com directly. Don't open a public issue for anything that looks
exploitable (a leaked-token path, an auth bypass, a way to escape the app's intended file/process
boundaries).

## Scope notes

The app spawns real shell processes by design (it's a terminal) and stores a GitHub OAuth token
locally. See the plan's "Security" and "Threat model" sections for what's already an accepted,
deliberate trade-off (no sandboxing of the terminal itself) versus what's a genuine defect worth
reporting (the token leaking somewhere it shouldn't, a path-traversal escape, credentials
surfacing in logs).
