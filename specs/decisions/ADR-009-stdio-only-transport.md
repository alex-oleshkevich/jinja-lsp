# ADR-009 — stdio-only transport

> **Status:** Accepted
>
> **Date:** 2026-06-24
>
> **Supersedes:** —   ·   **Superseded by:** —

## Context

An LSP server can speak over stdio or over a TCP listener. In practice every editor integration — VS Code, Zed, Neovim, and generic LSP clients ([F20](../features/F20-editor-integrations.md)) — launches the server as a subprocess and speaks LSP over its stdin/stdout. A second TCP transport would be a code path no integration actually uses, and a network listener carries real costs: it's an attack surface (a port anything on the machine can connect to), it complicates the security posture (P1's "never executes" is cleaner when there's no network at all), and it doubles the transport code to test and maintain for no benefit.

## Decision

The server supports stdio as its only transport. `jinja-lsp lsp` reads LSP over stdin and writes over stdout, full stop.

## Consequences

The server opens no network listener, ever — which makes the security story trivially clean (no port, no network attack surface, nothing to firewall — §13.1 across the suite) and reinforces P2 (editor-agnostic over a standard transport). There's one transport code path to test and one way every editor connects, so integrations are uniform ([F20](../features/F20-editor-integrations.md)). The one thing we give up is the ability to attach a remote LSP client over a socket — a workflow no integration used and that, if it ever genuinely arose, would be better served by a deliberate, security-reviewed addition than by leaving a listener on by default. Editors that downloaded the binary fetch it over HTTPS from a GitHub release ([F21](../features/F21-release-ci.md)); that's a client-side download, not a server network surface.

## Alternatives considered

| Alternative | Why not chosen |
|---|---|
| Add a second TCP transport | A transport no integration would use; added attack surface and double the code to maintain for no benefit. |
| TCP only | Every editor launches a subprocess and expects stdio; a network-only server would fit nothing. |
| Add a Unix-socket transport | Same "unused second path" problem as TCP; stdio already covers the subprocess model every client uses. |

## Changelog

- **2026-06-24** — Created.
