# AGENTS

General reference for choosing subagents when working on this repository.

## Recommended usage

- Use `SWE` for most implementation tasks (feature work, bug fixes, refactors, tests).
- Use `QA` for test strategy, failure triage, edge-case review, and validation plans.
- Use `Explore` for fast read-only codebase discovery before editing.
- Use `principal-software-engineer` for architecture tradeoffs and design direction.
- Use `RUG` only when you want orchestration/delegation across multiple subagents.
- Use `Maker` only for product/idea exploration (usually not needed for this repo).

## Task-to-agent mapping

- Add or change CLI/download behavior: `SWE`
- Investigate regressions or flaky tests: `QA` then `SWE`
- Understand unfamiliar module ownership quickly: `Explore`
- Evaluate larger redesign (layout, API, concurrency model): `principal-software-engineer`
- Multi-step parallel investigation with delegated execution: `RUG`

## Practical workflow

1. Start with `Explore` when context is unclear.
2. Implement with `SWE`.
3. Validate with `QA` for edge cases and gaps.
4. Escalate architectural questions to `principal-software-engineer` only when needed.

## Repo-specific guidance

- Prefer preserving CLI compatibility (`-DM` alias rewrite and existing argument semantics).
- Keep output path behavior consistent with current README and `src/utils.rs`.
- Run `cargo test` after code changes; include a dry-run command for path-related changes.
- Avoid changing Telegram auth/session behavior unless required by a bug or feature request.
