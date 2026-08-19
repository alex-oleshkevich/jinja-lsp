# Agent Instructions

## 0. Non-negotiables

These rules override everything else in this file when in conflict:

1. **No flattery, no filler.** Skip openers like "Great question", "You're absolutely right", "Excellent idea", "I'd be happy to". Start with the answer or the action.
2. **Disagree when you disagree.** If the user's premise is wrong, say so before doing the work. Agreeing with false premises to be polite is the single worst failure mode in coding agents.
3. **Never fabricate.** Not file paths, not commit hashes, not API names, not test results, not library functions. If you don't know, read the file, run the command, or say "I don't know, let me check."
4. **Stop when confused.** If the task has two plausible interpretations, ask. Do not pick silently and proceed.
5. **Touch only what you must.** Every changed line must trace directly to the user's request. No drive-by refactors, reformatting, or "while I was in there" cleanups.

---

## 1. Before writing code

**Goal: understand the problem and the codebase before producing a diff.**

- State your plan in one or two sentences before editing. For anything non-trivial, produce a numbered list of steps with a verification check for each.
- Read the files you will touch. Read the files that call the files you will touch. Claude Code: use subagents for exploration so the main context stays clean.
- Match existing patterns in the codebase. If the project uses pattern X, use pattern X, even if you'd do it differently in a greenfield repo.
- Surface assumptions out loud: "I'm assuming you want X, Y, Z. If that's wrong, say so." Do not bury assumptions inside the implementation.
- If two approaches exist, present both with tradeoffs. Do not pick one silently. Exception: trivial tasks (typo, rename, log line) where the diff fits in one sentence.

---

## 2. Writing code: simplicity first

**Goal: the minimum code that solves the stated problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code. No configurability, flexibility, or hooks that were not requested.
- No error handling for impossible scenarios. Handle the failures that can actually happen.
- If the solution runs 200 lines and could be 50, rewrite it before showing it.
- If you find yourself adding "for future extensibility", stop. Future extensibility is a future decision.
- Bias toward deleting code over adding code. Shipping less is almost always better.

The test: would a senior engineer reading the diff call this overcomplicated? If yes, simplify.

---

## 3. Surgical changes

**Goal: clean, reviewable diffs. Change only what the request requires.**

- Do not "improve" adjacent code, comments, formatting, or imports that are not part of the task.
- Do not refactor code that works just because you are in the file.
- Do not delete pre-existing dead code unless asked. If you notice it, mention it in the summary.
- Do clean up orphans created by your own changes (unused imports, variables, functions your edit made obsolete).
- Match the project's existing style exactly: indentation, quotes, naming, file layout.

The test: every changed line traces directly to the user's request. If a line fails that test, revert it.

---

## 4. Goal-driven execution

**Goal: define success as something you can verify, then loop until verified.**

Rewrite vague asks into verifiable goals before starting:

- "Add validation" becomes "Write tests for invalid inputs (empty, malformed, oversized), then make them pass."
- "Fix the bug" becomes "Write a failing test that reproduces the reported symptom, then make it pass."
- "Refactor X" becomes "Ensure the existing test suite passes before and after, and no public API changes."
- "Make it faster" becomes "Benchmark the current hot path, identify the bottleneck with profiling, change it, show the benchmark is faster."

For every task:

1. State the success criteria before writing code.
2. Write the verification (test, script, benchmark, screenshot diff) where practical.
3. Run the verification. Read the output. Do not claim success without checking.
4. If the verification fails, fix the cause, not the test.

---

## 5. Tool use and verification

- Prefer running the code to guessing about the code. If a test suite exists, run it. If a linter exists, run it. If a type checker exists, run it.
- Never report "done" based on a plausible-looking diff alone. Plausibility is not correctness.
- When debugging, address root causes, not symptoms. Suppressing the error is not fixing the error.
- For UI changes, verify visually: screenshot before, screenshot after, describe the diff.
- Use CLI tools (gh, aws, gcloud, kubectl) when they exist. They are more context-efficient than reading docs or hitting APIs unauthenticated.
- When reading logs, errors, or stack traces, read the whole thing. Half-read traces produce wrong fixes.

---

## 6. Session hygiene

- Context is the constraint. Long sessions with accumulated failed attempts perform worse than fresh sessions with a better prompt.
- After two failed corrections on the same issue, stop. Summarize what you learned and ask the user to reset the session with a sharper prompt.
- Use subagents (Claude Code: "use subagents to investigate X") for exploration tasks that would otherwise pollute the main context with dozens of file reads.
- When committing, write descriptive commit messages (subject under 72 chars, body explains the why). No "update file" or "fix bug" commits. No "Co-Authored-By: Claude" attribution unless the project explicitly wants it.

---

## 7. Communication style

- Direct, not diplomatic. "This won't scale because X" beats "That's an interesting approach, but have you considered...".
- Concise by default. Two or three short paragraphs unless the user asks for depth. No padding, no restating the question, no ceremonial closings.
- When a question has a clear answer, give it. When it does not, say so and give your best read on the tradeoffs.
- Celebrate only what matters: shipping, solving genuinely hard problems, metrics that moved. Not feature ideas, not scope creep, not "wouldn't it be cool if".
- No excessive bullet points, no unprompted headers, no emoji. Prose is usually clearer than structure for short answers.

---

## 8. When to ask, when to proceed

**Ask before proceeding when:**
- The request has two plausible interpretations and the choice materially affects the output.
- The change touches something you've been told is load-bearing, versioned, or has a migration path.
- You need a credential, a secret, or a production resource you don't have access to.
- The user's stated goal and the literal request appear to conflict.

**Proceed without asking when:**
- The task is trivial and reversible (typo, rename a local variable, add a log line).
- The ambiguity can be resolved by reading the code or running a command.
- The user has already answered the question once in this session.

---

## 9. Self-improvement loop

**This file is living. Keep it short by keeping it honest.**

After every session where the agent did something wrong:

1. Ask: was the mistake because this file lacks a rule, or because the agent ignored a rule?
2. If lacking: add the rule under "Project Learnings" below, written as concretely as possible ("Always use X for Y" not "be careful with Y").
3. If ignored: the rule may be too long, too vague, or buried. Tighten it or move it up.
4. Every few weeks, prune. For each line, ask: "Would removing this cause the agent to make a mistake?" If no, delete. Bloated AGENTS.md files get ignored wholesale.

Boris Cherny (creator of Claude Code) keeps his team's file around 100 lines. Under 300 is a good ceiling. Over 500 and you are fighting your own config.

---

## 10. Project context

### What this is
`jinja-lsp` — a **specialist** language server for Jinja2 templates. One Rust binary, three front-ends
(`lsp`, `check`, `format`) over one shared analysis pipeline. Static analysis only: it never imports,
renders, or executes templates or host Python.

**Companion principle (non-negotiable):** it runs *alongside* the host Python/HTML LSPs and owns only
the Jinja layer. Features fire inside Jinja constructs and stay silent everywhere else — no generic
completions, no hover on unknown symbols, never diagnose what can't be positively resolved.

### Stack
| | |
|---|---|
| Language | Rust, edition 2024, MSRV 1.85 (`rust-version` in Cargo.toml) |
| Protocol | `tower-lsp` 0.20 — stdio transport only (ADR-009), no TCP/socket |
| Parsing | `tree-sitter` 0.26 + `tree-sitter-jinja` / `tree-sitter-jinja-inline` (git dep, pinned rev) |
| Async | `tokio` (full); CPU-bound indexing under `spawn_blocking` |
| State | `Arc<RwLock<ServerState>>` (tokio RwLock) — **not** DashMap |
| CLI | `clap` 4 derive; `owo-colors` + `similar` for rich/diff output |
| Config | `toml` + `serde`; `serde_yaml` for hint files |
| Logging | `tracing` → **stderr only** (stdout carries JSON-RPC) |
| Tests | `cargo nextest`, `tempfile`, `pytest-lsp`/`lsprotocol` (Python e2e) |
| Distribution | GitHub Releases, PyPI wheels (maturin), AUR (`jinja-lsp-plus-bin`), Zed ext (`jinja-plus`) |

### Commands

**Use the `Justfile` — it is the entry point for every routine task.** `just --list` shows
the full set. Prefer a recipe over a raw `cargo` invocation so the flags stay in one place
and match CI; add a recipe rather than documenting a bare command here.

```bash
just              # list all recipes
just build
just test         # cargo nextest run — the runner CI uses (1169 tests, ~1s after build)
just test-e2e     # Python LSP-protocol e2e (35 tests); builds the debug binary first
just lint         # clippy --all-targets -D warnings
just fmt
just check        # the CI gate set: fmt --check + clippy + nextest
just check-all    # check + the Python e2e gate
just notes        # release notes for commits since the last tag (read-only)
just release X.Y.Z # verify gates, then tag locally; pushing the tag is left to you
just install-zed  # build server + Zed extension, install both locally
```

Not worth a recipe (one-offs):

```bash
cargo nextest run -E 'test(NAME)'    # single test;  cargo test --test <file> also works
UPDATE_FIXTURES=1 cargo nextest run  # regenerate golden expected-diagnostics/formatter fixtures
cargo run -- check tests/fixtures/starlette-blog/templates --format json
```

### Verification gates (all must pass before a task is done)
1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo nextest run` — includes the golden-diagnostics and structural gates below
4. `cd tests/e2e && uv run pytest -q` — protocol behavior against the real stdio binary
5. `cargo update --locked --workspace` must not change `Cargo.lock` (CI gate REQ-REL-11)

CI (`.github/workflows/ci.yml`) runs 1–3 on Linux/macOS/Windows and 4 on Linux.
Release (`.github/workflows/release.yml`) is tag-triggered and gates on tag == `Cargo.toml`
version **and** a dated `CHANGELOG.md` section for that version.

### Layout
```
src/main.rs           clap CLI dispatch only — routes to linter/, format/cli.rs, doctor.rs, server/
src/linter/           the `check` front-end: orchestration + rich/compact/json output
src/doctor.rs         the `doctor` front-end: config/template/builtin discovery report
src/server/           tower-lsp backend (mod.rs) + ServerState (state.rs)
src/config.rs         jinja.toml / [tool.jinja] discovery, zero-config, InitializationOptions overlay
src/parsing/          tree-sitter wrapper, extractor, inline-template detection, queries/*.scm (17)
src/workspace/        TemplateIndex, WorkspaceIndex, symbols, builder (Pass 2)
src/diagnostics/      DiagCode enum, checks/ (one module per code), noqa, select/ignore filter
src/builtins/         doc registry, docs/ (113 embedded .md), framework packs, user hints
src/features/         one pure-function module per LSP capability (top layer)
src/format/           the Jinja-only formatter engine (+ cli.rs, the `format` front-end)
src/edit/             shared TextEdit/WorkspaceEdit builders
tests/*.rs            55 integration crates; tests/fixtures/ golden corpora; tests/e2e/ pytest-lsp
specs/                the source of truth — index.md, foundations E##, features F##, decisions ADR-###
editors/zed/          Zed WASM extension (id jinja-plus, 24 Jinja2 language variants)
aur/ scripts/         PKGBUILD; Zed install/package helpers
```
Do not edit: `editors/zed/grammars/` (vendored upstream grammar clone), `target/`.

### Architecture
- **Two-pass pipeline.** Pass 1 (`ServerState::update_file`) reparses **one** file and atomically
  replaces its `TemplateIndex`; it bumps a `generation` counter. Pass 2 (`workspace::build_workspace`)
  relinks the workspace for cross-file facts; it is guarded by the generation counter so a stale
  relink never overwrites a newer one. Both run under `spawn_blocking`.
- **Feature handlers are pure reads** — `(index snapshot, params) → response`. They never parse,
  never mutate shared state, never block on Pass 2. Parsing happens in Pass 1 only.
- **One engine, three front-ends.** `check` and the LSP publish path share `run_checks` +
  `suppress_by_noqa` + `filter_by_config`, so CLI and editor can never disagree.
- **Position encoding** is negotiated at `initialize` (UTF-8 preferred, UTF-16 fallback) and stored
  as `state.position_encoding_utf8`; all offset conversion goes through that flag.
- **Watched files**: config and hint files are detected *before* the template branch in
  `did_change_watched_files` and never fed to Pass 1.

### Diagnostics
21 codes, `JINJA-<SEV><CLASS><NN>` (ADR-003): `E`=error `W`=warning `I`=info `H`=hint. Severity is
derived from the code string. The `DiagCode` enum in `src/diagnostics/mod.rs` (with `DiagCode::ALL`)
is the single source of truth — noqa's known-code list derives from it. Every diagnostic carries a
stable kebab-case `slug`.
- When a template has syntax errors, **only `E001` fires** — all other checks are suppressed to avoid
  a false-positive cascade.
- Suppression forms: `{# noqa #}`, `{# noqa: CODE, CLASS #}`, `{# noqa-file #}`, `{# noqa-file: CODE #}`.
  An unknown id emits `W107`. Suppression and select/ignore filtering happen inside the shared compute
  path (`publish_file_diagnostics` / the `check` runner), never as a publish-time-only filter.
- Diagnostics are **push-only** (`textDocument/publishDiagnostics`); there is no pull-mode
  `diagnosticProvider`. A cleared finding must be published as an explicit empty vector.
- Adding a code: add the `DiagCode` variant + `ALL` entry, write the check, add
  `tests/fixtures/corpus/<code>/` with `expected-diagnostics.json`, update `specs/features/F01`.

### Configuration
Discovery walks up for `jinja.toml`, then `pyproject.toml` `[tool.jinja]`. Zero-config falls back to
`templates/`, `<project-name>/templates/`, `jinja/`, `j2/`. The editor's `InitializationOptions`
overlay is applied on top of `base_config` (the pre-overlay file config) so clearing an editor
setting correctly falls back instead of leaving a stale value. Keys: `templates`, `extensions`,
`extras` (flask, starlette, starlette-babel, starlette-flash), `hints`, `custom_builtins`,
`inline_patterns`, `[lint] select/ignore`, `[format]`.

### Testing conventions
- **Golden diagnostics** (`tests/e2e_branch_a.rs`): each `tests/fixtures/corpus/<code>/` and scenario
  fixture carries `expected-diagnostics.json`, diffed against `check --format json`. Every new
  diagnostic needs one. Golden formatter fixtures live in `tests/fixtures/formatter/*.input/.expected`.
- **Structural gates**: `tests/architecture.rs` (Pass 1 isolation, generation counter, stale-publish
  guard), `tests/conventions.rs` (no bare `.unwrap()` in named modules, parse recovery, no panics on
  adversarial input), `tests/zed_extension.rs` and `tests/release_ci.rs` (extension/workflow
  invariants), `tests/performance.rs` (500-template rebuild under 2s).
- **Python e2e** (`tests/e2e/tests/`): drives the real stdio binary via `pytest-lsp` — protocol
  conformance and user journeys. Never hand-roll a JSON-RPC client. `JINJA_LSP_BINARY` overrides the
  binary path (CI points it at the release build).
- Tests reference requirement tags (`REQ-ARCH-03`, `REQ-DIAG-04`, …) and bead ids in comments — keep
  that habit so a failing test names the spec clause it defends.
- Golden comparisons use checked-in fixture files, never a snapshot library (REQ-STACK-05); `insta` was dropped as unused.

### Conventions specific to this repo
- Errors: `ConfigError` (`src/config.rs`), `PackError` (`src/builtins/packs.rs`), `SyntaxError`
  (`src/workspace/symbols.rs`). There is no `src/error.rs`.
- `Span` (`src/workspace/symbols.rs`) is byte range + line/col; diagnostics report 1-based line, col.
- Query files: `src/parsing/queries/*.scm`, one per construct, loaded via `include_str!`.
- Downward-dependency rule (REQ-FOLD-08): `features/` is the top layer; `parsing/`, `workspace/`,
  `diagnostics/` must never import from it.
- tree-sitter 0.26 `QueryMatches` is a `StreamingIterator` — `use tree_sitter::StreamingIterator` and
  `while let Some(m) = matches.next()`.
- Spec-first: behavior changes update the owning `F##`/`E##` spec in the same change; decisions get an
  ADR (append-only — supersede, never edit).

### Forbidden
- `println!` / anything on stdout outside JSON-RPC — it breaks the protocol. Log via `tracing`.
- Importing `crate::features` from `parsing/`, `workspace/`, or `diagnostics/` (REQ-FOLD-08).
- Re-parsing inside a feature handler, or mutating shared state from one.
- Emitting a diagnostic for anything not *positively* resolvable — silence beats a false positive.
- Auto-downloading the server binary from the Zed extension, or `std::env::var` inside the WASM
  extension (use `worktree.shell_env()` / `worktree.which()`).
- Using `cat`, `head`, `tail`, `sed`, `awk`, or heredocs instead of the Read/Edit/Write tools.

---

## 11. Project Learnings

- Always use `while let Some(m) = matches.next()` for tree-sitter QueryMatches — `flat_map` / `Iterator` trait methods do not work with tree-sitter 0.26's StreamingIterator.
- The upstream grammar exposes `language()`, not `LANGUAGE_JINJA` — use `tree_sitter_jinja::language()`.
- The inline grammar (`tree-sitter-jinja-inline`) uses `# statement` notation, not Jinja delimiters; test inputs must be `"# set x = 1"` not `"{{ x }}"`.
- `blocks.scm` has no `scoped` capture because the upstream grammar does not model the `scoped` modifier; `BlockDefinition.scoped` defaults to `false` always.

---

This project uses **bd** (beads) for issue tracking. Run `bd prime` for full workflow context.

## Specifications

This project is spec-driven — the docs in `specs/` are the source of truth for what to build and why.

- **Start at `specs/index.md`** — the map of every spec. Read it first.
- **Load specs on demand** — from the index, open only the spec(s) relevant to your task; don't load the whole suite into context.
- **Spec-first for new features** — before building a new feature, create its spec (copy `specs/features/F00-template.md`), get it reviewed, then implement.
- **Keep specs in sync** — when you change a feature's behavior, update its spec in the same change. Specs must not drift from code.

## Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work atomically
bd close <id>         # Complete work
bd dolt push          # Push beads data to remote
```

## Non-Interactive Shell Commands

**ALWAYS use non-interactive flags** with file operations to avoid hanging on confirmation prompts.

Shell commands like `cp`, `mv`, and `rm` may be aliased to include `-i` (interactive) mode on some systems, causing the agent to hang indefinitely waiting for y/n input.

**Use these forms instead:**
```bash
# Force overwrite without prompting
cp -f source dest           # NOT: cp source dest
mv -f source dest           # NOT: mv source dest
rm -f file                  # NOT: rm file

# For recursive operations
rm -rf directory            # NOT: rm -r directory
cp -rf source dest          # NOT: cp -r source dest
```

**Other commands that may prompt:**
- `scp` - use `-o BatchMode=yes` for non-interactive
- `ssh` - use `-o BatchMode=yes` to fail instead of prompting
- `apt-get` - use `-y` flag
- `brew` - use `HOMEBREW_NO_AUTO_UPDATE=1` env var

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:ca08a54f -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

## Session Completion

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   bd dolt push
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
<!-- END BEADS INTEGRATION -->
