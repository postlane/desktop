# CLAUDE.md — postlane/desktop

## Non-negotiable rules

**No `Co-Authored-By: Claude` or any AI attribution in commits or PRs.** Never
add `Co-Authored-By:`, "Generated with Claude Code", or any AI tool attribution
to a commit message or PR description.

**Never** `git commit --no-verify`. **Never** `git push --force`.

---

## Language — Rust + React/TypeScript (Tauri v2)

### Rust

- `cargo clippy --deny warnings` must pass — zero warnings
- No `unwrap()` on `Result` or `Option` — use `?`, `map_err`, or explicit handling
- No `unsafe` blocks, no `todo!()`, no `unimplemented!()` in committed code
- Atomic file writes: write to `{file}.tmp` then `std::fs::rename` — never
  `std::fs::write` directly on state files
- All errors must include context (path, operation, what was expected)

### TypeScript/React

- `"strict": true` — no `any`, no type assertions (`as T`), no `!`, no `@ts-ignore`
- All Tauri IPC `invoke()` calls must have explicit error handling — never assume success
- Surface errors to the user; do not swallow them silently

---

## Security — never violate

1. Credentials go in the OS keyring only (`tauri-plugin-keyring`)
2. Validate all repo paths against `repos.json` before reading or writing
3. `session.token` and `port` files must have 0600 permissions on Unix
4. All URLs must start with `https://` — reject bare IPs and `http://`
5. SSRF: validate URLs do not resolve to private ranges before any fetch
6. No analytics SDK, no telemetry — zero

Each security rule must have a corresponding test.

---

## Testing (TDD — non-negotiable)

1. Write failing test → confirm RED
2. Write minimum code to pass → confirm GREEN
3. Refactor → commit

- `cargo test` — all pass
- Vitest — all pass
- `cargo clippy --deny warnings` — zero warnings
- `npm audit` — no high/critical

---

## Code limits

| Metric | Limit |
|--------|-------|
| Lines per file | 400 |
| Lines per function | 60 |
| Nesting depth | 3 |
| Cyclomatic complexity | 12 |

No `#[allow(...)]` or ESLint disable comments to bypass limits — fix the code.

---

## Naming

Forbidden: `utils`, `helpers`, `common`, `shared`, `core`, `misc`, `lib`.
Use domain-specific names.
