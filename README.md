# git-ai

[中文说明](README_ZH.md)

`git-ai` is a Git extension for tracking AI-authored code and storing line-level attribution in Git Notes. It is built around explicit attribution: AI agents call checkpoints before and after editing files, and `git-ai` turns those checkpoints into durable metadata that links changed lines to the agent, session, model, and prompt context behind them.

This repository is a fork. This README describes the code in this fork rather than serving as upstream marketing copy.

## What it does

- Tracks line-level attribution for AI, known-human, and unattributed changes.
- Writes post-commit authorship data to `refs/notes/ai`.
- Provides `git ai blame`, a blame overlay that shows AI attribution alongside normal Git blame data.
- Provides `git ai stats` for AI percentage, acceptance rate, and tool/model breakdowns.
- Preserves attribution across common Git operations such as rebase, merge, stash, cherry-pick, reset, and amend.
- Supports multiple agent presets, including `claude`, `codex`, `cursor`, `gemini`, `github-copilot`, `windsurf`, `opencode`, `pi`, `amp`, `firebender`, and `ai_tab`.
- Includes test presets: `human`, `mock_ai`, `mock_known_human`, and `known_human`.

## Core concepts

### Checkpoints

An agent, editor integration, or test invokes a checkpoint to tell `git-ai` which files were edited and who should be credited for those edits.

Common checkpoint types:

- `human`: legacy/unattributed-human checkpoint.
- `known_human` / `mock_known_human`: explicit known-human edits.
- `mock_ai`: test preset for manually simulating AI edits.
- Real agent presets such as `codex`, `claude`, and `cursor`.

### Working logs

Checkpoints are first persisted under:

```text
.git/ai/working_logs/<base_commit>/
```

The working log stores pre-commit attribution state, checkpoint records, and file snapshots.

### Authorship notes

After a commit, the background daemon converts the working log into an authorship log and stores it in Git Notes:

```bash
git notes --ref=ai list
git log --notes=ai
```

## Local development

This repository is designed to be developed through `task`:

```bash
# Install a debug build and restart the daemon
task dev

# Compile only
task build

# Run tests
task test

# Run a filtered test
task test TEST_FILTER=mock_ai

# Format and lint
task fmt
task lint
```

If `task` is not available in your shell, the following Cargo commands are useful for local verification:

```bash
cargo build
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets
```

> For real local usage, prefer `task dev`. Running `cargo run` directly can bypass installation, daemon setup, Git proxy behavior, or hook configuration, so it may not match production behavior.

## Common commands

### Help

```bash
git ai help
git-ai help
```

### Background daemon

Checkpoint requests are sent to a background daemon. If you see “Failed to send checkpoint to background worker” or a missing named pipe / socket error, the daemon is usually not running.

```bash
git ai bg start
git ai bg status
git ai bg restart
git ai bg shutdown
```

### Install agent hooks

```bash
git ai install-hooks
git ai install-hooks --skills
```

On Windows, Visual Studio extension installation is also supported:

```powershell
git ai install-hooks --visual-studio-extension
```

### Checkpoint

Default checkpoint:

```bash
git ai checkpoint
```

Test presets with explicit file paths:

```bash
git ai checkpoint mock_ai README.md
git ai checkpoint mock_known_human README.md
git ai checkpoint human README.md
```

Preset hook input:

```bash
git ai checkpoint codex --hook-input '{"session_id":"s1","cwd":"C:/repo","model":"gpt-5"}'
git ai checkpoint codex --hook-input stdin
```

### `mock_ai` AgentId arguments

This fork supports explicit `AgentId` overrides for the `mock_ai` preset:

```bash
git ai checkpoint mock_ai README.md --tool codex --id session-123 --model gpt-5
```

| Argument | Stored field | Default |
| --- | --- | --- |
| `--tool <tool>` | `AgentId.tool` | `mock_ai` |
| `--id <id>` | `AgentId.id` | `ai-thread-<timestamp>` |
| `--model <model>` | `AgentId.model` | `unknown` |

This is mainly useful for tests and reproductions where you need deterministic agent metadata:

```bash
git ai checkpoint mock_ai src/lib.rs --tool codex --id codex-thread-1 --model gpt-5
git ai checkpoint mock_ai src/main.rs --tool claude --id claude-session-2 --model claude-sonnet-4
```

### Blame

```bash
git ai blame src/main.rs
git ai blame --json src/main.rs
git ai blame -L 10,40 src/main.rs
```

`git ai blame` aims to preserve familiar `git blame` behavior while adding AI attribution.

### Stats

```bash
git ai stats
git ai stats --json
git ai stats HEAD~5..HEAD --json
```

Use stats to inspect AI/human line counts, accepted AI code, and tool/model breakdowns.

### Diff

```bash
git ai diff HEAD
git ai diff HEAD~1..HEAD
git ai diff HEAD --json
git ai diff HEAD --json --include-stats --all-prompts
```

### Log and show

```bash
git ai log
git ai log --raw
git ai log --notes

git ai show HEAD
git ai show HEAD~3..HEAD
```

### Prompt lookup

```bash
git ai show-prompt <prompt-id>
git ai show-prompt <prompt-id> --commit HEAD
git ai show-prompt <prompt-id> --offset 1
```

### Config

```bash
git ai config
git ai config notes_backend.kind
git ai config set notes_backend.kind local
git ai config unset notes_backend.kind
git ai config --add allow_repositories C:/work/repo
```

### Notes sync and migration

```bash
git ai fetch-notes
git ai fetch-notes --remote origin --json

git ai notes migrate
git ai notes migrate --force
```

## Minimal manual test

```bash
git init demo
cd demo

echo "hello" > README.md
git add README.md
git commit -m "initial"

echo "hello from ai" > README.md
git ai checkpoint mock_ai README.md --tool codex --id demo-session --model gpt-5

git add README.md
git commit -m "ai edit"

git ai blame README.md
git ai stats --json
git log --notes=ai
```

If checkpointing fails because the daemon socket / named pipe does not exist:

```bash
git ai bg start
git ai bg status
```

Then rerun the checkpoint.

## Architecture overview

```text
AI agent / editor
      |
      | git ai checkpoint <preset>
      v
checkpoint preset parser
      |
      | AgentId + file snapshots + metadata
      v
daemon checkpoint processor
      |
      | char/line attribution
      v
.git/ai/working_logs/<base_commit>
      |
      | after git commit / rewrite operation
      v
Git Notes: refs/notes/ai
      |
      v
git ai blame / stats / diff / log / show
```

The executable has two dispatch modes:

- Invoked as `git-ai`: handles `git ai` subcommands.
- Invoked as a Git proxy: forwards to the real Git binary while allowing the daemon to observe Git operations through trace2.

The daemon receives checkpoints, ingests Git trace2 events, writes post-commit authorship notes, and migrates attribution through history-rewrite operations.

## Development notes

- Rust edition: 2024.
- Main entry point: `src/main.rs`.
- `git ai` command handling: `src/commands/git_ai_handlers.rs`.
- Agent presets: `src/commands/checkpoint_agent/presets/`.
- Working log data structures: `src/authorship/working_log.rs`.
- Working log persistence: `src/git/repo_storage.rs`.
- Daemon and rewrite logic: `src/daemon.rs`, `src/daemon/`, and `src/authorship/rewrite*.rs`.
- Integration test entry point: `tests/integration/main.rs`.

When adding checkpoint behavior, prefer adding integration coverage. For CLI behavior around `mock_ai`, see:

```text
tests/integration/checkpoint_explicit_paths.rs
```

## Troubleshooting

### Failed to send checkpoint to background worker

The daemon is usually not running, or the shell is not using the latest installed debug build.

```bash
git ai bg status
git ai bg start
```

For development, reinstall and restart through:

```bash
task dev
```

### A checkpoint did not record a file

Check that:

- The file path is inside the current Git repository.
- The file actually has uncommitted changes.
- The selected preset does not require `--hook-input`.
- The daemon is running.

Use an explicit path to avoid ambiguity in dirty-file discovery:

```bash
git ai checkpoint mock_ai path/to/file.rs
```

### Git Notes are missing

Git AI uses `refs/notes/ai`, not Git's default notes namespace.

```bash
git notes --ref=ai list
git log --notes=ai
```

### Windows named pipe error

If you see an error like:

```text
timed out after 250ms connecting daemon socket \\.\pipe\...\: The system cannot find the file specified
```

Start or restart the daemon:

```powershell
git ai bg start
git ai bg status
```

## License

Apache-2.0
