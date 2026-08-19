# Publishing to the Zed extension marketplace

The server binary is published by `release.yml` on a version tag. **The extension is
not** — Zed extensions ship through a pull request to
[`zed-industries/extensions`](https://github.com/zed-industries/extensions), so
tagging a release leaves the marketplace on the previous version until someone opens
that PR. This file is the checklist for doing it.

## Preflight

All of these are enforced by `tests/zed_extension.rs`, so `just test` covers them:

- `LICENSE` at the repo root **and** committed at `editors/zed/LICENSE`. The validator
  reads the extension *directory*, not the repo root and not the packaged zip — a
  LICENSE present only in the zip passes every local check and then fails submission.
- `editors/zed/extension.toml` declares `repository` (canonical GitHub URL) and
  `authors`.
- The grammar is pinned to a full 40-character SHA, never `HEAD`.
- Every language declared for `jinja2-lsp` has a `language_ids` mapping the server
  accepts, or Zed sends a `languageId` that is silently rejected on every `didOpen`.
- The extension compiles for `wasm32-wasip2` — CI job **Build Zed extension**.

Confirm the version is the one you mean to publish:

```bash
grep '^version' editors/zed/extension.toml   # must match the release being published
```

`extension.toml`, `editors/zed/Cargo.toml` and the root `Cargo.toml` are bumped
together; they are not automatically kept in sync, so check before submitting.

## Submitting

```bash
gh repo fork zed-industries/extensions --clone --remote
cd extensions

git submodule add https://github.com/alex-oleshkevich/jinja-lsp.git extensions/jinja-plus
```

Add the entry to `extensions.toml`:

```toml
[jinja-plus]
submodule = "extensions/jinja-plus"
path = "editors/zed"
version = "0.2.0"
```

`path` is what points the validator at `editors/zed/` rather than the repo root — this
is why the LICENSE has to live there.

```bash
pnpm install && pnpm sort-extensions   # CI enforces alphabetical order; skipping this fails the PR
git add . && git commit -m "Add Jinja Plus extension"
gh pr create --repo zed-industries/extensions --head "alex-oleshkevich:main" --base main
```

## Updating an existing entry

For a version bump, no new submodule is needed — update the pinned commit and the
version:

```bash
cd extensions/jinja-plus && git fetch && git checkout <tag-or-sha> && cd ../..
# then bump `version` in extensions.toml to match editors/zed/extension.toml
```

## Local testing before submitting

```bash
just install-zed   # builds the server + extension and installs both as a dev extension
```

Zed loads a dev extension from the source directory, so this exercises the real
`language_server_command` path — including `worktree.shell_env()` and the
`worktree.which()` fallback — without going through the marketplace.
