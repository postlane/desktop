# Postlane Desktop

The desktop app is the primary way to set up Postlane. Sign in, click
**"+ New workspace"** (or **"Add workspace"** from an existing org's
Settings tab), and the in-app setup wizard walks you through:

1. Picking the folder that contains your Git repositories
2. Basic config — base URL, platforms, author, writing style
3. LLM provider and model
4. Scheduler provider and API key
5. Attribution preference
6. Review and confirm

No terminal required — the wizard writes `config.json`, copies the skill
files into every discovered repo, stores your scheduler API key in the OS
keyring, and registers the workspace, all from the UI.

## Power users and CI/CD

For scripted or headless setup, use the CLI instead:

```bash
npx @postlane/cli init --workspace [path]
```

This does the same underlying setup (`config.json`, skill files, keyring
credentials, workspace registration) without the desktop app, and is the
right choice for CI pipelines or automating setup across many repos. The
CLI and desktop wizard currently write slightly different `config.json`
field sets (a known, tracked gap, not yet reconciled) -- either path
produces a working workspace, but don't assume byte-identical output
between them.

## Development

See `CLAUDE.md` in this directory for the coding conventions (strict TDD,
clippy/ESLint limits, security rules) enforced on every commit.
