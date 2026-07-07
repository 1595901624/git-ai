# git-ai

`git-ai` 是一个 Git 扩展，用来记录代码中的 AI 生成痕迹，并把这些信息写入 Git Notes。它关注的是“显式归因”：AI Agent 在改文件前后调用 checkpoint，`git-ai` 根据这些 checkpoint 计算行级别归因，最终把每一行关联到对应的 agent、session、model 和提示上下文。

这个仓库是 fork 版本，当前 README 以本仓库代码为准，而不是上游官网文案。

## 能做什么

- 记录 AI / 人类 / 未归因改动的行级别来源。
- 在提交后把归因数据写入 `refs/notes/ai`。
- 提供 `git ai blame`，在普通 `git blame` 的基础上展示 AI 归因。
- 提供 `git ai stats`，统计提交或提交范围里的 AI 占比、接受率、工具/模型分布。
- 支持 rebase、merge、stash、cherry-pick、reset、amend 等常见 Git 操作中的归因迁移。
- 支持多种 Agent preset，包括 `claude`、`codex`、`cursor`、`gemini`、`github-copilot`、`windsurf`、`opencode`、`pi`、`amp`、`firebender`、`ai_tab` 等。
- 提供测试 preset：`human`、`mock_ai`、`mock_known_human`、`known_human`。

## 基本概念

### checkpoint

Agent 或测试代码通过 checkpoint 告诉 `git-ai`：某些文件在某个时刻被谁改过。

常见 checkpoint 类型：

- `human`：历史兼容名称，表示未明确归因的人类/未知改动。
- `known_human` / `mock_known_human`：明确的人类改动。
- `mock_ai`：测试用 AI preset，用于手动模拟 AI 改动。
- 真实 Agent preset：例如 `codex`、`claude`、`cursor` 等。

### working log

checkpoint 会先写入仓库的工作日志目录：

```text
.git/ai/working_logs/<base_commit>/
```

这里保存了提交前的归因状态、checkpoint 列表和文件快照。

### authorship note

提交完成后，后台 daemon 会把 working log 转换成 authorship log，并写入 Git Notes：

```bash
git notes --ref=ai list
git log --notes=ai
```

## 安装和本地开发

本仓库推荐使用 `task` 命令做本地开发安装、构建和测试：

```bash
# 安装 debug 构建，并重启 daemon
task dev

# 只检查编译
task build

# 运行测试
task test

# 运行单个测试
task test TEST_FILTER=mock_ai

# 格式化和 lint
task fmt
task lint
```

如果当前环境没有安装 `task`，可以临时使用 Cargo 命令验证：

```bash
cargo build
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets
```

> 注意：项目自己的本地运行方式以 `task dev` 为准。直接运行 `cargo run` 可能绕过安装、daemon、Git 代理或 hook 配置，导致行为和真实使用不一致。

## 常用命令

### 查看帮助

```bash
git ai help
git-ai help
```

### 启动后台服务

checkpoint 需要后台 daemon 接收请求。如果你看到 “Failed to send checkpoint to background worker” 或 named pipe / socket 不存在，通常是 daemon 没启动。

```bash
git ai bg start
git ai bg status
git ai bg restart
git ai bg shutdown
```

### 安装 Agent hooks

```bash
git ai install-hooks
git ai install-hooks --skills
```

Windows 上还可以安装 Visual Studio 扩展：

```powershell
git ai install-hooks --visual-studio-extension
```

### checkpoint

默认 checkpoint：

```bash
git ai checkpoint
```

指定测试 preset 和文件：

```bash
git ai checkpoint mock_ai README.md
git ai checkpoint mock_known_human README.md
git ai checkpoint human README.md
```

传入 JSON hook input：

```bash
git ai checkpoint codex --hook-input '{"session_id":"s1","cwd":"C:/repo","model":"gpt-5"}'
git ai checkpoint codex --hook-input stdin
```

### mock_ai 的 AgentId 参数

本 fork 支持给 `mock_ai` 显式传入 `AgentId` 中的 `tool`、`id`、`model`：

```bash
git ai checkpoint mock_ai README.md --tool codex --id session-123 --model gpt-5
```

参数含义：

| 参数 | 写入字段 | 默认值 |
| --- | --- | --- |
| `--tool <tool>` | `AgentId.tool` | `mock_ai` |
| `--id <id>` | `AgentId.id` | `ai-thread-<timestamp>` |
| `--model <model>` | `AgentId.model` | `unknown` |

这个能力主要用于测试和复现指定 AgentId 的归因数据。例如你可以模拟不同工具或不同 session 写入同一个提交：

```bash
git ai checkpoint mock_ai src/lib.rs --tool codex --id codex-thread-1 --model gpt-5
git ai checkpoint mock_ai src/main.rs --tool claude --id claude-session-2 --model claude-sonnet-4
```

### blame

```bash
git ai blame src/main.rs
git ai blame --json src/main.rs
git ai blame -L 10,40 src/main.rs
```

`git ai blame` 会尽量兼容常见 `git blame` 参数，并在输出中叠加 AI 归因。

### stats

```bash
git ai stats
git ai stats --json
git ai stats HEAD~5..HEAD --json
```

可用于查看 AI / human 行数、接受率、工具和模型分布等统计数据。

### diff

```bash
git ai diff HEAD
git ai diff HEAD~1..HEAD
git ai diff HEAD --json
git ai diff HEAD --json --include-stats --all-prompts
```

### log 和 show

```bash
git ai log
git ai log --raw
git ai log --notes

git ai show HEAD
git ai show HEAD~3..HEAD
```

### prompt 查询

```bash
git ai show-prompt <prompt-id>
git ai show-prompt <prompt-id> --commit HEAD
git ai show-prompt <prompt-id> --offset 1
```

### 配置

```bash
git ai config
git ai config notes_backend.kind
git ai config set notes_backend.kind local
git ai config unset notes_backend.kind
git ai config --add allow_repositories C:/work/repo
```

### notes 同步和迁移

```bash
git ai fetch-notes
git ai fetch-notes --remote origin --json

git ai notes migrate
git ai notes migrate --force
```

## 典型测试流程

下面是一个最小的手动复现流程：

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

如果 checkpoint 时报 daemon socket / named pipe 不存在：

```bash
git ai bg start
git ai bg status
```

然后重新执行 checkpoint。

## 架构速览

```text
AI Agent / editor
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

二进制分发上，一个 `git-ai` 可执行文件承担两类入口：

- 以 `git-ai` 调用：处理 `git ai` 子命令。
- 以 `git` 代理方式调用：转发真实 Git，并让 daemon 通过 trace2 感知 Git 操作。

后台 daemon 负责接收 checkpoint、监听 Git trace2 事件、生成提交后的 authorship note，并在重写历史时迁移归因。

## 开发约定

- Rust edition：2024。
- 主要入口：`src/main.rs`。
- `git ai` 命令处理：`src/commands/git_ai_handlers.rs`。
- Agent preset：`src/commands/checkpoint_agent/presets/`。
- working log 数据结构：`src/authorship/working_log.rs`。
- working log 持久化：`src/git/repo_storage.rs`。
- daemon 和 rewrite 逻辑：`src/daemon.rs`、`src/daemon/`、`src/authorship/rewrite*.rs`。
- 集成测试入口：`tests/integration/main.rs`。

新增 checkpoint 行为时，优先补集成测试。对于 `mock_ai` 这类 CLI 行为，可以参考：

```text
tests/integration/checkpoint_explicit_paths.rs
```

## 排错

### Failed to send checkpoint to background worker

常见原因是 daemon 没启动，或当前 shell 还没使用最新安装的 debug build。

```bash
git ai bg status
git ai bg start
```

开发时建议重新安装并重启：

```bash
task dev
```

### checkpoint 没有记录文件

检查：

- 文件路径是否在当前 Git 仓库内。
- 文件是否真的有未提交改动。
- preset 是否需要 `--hook-input`。
- daemon 是否已启动。

可以用显式路径避免自动 dirty file 发现带来的干扰：

```bash
git ai checkpoint mock_ai path/to/file.rs
```

### 看不到 Git Notes

Git AI 使用的是 `refs/notes/ai`，不是 Git 默认 notes namespace。

```bash
git notes --ref=ai list
git log --notes=ai
```

### Windows named pipe 报错

如果看到类似：

```text
timed out after 250ms connecting daemon socket \\.\pipe\...\: 系统找不到指定的文件
```

先启动或重启 daemon：

```powershell
git ai bg start
git ai bg status
```

## License

Apache-2.0
