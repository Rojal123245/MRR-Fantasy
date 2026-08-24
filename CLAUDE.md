# MRR Fantasy

Full-stack fantasy football app. See [README.md](README.md) for architecture, setup, and API docs.

- **Frontend**: Next.js 16 (App Router) + TypeScript + Tailwind + Framer Motion (`frontend/`)
- **Backend**: Rust (Axum) + SQLx + PostgreSQL (`backend/`)
- **DB migrations**: `migrations/`

## Wiki Knowledge Base

Path: ./wiki

This project is paired with a [claude-obsidian](https://github.com/AgriciDaniel/claude-obsidian)
vault at `./wiki` (an Obsidian vault, git-ignored from this repo). Use it as the persistent,
compounding knowledge base for this project.

- Notes live under `wiki/wiki/` (`concepts/`, `entities/`, `sources/`, `meta/`).
- Use `/wiki` to scaffold/continue the knowledge base, `ingest [file]` to process sources,
  `/autoresearch [topic]` for research loops, and `/think [problem]` for the thinking framework.
- Run `lint the wiki` for a vault health check.
