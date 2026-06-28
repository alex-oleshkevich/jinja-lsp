# Project Instructions for AI Agents

This file provides instructions and context for AI coding agents working on this project.

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


## Specifications

This project is spec-driven. See **AGENTS.md › Specifications** — start at `specs/index.md`, load specs on demand, and create a spec before building any new feature.

## Autonomous Implementation Workflow

When implementing tasks from the beads backlog, use the **`autonomous-coder` skill** and run continuously:

- **Never stop after finishing a single task.** When a task is done, claim the next ready bead (`bd ready --limit 500`) and keep going.
- **Stop ONLY when** all tasks are done, OR you genuinely need human intervention — an ambiguous product decision, an external blocker you cannot resolve, or a destructive/irreversible action that requires approval. A normal failing test or lint is NOT a stop condition; debug and fix it.
- **After every task, self-review:** run the **`self-review` skill**, fix every finding, and re-iterate the review until it is clean.
- **Commit at the end of each task** (commit permission is granted — commit without asking), then move on to the next task.
  - No git remote is configured, so commits are local: `git commit` for code + `bd dolt commit` for issue state. Push only if/when a remote is added.
- Follow the layered order: work `bd ready` from the foundations up; closing a layer's tasks automatically unblocks the next.
- Use **TDD** (`test-driven-development` skill) — every impl task's acceptance is "implemented and verified by a passing test."

## Build & Test

_Add your build and test commands here_

```bash
# Example:
# npm install
# npm test
```

## Architecture Overview

_Add a brief overview of your project architecture_

## Conventions & Patterns

_Add your project-specific conventions here_
