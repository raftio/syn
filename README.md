# syn

Personal knowledge base maintained by an LLM. You feed it documents and URLs; it builds and cross-references a wiki of markdown pages. You query it with natural language; it answers with citations. **Chat** is the same retrieval-backed Q&A in a multi-turn stdin loop.

## How it works

1. **Ingest** a source (local file or URL) → the LLM reads it, creates/updates wiki pages, and logs the operation.
2. **Query** in plain English → BM25 retrieves the most relevant pages, the LLM synthesises an answer with `[[wikilinks]]`.
3. **Chat** (optional) → same idea as query, but each line keeps conversation context; the wiki index is sent once in the system prompt, and each turn adds fresh BM25 hits for that line.
4. **Lint** periodically → static analysis finds orphan pages, broken links, and missing frontmatter; the LLM review finds contradictions, stale claims, and knowledge gaps.

All wiki edits are done via a structured `wiki_edit` tool call so every change is auditable.

## Installation

### Pre-built binary

Download the archive for your platform, unpack it, and put the `syn` binary on your `PATH`. Assets for [v0.0.1](https://github.com/raftio/syn/releases/tag/v0.0.1):

| Platform | Asset |
| -------- | ----- |
| macOS (Apple Silicon, aarch64) | [`syn-aarch64-apple-darwin.tar.gz`](https://github.com/raftio/syn/releases/download/v0.0.1/syn-aarch64-apple-darwin.tar.gz) |
| Linux (x86_64) | [`syn-x86_64-unknown-linux-gnu.tar.gz`](https://github.com/raftio/syn/releases/download/v0.0.1/syn-x86_64-unknown-linux-gnu.tar.gz) |
| Windows (x86_64) | [`syn-x86_64-pc-windows-msvc.zip`](https://github.com/raftio/syn/releases/download/v0.0.1/syn-x86_64-pc-windows-msvc.zip) |

**macOS (Apple Silicon)**

```bash
curl -LO https://github.com/raftio/syn/releases/download/v0.0.1/syn-aarch64-apple-darwin.tar.gz
tar xzf syn-aarch64-apple-darwin.tar.gz
chmod +x syn
sudo mv syn /usr/local/bin/
```

**Linux (x86_64)**

```bash
curl -LO https://github.com/raftio/syn/releases/download/v0.0.1/syn-x86_64-unknown-linux-gnu.tar.gz
tar xzf syn-x86_64-unknown-linux-gnu.tar.gz
chmod +x syn
sudo mv syn /usr/local/bin/
```

**Windows (x86_64)**

Download the [zip](https://github.com/raftio/syn/releases/download/v0.0.1/syn-x86_64-pc-windows-msvc.zip), extract `syn.exe`, and either run it from that folder or add the folder to your PATH.

### Build from source

Requires [Rust](https://rustup.rs) (see [Requirements](#requirements)).

```bash
git clone https://github.com/raftio/syn.git
cd syn
cargo install --path .
```

## Quick start

After [installing](#installation) `syn`:

```bash
# Initialise a knowledge base in the current directory
syn init
export ANTHROPIC_API_KEY=sk-...

# Ingest something
syn ingest path/to/article.md
syn ingest https://example.com/post

# Query
syn query "What are the main themes across my notes?"

# Multi-turn chat (stdin; type exit or quit to leave)
syn chat

# Check for issues
syn lint
```

## Commands

### `syn init`

Initialise a knowledge base in the current directory.

```bash
syn init [--force] [--vault]
```

- `--force` — re-initialise even if a KB already exists (overwrites templates, keeps config)
- `--vault` — initialise inside an existing **Obsidian vault** (uses `syn/` + `syn-sources/` dirs, Obsidian `[[Note Name]]` wikilinks)

Creates:

```text
.syn/config.toml   ← configuration
CLAUDE.md          ← wiki schema / LLM instructions
index.md           ← catalog of all pages
log.md             ← append-only operation log
wiki/              ← LLM-authored pages (entities/, concepts/, sources/, synthesis/)
raw/               ← raw source files
```

### `syn ingest <path|url>`

Ingest a source document into the wiki.

```bash
syn ingest article.md
syn ingest https://example.com/post
syn ingest --dry-run article.md   # preview edits without writing
syn ingest --yes article.md       # skip confirmation prompt
syn ingest --model claude-opus-4-7 article.md
```

The LLM creates a summary page at `wiki/sources/<slug>.md`, creates or updates entity/concept pages, updates `index.md`, and appends a log entry.

### `syn query <question>`

Answer a question using the wiki.

```bash
syn query "What did the article say about caching?"
syn query --save "caching-summary" "Summarise everything on caching"
```

BM25 retrieves the top-K relevant pages; the LLM synthesises an answer with `[[wikilink]]` citations. `--save` writes the answer as a synthesis page.

### `syn chat`

Interactive multi-turn wiki chat on stdin. The prompt is `chat> ` on stderr; assistant text streams on stdout.

```bash
syn chat
syn chat --model claude-opus-4-7
```

- Each non-empty line is searched with BM25 (same `top_k` as **Query**); retrieved page excerpts are attached to that turn’s user message.
- The full `index.md` catalogue is included **once** in the system prompt (token-efficient vs. repeating it every turn).
- Type **`exit`** or **`quit`** (any case), or end stdin (Ctrl-D), to stop.
- There is no `--save` flag; use **`syn query --save …`** when you want a synthesis page written to disk.

### `syn search <query>`

Local BM25 search (no LLM call). Fast, offline.

```bash
syn search "vector database"
syn search --top 3 "embedding"
```

### `syn lint [--fix]`

Health-check the wiki.

```bash
syn lint                 # static analysis + LLM review (read-only)
syn lint --fix           # also auto-apply suggested fixes
syn lint --static-only   # skip LLM pass
syn lint --yes           # apply fixes without confirmation
```

**Static checks**: orphan pages, broken wikilinks, pages missing from `index.md`, pages without frontmatter.

**LLM review**: contradictions between pages, stale claims, missing concept pages, weak cross-references.

### `syn log`

Show recent operation log entries.

```bash
syn log              # last 10 entries
syn log --tail 20
```

### `syn config`

View or edit configuration.

```bash
syn config show
syn config get llm.model
syn config set llm.model gpt-4o
syn config set llm.provider openai
```

## Configuration

`.syn/config.toml` (auto-generated on `init`):

```toml
[llm]
provider = "anthropic"        # "anthropic" | "openai"
model = "claude-sonnet-4-6"
max_tokens = 8192
api_key_env = "ANTHROPIC_API_KEY"

[paths]
wiki = "wiki"
raw = "raw"
schema = "CLAUDE.md"
index = "index.md"
log = "log.md"

[search]
backend = "bm25"
top_k = 8

[ingest]
auto_commit = false
include_schema_in_prompt = true
```

Switch to OpenAI:

```bash
syn config set llm.provider openai
syn config set llm.model gpt-4o
syn config set llm.api_key_env OPENAI_API_KEY
export OPENAI_API_KEY=sk-...
```

## Obsidian vault mode

If you already use Obsidian, `syn init --vault` integrates into your vault without touching your existing notes:

```bash
cd ~/my-obsidian-vault
syn init --vault
```

- Wiki pages go into `syn/` (visible in Obsidian as a folder of notes)
- Source files go into `syn-sources/`
- Wikilinks use Obsidian `[[Note Name]]` syntax (no path, no extension)
- Lint detects ambiguous wikilinks when multiple notes share the same name

The `.syn/` directory is hidden from Obsidian by default.

## Wiki page conventions

Every page should have YAML frontmatter:

```yaml
---
title: "Vector Databases"
tags: [concept, database]
sources: [raw/pinecone-docs.md]
updated: 2026-04-23
---
```

Pages are organised by type:

| Directory          | Purpose                                              |
| ------------------ | ---------------------------------------------------- |
| `wiki/entities/`   | People, places, organisations, products              |
| `wiki/concepts/`   | Ideas, topics, frameworks, techniques              |
| `wiki/sources/`    | One-page summary per ingested document               |
| `wiki/synthesis/`  | Comparisons, analyses, cross-cutting essays          |

## Requirements

- **Runtime**: the pre-built binary has no Rust requirement. Building from source needs Rust 1.75+.
- `ANTHROPIC_API_KEY` (default) or `OPENAI_API_KEY`

## Development

```bash
cargo build
cargo test
cargo clippy -- -D warnings
```

Integration tests use [wiremock](https://github.com/LukeMathWalker/wiremock-rs) to mock LLM APIs — no real API key needed for `cargo test`.
