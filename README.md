# Git MCP Server (Rust)

一个高性能的 Rust 版 Git MCP Server，为 AI 代理提供完整的 Git 操作能力。

## 特性

- **26 个 Git 工具** - 覆盖所有常用 Git 操作
- **STDIO 传输** - 与 MCP 客户端无缝集成
- **会话工作目录** - 支持动态切换项目目录
- **安全路径验证** - 可配置的基础目录限制
- **高性能** - Rust 实现，单二进制部署

## 安装

### 方式一：从源码构建

```bash
# 克隆仓库
git clone https://github.com/QiuZH001/git-mcp-server-rust.git
cd git-mcp-server-rust

# 构建 release 版本
cargo build --release

# 二进制文件位置
./target/release/git-mcp-server
```

### 方式二：直接下载

```bash
# 下载预编译二进制（如果可用）
# 或复制已编译的二进制到任意位置
cp ./target/release/git-mcp-server /usr/local/bin/
```

### 前置要求

- Rust 1.70+（仅从源码构建时需要）
- Git 2.0+
- 系统：macOS / Linux / Windows

## 配置

### 环境变量

| 变量 | 描述 | 默认值 | 示例 |
|------|------|--------|------|
| `GIT_BASE_DIR` | 限制操作的基础目录 | 无（任意目录） | `/Users/you/projects` |
| `GIT_USERNAME` | Git 提交者名称 | 全局 git config | `John Doe` |
| `GIT_EMAIL` | Git 提交者邮箱 | 全局 git config | `john@example.com` |
| `GIT_SIGN_COMMITS` | 启用提交签名 | `false` | `true` |
| `MCP_LOG_LEVEL` | 日志级别 | `info` | `debug`, `warn`, `error` |
| `MCP_TRANSPORT_TYPE` | 传输类型 | `stdio` | `http` |

### MCP 客户端配置

#### Claude Code

编辑 `~/.claude/claude_desktop_config.json`：

```json
{
  "mcpServers": {
    "git": {
      "command": "/path/to/git-mcp-server",
      "env": {
        "GIT_BASE_DIR": "/Users/yourname/projects",
        "GIT_USERNAME": "Your Name",
        "GIT_EMAIL": "your@email.com"
      }
    }
  }
}
```

#### Cursor

编辑 `~/.cursor/mcp.json`：

```json
{
  "mcpServers": {
    "git": {
      "command": "/path/to/git-mcp-server",
      "env": {
        "GIT_BASE_DIR": "/Users/yourname/projects"
      }
    }
  }
}
```

#### Windsurf

编辑 Windsurf 配置文件：

```json
{
  "mcpServers": {
    "git": {
      "command": "/path/to/git-mcp-server",
      "args": [],
      "env": {
        "GIT_BASE_DIR": "/home/user/code"
      }
    }
  }
}
```

#### OpenCode

编辑 opencode MCP 配置：

```json
{
  "mcpServers": {
    "git": {
      "command": "/path/to/git-mcp-server",
      "env": {
        "GIT_BASE_DIR": "/path/to/your/projects",
        "MCP_LOG_LEVEL": "debug"
      }
    }
  }
}
```

## 工具列表

### 仓库管理

| 工具 | 描述 | 关键参数 |
|------|------|----------|
| `git_init` | 初始化新仓库 | `path`, `initial_branch`, `bare` |
| `git_clone` | 克隆远程仓库 | `url`, `local_path`, `branch`, `depth` |
| `git_status` | 查看工作区状态 | `path`, `include_untracked` |
| `git_clean` | 删除未跟踪文件 | `force`, `dry_run`, `directories` |

### 暂存与提交

| 工具 | 描述 | 关键参数 |
|------|------|----------|
| `git_add` | 暂存文件 | `files`, `all`, `update`, `force` |
| `git_commit` | 创建提交 | `message`, `amend`, `files_to_stage`, `no_verify` |
| `git_diff` | 查看差异 | `target`, `source`, `staged`, `stat` |

### 历史查询

| 工具 | 描述 | 关键参数 |
|------|------|----------|
| `git_log` | 查看提交历史 | `max_count`, `author`, `since`, `until`, `grep` |
| `git_show` | 显示对象详情 | `object`, `format`, `stat` |
| `git_blame` | 逐行追溯 | `file`, `start_line`, `end_line` |
| `git_reflog` | 引用日志 | `ref`, `max_count` |

### 分支操作

| 工具 | 描述 | 关键参数 |
|------|------|----------|
| `git_branch` | 分支管理 | `operation`, `name`, `force`, `all` |
| `git_checkout` | 切换分支/恢复 | `target`, `create_branch`, `force` |
| `git_merge` | 合并分支 | `branch`, `no_fast_forward`, `squash` |
| `git_rebase` | 变基操作 | `upstream`, `mode`, `interactive` |
| `git_cherry_pick` | 摘取提交 | `commits`, `no_commit` |

### 远程操作

| 工具 | 描述 | 关键参数 |
|------|------|----------|
| `git_remote` | 远程仓库管理 | `mode`, `name`, `url` |
| `git_fetch` | 获取远程更新 | `remote`, `prune`, `tags` |
| `git_pull` | 拉取并合并 | `remote`, `branch`, `rebase` |
| `git_push` | 推送到远程 | `remote`, `branch`, `force`, `tags` |

### 高级操作

| 工具 | 描述 | 关键参数 |
|------|------|----------|
| `git_tag` | 标签管理 | `mode`, `tag_name`, `message`, `annotated` |
| `git_stash` | 暂存区管理 | `mode`, `message`, `stash_ref` |
| `git_reset` | 重置 HEAD | `mode`, `target`, `confirmed` |
| `git_worktree` | 工作树管理 | `mode`, `worktree_path`, `branch` |
| `git_set_working_dir` | 设置会话目录 | `path` |
| `git_clear_working_dir` | 清除会话目录 | `confirm` |

## 使用示例

### 示例 1：初始化并提交

```
AI: 帮我在 /projects/my-app 初始化一个 git 仓库并做第一次提交

1. 使用 git_init 初始化仓库
2. 使用 git_add 添加所有文件
3. 使用 git_commit 创建初始提交
```

### 示例 2：查看项目状态

```
AI: 查看当前项目的 git 状态

使用 git_status 查看分支、暂存文件、未跟踪文件等
```

### 示例 3：创建功能分支

```
AI: 创建一个名为 feature/login 的分支并切换过去

1. git_branch 创建分支
2. git_checkout 切换分支
```

### 示例 4：查看提交历史

```
AI: 查看最近 10 条提交记录

使用 git_log 并设置 max_count=10
```

### 示例 5：推送代码

```
AI: 将当前分支推送到 origin

使用 git_push，设置 remote=origin, branch=当前分支
```

## 工作目录管理

### 设置工作目录

在执行任何 Git 操作前，建议先设置工作目录：

```
使用 git_set_working_dir 设置项目路径
参数: { "path": "/path/to/your/project" }
```

### 清除工作目录

```
使用 git_clear_working_dir 清除会话中的工作目录设置
参数: { "confirm": "Y" }
```

## 安全特性

### 路径限制

设置 `GIT_BASE_DIR` 后，所有操作将被限制在该目录及其子目录内：

```json
{
  "env": {
    "GIT_BASE_DIR": "/Users/you/safe-projects"
  }
}
```

尝试访问 `GIT_BASE_DIR` 外的路径将返回错误。

### 危险操作确认

以下操作需要明确确认：

- `git_reset` (hard 模式) - 需要 `confirmed: true`
- `git_clean` - 需要 `force: true`
- `git_push --force` - 需要 `force: true`

## 故障排除

### 常见问题

**Q: 服务器启动后无响应**

确保使用 STDIO 传输模式，检查日志输出：
```bash
MCP_LOG_LEVEL=debug ./git-mcp-server
```

**Q: Git 操作失败 "command not found"**

确保系统已安装 Git 并在 PATH 中：
```bash
which git
```

**Q: 权限被拒绝**

检查 `GIT_BASE_DIR` 配置和文件系统权限。

**Q: 中文文件名乱码**

确保系统 locale 设置正确：
```bash
export LANG=en_US.UTF-8
```

### 调试模式

启用详细日志：
```json
{
  "env": {
    "MCP_LOG_LEVEL": "debug"
  }
}
```

## 开发

### 运行测试

```bash
# 运行所有测试
cargo test

# 运行单个测试
cargo test test_git_status

# 带输出运行
cargo test -- --nocapture
```

### 代码检查

```bash
cargo clippy
cargo fmt -- --check
```

## 与 TypeScript 版本对比

| 特性 | Rust 版本 | TypeScript 版本 |
|------|----------|-----------------|
| 性能 | 更快（编译型） | 较慢（解释型） |
| 部署 | 单二进制 | 需要 Node.js |
| 内存 | 更低 | 较高 |
| 启动时间 | 毫秒级 | 秒级 |
| 依赖 | 无运行时依赖 | npm 依赖 |

## 许可证

Apache-2.0

## 相关链接

- [GitHub 仓库](https://github.com/QiuZH001/git-mcp-server-rust)
- [MCP 协议规范](https://modelcontextprotocol.io)
- [原 TypeScript 版本](https://github.com/cyanheads/git-mcp-server)
