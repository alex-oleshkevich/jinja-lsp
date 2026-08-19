# ADR-011 — Distribution channels: no crates.io, no VS Code, no extension-side binary download

> **Status:** Accepted
>
> **Date:** 2026-08-19
>
> **Supersedes:** —   ·   **Superseded by:** —

## Context

The original release plan ([F21](../features/F21-release-ci.md) REQ-REL-06) named four distribution channels — crates.io, PyPI, the VS Code marketplace, and GitHub releases — and [F20](../features/F20-editor-integrations.md) specified a VS Code extension (REQ-EDIT-03..06) plus a Zed extension that downloads and checksum-verifies the release binary when it isn't on `PATH` (REQ-EDIT-12). Building toward 0.1.0 surfaced three separate problems with that plan.

**crates.io was never publishable.** The tree-sitter Jinja grammar is a git-pinned dependency ([ADR-002](ADR-002-tree-sitter-grammar.md)) and `cargo publish` rejects crates with git dependencies. REQ-REL-12's workaround — publish a vendored grammar crate under our namespace, or wait for an upstream published crate, and swap the pin via `[patch.crates-io]` — meant maintaining a second published crate and an assertion that the pinned revision and the published version name the same commit, in service of a channel whose audience (developers who already have a Rust toolchain) is the smallest of the four. The name `jinja-lsp` on crates.io also already belongs to an unrelated project, so the channel could not have shipped under its own name regardless.

**The VS Code extension was a second product.** A TypeScript language client, an activation-event manifest, a settings-schema mirror of every config key, and a `tmLanguage` grammar are a meaningful maintenance surface — one that must be kept in lockstep with the server's config schema on every change ([E15](../foundations/E15-app-config.md)) and republished on every release. VS Code has a first-class generic LSP path, and the shim added no analysis capability the generic `InitializationOptions` recipe does not already provide.

**The Zed auto-download was a supply-chain surface.** REQ-EDIT-12 had the extension fetch a binary over HTTPS inside Zed's WASM sandbox and verify it against a published checksum before execution. That is fetch-and-execute code running in an environment with no host syscalls, where a verification bug means executing an unverified binary on a developer's machine — the highest-consequence code path in the project, written to serve users who could instead run one install command. It also conflicts with the project's own security posture (P1: the server never executes anything; §13.1: nothing leaves the machine).

## Decision

jinja-lsp does not publish to crates.io. The channel is dropped entirely, along with the grammar-vendoring machinery that existed only to serve it.

jinja-lsp ships no VS Code extension. VS Code users configure the generic stdio recipe like any other LSP client.

No editor extension ever downloads, fetches, or installs the server binary. Every integration requires `jinja-lsp` to be **preinstalled** and discoverable on `PATH` (or pointed at explicitly by the user's own binary-path setting). When the binary is missing, the integration fails with a message naming the install channels and the releases page — it never repairs the situation itself.

The four distribution channels are **GitHub releases**, **PyPI** (maturin wheels — [ADR-010](ADR-010-pypi-distribution.md)), **AUR**, and the **Zed extension marketplace** (which ships the extension only, never the binary).

## Consequences

The release pipeline loses its most failure-prone stage. There is no vendored grammar crate to maintain, no same-commit assertion between a git pin and a published version, no `[patch.crates-io]` divergence between local and published builds, and no long-lived crates.io token — leaving OIDC trusted publishing (PyPI) and an SSH deploy key (AUR) as the only publish credentials.

Every editor integration collapses to the same three lines: find the binary, launch `jinja-lsp lsp`, forward `InitializationOptions`. There is no download path to test, no checksum verification to get right, and no network operation anywhere in the product — the "nothing leaves the machine" claim now holds without an exception clause. The Zed extension is 73 lines of boilerplate rather than a fetch-verify-execute pipeline.

The cost is onboarding friction: the binary is now always a separate, explicit install step, and a user who installs the Zed extension from the marketplace and nothing else gets an error instead of a working server. This is mitigated by making the not-found message name the install channels and the releases page verbatim, at the point of failure. Rust developers lose `cargo install jinja-lsp` and must use `uv tool install jinja-lsp`, the AUR package, or a release download. VS Code users lose the GUI settings panel and must hand-write `InitializationOptions` or a `jinja.toml` — the config file path, which works identically in every editor.

Retired by this decision: **REQ-EDIT-03**, **REQ-EDIT-04**, **REQ-EDIT-05**, **REQ-EDIT-06** (VS Code extension), **REQ-EDIT-12** (Zed binary download), and **REQ-REL-12** (crates.io grammar vendoring). **REQ-REL-06**'s channel table is rewritten. These numbers are retired, not reused.

## Alternatives considered

| Alternative | Why not chosen |
|---|---|
| Keep crates.io, publish a vendored `tree-sitter-jinja` crate under our namespace | A second published crate to version, release, and keep commit-identical with the git pin — permanent maintenance for the channel with the smallest audience and a name we don't own. |
| Keep crates.io, wait for the upstream grammar to publish | Makes our release cadence depend on an upstream we don't control, for a channel we're dropping anyway. |
| Keep the VS Code extension, drop only its settings UI | The language client, activation manifest, and marketplace publish are the bulk of the cost; halving the feature keeps all the overhead. |
| Keep the Zed download but verify the attestation instead of the digest | Still fetch-and-execute in a WASM sandbox; a stronger check does not remove the class of bug, only narrows it. |
| Auto-download in every extension, uniformly | Multiplies the highest-consequence code path across editors instead of removing it. |

## Changelog

- **2026-08-19** — Created. Records the removal of the VS Code extension, crates.io publishing, and the Zed extension's binary auto-download, all of which had already been removed from the codebase (see `CHANGELOG.md`) ahead of the specs being reconciled.
