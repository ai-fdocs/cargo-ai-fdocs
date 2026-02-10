Version-locked documentation for AI coding assistants.

Sync README, CHANGELOG, and guides from your Rust dependencies — pinned to the
exact versions in your `Cargo.lock` — so that Cursor, Copilot, Windsurf, and
other AI tools stop hallucinating about APIs that changed three releases ago.

```bash
cargo install cargo-ai-fdocs
cargo ai-fdocs init      # scan Cargo.lock, generate config
cargo ai-fdocs sync      # fetch docs into docs/ai/vendor-docs/rust/

The Problem No One Talks About Honestly
Here's what actually happens when you code with AI assistants in 2025.
You ask Cursor to write an Axum handler. It generates beautiful, confident,
idiomatic Rust. You paste it in, hit compile — and get three errors. The
method signature changed two versions ago. An extractor was renamed. A trait
was moved to a different module. You spend twenty minutes fixing what should
have taken two.
This is not a rare edge case. By our rough estimate, somewhere around 20–30%
of failures in AI-assisted projects trace back to a single root cause: the
model's training data is stale. AI models train for months. Libraries update
in weeks. By the time GPT-4 or Claude learns about Axum 0.6, you're already
on 0.7. By the time the next training run catches up to 0.7, the ecosystem has
moved to 0.8.
Gemini's team calls this the "last mile" problem in AI training. We think
that understates it. It's not the last mile — it's the gap that kills trust.
When a developer gets hallucinated API calls from an AI assistant, something
breaks psychologically. One bad experience — one method not found on
confident-looking generated code — and the developer mentally downgrades the
AI from "copilot" to "toy." They stop using it for real work. They go back to
reading docs manually and writing everything by hand.
We estimate that roughly half of abandoned AI-assisted coding attempts die
not from fundamental AI limitations, but from this trust collapse. The model
could have helped with the architecture, the business logic, the tests — but
the developer gave up after the third hallucinated function signature.
The worst part: this is a completely solvable problem.

The Solution
ai-fdocs takes an almost stupidly simple approach.

1. Read your Cargo.lock to find exact dependency versions.
2. Find the corresponding git tag on GitHub for each version.
3. Download the README, CHANGELOG, and any other docs you specify.
4. Put them where your AI assistant can see them.

That's it. No vector databases. No embeddings. No cloud services. No
subscriptions. Just the right files in the right place.
The AI assistant now has the truth — not a guess from training, not a
memory from eighteen months ago, but the actual documentation for the actual
version you are using right now.
When you run cargo update, run cargo ai-fdocs sync next. Changed versions
get new docs. Unchanged versions stay cached. Removed dependencies get pruned.
The docs always match your lockfile.

The Butterfly Effect (Why This Matters Beyond Your Project)
There's a second-order consequence we didn't expect when building this tool.
When an AI assistant has accurate documentation, it generates correct code.
That code gets committed. It ends up in public repositories, in blog posts,
in Stack Overflow answers, in the open-source projects that future AI models
will train on.
Think about what that means.
accurate docs in context
    → correct generated code
        → better public code corpus
            → better-trained future models
                → less hallucination out of the box

Every major AI lab spends enormous resources trying to improve code generation.
They curate training datasets, filter for quality, run RLHF on benchmarks. But
there is a much simpler lever that nobody is pulling: if the code that exists
in the world is more correct, the models trained on it will be more correct.
We call this the Documentation Flywheel. It's a hypothesis, not a proven
fact. But the mechanism is straightforward, and we believe the effect is real
and potentially significant:

* Today: Your AI assistant writes better code because it has the right docs.
* This month: Your team's codebase has fewer deprecated API calls.
* This year: Thousands of projects using ai-fdocs produce cleaner code.
* Next training cycle: Models learn from a corpus with less API drift noise.
* Next year: AI assistants hallucinate less, even without ai-fdocs.

We can't put a precise number on the magnitude. Maybe it's a 5% improvement in
code generation quality across the ecosystem. Maybe it's 50%. It depends on
adoption, on how much of the training corpus is affected, on a dozen variables
we can't measure yet. But the direction is clear: more accurate code in the
world means more accurate AI in the future.
The companies building AI models are trying to solve this from the top down
with better training. ai-fdocs solves it from the bottom up, one project at a
time. Both approaches compound. Together, they could close the knowledge gap
much faster than either alone.

How It Works
ai-fdocs operates on three principles:
Lockfile is truth. Your Cargo.lock contains the exact versions of every
dependency. ai-fdocs reads it directly — no guessing, no "latest", no
approximation.
Tags are anchors. For each dependency, ai-fdocs finds the corresponding
git tag (v1.0.0, 1.0.0, crate-v1.0.0) on GitHub and fetches
documentation from that exact commit. If no tag is found, it falls back to the
default branch and warns you clearly.
Freshness is automatic. Run sync after cargo update and the tool
detects which versions changed, prunes stale docs, and fetches new ones.
Unchanged dependencies are served from cache. A check command exits with
code 1 if docs are outdated — plug it into CI and never drift again.

Quick Start
1. Install
bashDownloadCopy codecargo install cargo-ai-fdocs
2. Generate config from your Cargo.lock
bashDownloadCopy codecargo ai-fdocs init
This scans your Cargo.lock, queries crates.io for GitHub repository URLs,
filters out low-level crates (proc-macro internals, libc, unicode tables),
and generates an ai-fdocs.toml with all your meaningful dependencies.
Review the generated file. Remove crates you don't need docs for. Add
ai_notes for your key dependencies — these are injected into the index
and help the AI understand how your project uses each library.
3. Sync documentation
bashDownloadCopy codecargo ai-fdocs sync
Documentation is saved to docs/ai/vendor-docs/rust/ by default:
docs/ai/vendor-docs/rust/
├── _INDEX.md                    # Global index with AI notes
├── axum@0.7.4/
│   ├── _SUMMARY.md              # Local index with file table
│   ├── .aifd-meta.toml          # Metadata (version, ref, hash)
│   ├── README.md                # Fetched from v0.7.4 tag
│   └── CHANGELOG.md             # Truncated to relevant versions
├── serde@1.0.197/
│   ├── _SUMMARY.md
│   ├── .aifd-meta.toml
│   └── README.md
└── tokio@1.36.0/
    ├── _SUMMARY.md
    ├── .aifd-meta.toml
    ├── README.md
    └── CHANGELOG.md

4. Point your AI assistant to the docs
For Cursor, add to .cursorrules:
When working with Rust dependencies, always check docs/ai/vendor-docs/rust/
for version-specific documentation before suggesting API usage. Start with
_INDEX.md for an overview, then read the relevant crate's _SUMMARY.md and
README.md. The documentation is pinned to the exact versions in Cargo.lock.

For GitHub Copilot, the docs are automatically included in workspace
context if they're in the repository.
For Windsurf or other tools, point them to the _INDEX.md file.
5. Keep it fresh
bashDownloadCopy codecargo update
cargo ai-fdocs sync
Only changed versions are re-fetched. Everything else comes from cache.

Configuration
Full example
tomlDownloadCopy code[settings]
output_dir = "docs/ai/vendor-docs/rust"   # Where to save docs
max_file_size_kb = 200                     # Truncate files larger than this
prune = true                               # Remove docs for removed/updated deps

[crates.axum]
repo = "tokio-rs/axum"
ai_notes = """
Primary web framework. Use axum 0.7 patterns:
- Router::new().route() for routing
- extract::State for shared state
- Json/Path/Query extractors for request data
"""

[crates.serde]
repo = "serde-rs/serde"
ai_notes = "Use #[derive(Serialize, Deserialize)] for all DTOs."

[crates.sqlx]
repo = "launchbadge/sqlx"
files = ["README.md", "CHANGELOG.md", "docs/migration-guide.md"]
ai_notes = "Use compile-time checked queries with sqlx::query! macro."

[crates.axum-core]
repo = "tokio-rs/axum"
subpath = "axum-core"
Configuration reference
[settings] — Global settings. All fields optional with sensible defaults.
[crates.<name>] — One section per dependency you want docs for.
FieldRequiredDescriptionrepoYesGitHub owner/repo stringsubpathNoSubdirectory in monorepo (e.g., axum-core)filesNoExplicit file list. Overrides default README/CHANGELOG searchai_notesNoFree-text context for AI. Injected into _INDEX.md

Commands
cargo ai-fdocs init
Generates ai-fdocs.toml by scanning Cargo.lock and querying crates.io for
repository URLs. Filters out low-level infrastructure crates automatically.
bashDownloadCopy codecargo ai-fdocs init              # Generate config
cargo ai-fdocs init --overwrite  # Replace existing config
cargo ai-fdocs sync
Fetches documentation for all configured crates. Skips cached versions
unless --force is used.
bashDownloadCopy codecargo ai-fdocs sync              # Normal sync (uses cache)
cargo ai-fdocs sync --force      # Re-download everything
cargo ai-fdocs status
Shows a table of all configured crates with their lockfile versions and
documentation status.
Crate                    Lock Version    Docs Status
──────────────────────────────────────────────────────────────────────
axum                     0.7.4           ✅ Synced
serde                    1.0.197         ✅ Synced
tokio                    1.36.0          ⚠️  Synced (fallback: main)
sqlx                     0.7.3           ❌ Missing

cargo ai-fdocs check
Exits with code 0 if all docs are up-to-date, code 1 if not. Designed for CI
pipelines.
bashDownloadCopy codecargo ai-fdocs check || echo "Docs are outdated! Run cargo ai-fdocs sync."

CI Integration
GitHub Actions
yamlDownloadCopy code- name: Check AI docs are fresh
  run: cargo ai-fdocs check
If docs are committed to the repo, this ensures they stay in sync with
Cargo.lock. If a developer runs cargo update but forgets cargo ai-fdocs sync, CI catches it.
Pre-commit hook
bashDownloadCopy code#!/bin/sh
cargo ai-fdocs check --quiet || {
    echo "AI docs are outdated. Running sync..."
    cargo ai-fdocs sync
    git add docs/ai/vendor-docs/
}

How Files Are Processed
Tag resolution. For each crate version, ai-fdocs tries multiple tag
patterns: v1.0.0, 1.0.0, crate-v1.0.0, crate-1.0.0. If none match,
it falls back to the default branch and marks the docs with a warning.
Filename flattening. Nested paths are flattened with double underscores:
docs/guides/overview.md becomes docs__guides__overview.md. This avoids
deep directory trees while preserving the original path in the name.
CHANGELOG truncation. CHANGELOGs are trimmed to the current version plus
one previous minor version. A 500KB changelog becomes a focused 5KB summary
of what matters for your installed version.
Size limits. Files exceeding max_file_size_kb (default 200KB) are
truncated with a clear marker. This keeps AI context windows manageable.
Markdown headers. Every .md file gets an HTML comment header with source
URL, git ref, and fetch date. Fallback files get an additional warning comment.
This helps the AI (and humans) understand provenance.
Config hash. The tool hashes your configuration for each crate (repo,
subpath, files). If you change the config — add a file, change the subpath —
the next sync detects the change and re-fetches without needing --force.

GitHub Token
Without a token, GitHub API allows 60 requests per hour. With a token, 5000.
For projects with many dependencies, set a token:
bashDownloadCopy codeexport GITHUB_TOKEN=ghp_your_token_here
cargo ai-fdocs sync
The tool checks GITHUB_TOKEN and GH_TOKEN environment variables. If you
use GitHub CLI (gh), GH_TOKEN is usually already set.

Should I Commit the Docs?
Yes. Committing vendor docs ensures every team member and CI system has
the same documentation without needing to run sync. Add to .gitattributes
to keep diffs clean:
docs/ai/vendor-docs/** linguist-generated=true

This tells GitHub to collapse these files in pull request diffs and exclude
them from language statistics.

The Numbers We're Honest About
We don't have peer-reviewed studies. We have experience, conversations with
dozens of developers, and pattern recognition. Here's what we believe, with
appropriate uncertainty:
~20–30% of AI-assisted coding failures seem to trace back to stale
library knowledge. Not the majority, but a large and entirely preventable
slice. The rest are architecture errors, unclear requirements, and
fundamental AI limitations that better docs can't fix.
~50% of developers who abandon AI coding tools appear to do so after
trust-breaking moments — and hallucinated APIs are the most common trust
breaker we've seen. Fix the hallucinations, and many of those developers
would have stayed.
The Documentation Flywheel effect is real but unmeasured. We believe
that widespread adoption of version-accurate documentation in AI context
would meaningfully improve the next generation of code models. Whether
that's a 5% improvement or a 50% improvement, we genuinely don't know.
The mechanism is clear; the magnitude isn't.
We'd rather be honest about uncertainty than confident about made-up
numbers. If you have data that sharpens these estimates, we'd love to hear
from you.

FAQ
Q: How is this different from just pasting docs into chat?
Pasting works for one library, one time. ai-fdocs works for all your
dependencies, automatically, and stays in sync with your lockfile. It also
structures the documentation in a way that AI assistants can navigate
efficiently — with an index, summaries, and AI-specific notes.
Q: Won't this bloat my repository?
A typical dependency's README + truncated CHANGELOG is 5–20KB. Even with 30
dependencies, you're looking at 150–600KB — less than a single image asset.
Q: What about crates that aren't on GitHub?
Currently ai-fdocs only supports GitHub-hosted repositories. GitLab and
Bitbucket support is planned for v0.3.
Q: What about private dependencies?
If your GITHUB_TOKEN has access to the private repository, ai-fdocs will
fetch from it just like any public repo.
Q: Does this work with workspaces?
Yes. ai-fdocs reads the root Cargo.lock, which covers all workspace members.
Run it from the workspace root.

Roadmap
v0.1 (alpha)

*  Config parsing (ai-fdocs.toml)
*  Cargo.lock resolution
*  GitHub tag resolution with fallback
*  README/CHANGELOG fetching
*  File flattening, headers, truncation
*  Caching with .aifd-meta.toml
*  Pruning
*  _INDEX.md generation

v0.2 (current)

*  init command via crates.io API
*  Parallel fetching (8 concurrent)
*  check command for CI (exit code)
*  _SUMMARY.md per crate
*  Config hash for change detection
*  Enhanced status with hints

v0.3 (planned)

*  GitLab / Bitbucket support
*  docs.rs fallback for crates without README
*  Custom file transforms (strip badges, TOC)
*  watch mode (auto-sync on Cargo.lock change)
*  Workspace-aware config inheritance

v0.4 (planned)

*  Semantic chunking for large docs
*  Cross-reference linking between crate docs
*  ai-fdocs explain <crate> — summarize a crate with AI
*  Plugin system for custom documentation sources


The Bigger Picture
Every AI coding assistant today works with incomplete information. The model
knows what it learned during training — a static snapshot of libraries as they
existed months or years ago. Your project uses libraries as they exist today.
The industry is converging on a solution: give the model the right context at
inference time. RAG, tool use, web search — these are all variations of the
same idea. ai-fdocs is the simplest, most reliable implementation for
dependency documentation: no infrastructure, no ongoing costs, no vendor
lock-in. Just the right files in the right place.
And through the Documentation Flywheel, every project that adopts this approach
contributes — however slightly — to a world where AI code generation is more
accurate for everyone.
It starts with a single command:
bashDownloadCopy codecargo ai-fdocs sync

License
MIT

Contributing
Contributions welcome. Please open an issue first to discuss what you'd like
to change.
If you have data on AI hallucination rates, code generation quality, or the
impact of documentation on AI output — we're especially interested. Help us
replace our estimates with real numbers.

