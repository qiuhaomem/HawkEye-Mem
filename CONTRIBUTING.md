# 贡献指南

感谢您对秋毫mem（HawkEye Mem）的关注！我们欢迎各种形式的贡献。

## 报告 Bug

如果你发现了 Bug，请提交 Issue，并包含以下信息：

- 操作系统版本与架构（`uname -a`）
- Rust 版本（`rustc --version`）
- 秋毫mem 版本（`hawk-eye-mem --version`）
- 完整的复现步骤
- 实际输出与期望输出

## 提交 Pull Request

1. Fork 本仓库
2. 从 `main` 分支创建你的功能分支
3. 提交代码，确保通过所有测试
4. 提交 Pull Request 到 `main` 分支
5. 等待人工审查

### 代码要求

- 所有测试必须通过：`cargo test`
- 无 clippy 警告：`cargo clippy -- -D warnings`
- 保持完全同步架构，禁止引入 tokio 等异步运行时
- 新增功能需附带单元测试或集成测试

### AI 辅助编码声明

本项目接受 AI 辅助编码（如 DeepSeek-TUI、Reasonix、Cursor 等工具），但所有合并的代码均**经过人工审查**。提交 PR 时无需特别声明是否使用了 AI 工具。

## 开发环境

```bash
# 克隆
git clone https://github.com/qiuhaomem/-HawkEye-Mem.git
cd -HawkEye-Mem

# 编译
cargo build

# 测试
cargo test

# 运行
cargo run -- --json
```

## 代码风格

- 遵循 Rust 标准代码风格（`rustfmt`）
- 使用 `thiserror` 定义错误类型
- 使用 `anyhow` 处理应用层错误
- 平台特定代码使用 `#[cfg(target_os = "...")]` 条件编译
- `#[cfg_attr(not(target_os = "..."), allow(dead_code))]` 处理跨平台死代码警告

## 许可证

贡献即代表您同意将代码以 [Apache-2.0](./LICENSE) 协议授权。

---

秋毫mem — 内存洞察，秋毫不放。
