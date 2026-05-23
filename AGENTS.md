# AGENTS.md

本仓库是一个独立完整的 Rust 命令行工具，用于算法竞赛题包管理、数据生成、校验、检查器运行、对拍、报告和导出。

## 工作原则

- 改动只围绕本仓库展开，不依赖父级工作区、外部封装脚本或其它自动化约定。
- 优先沿用现有模块、schema 类型和命令结构。`problem.yaml` 支持带 tag 的 YAML 值，修改题包语义时应走已有 serde/schema 路径。
- 尊重用户手写文件。只有命令明确拥有的生成数据、缓存、失败样例或报告输出，才可以由工具自动改写。
- 保持 CLI 输出稳定。`--json` 模式下 stdout 应是可直接解析的 JSON，不能混入进度文本或调试输出。
- 搜索代码优先使用 `rg`；修改命令行为前先读相邻实现和已有测试。

## 代码地图

- `src/cli/args.rs`：clap 命令结构和 help 文案。
- `src/tool/schema.rs`：题包 schema、默认值和反序列化行为。
- `src/tool/package.rs`：`pkg init` 脚手架生成。
- `src/tool/add.rs`：`add` 行为和源码脚手架。
- `src/tool/data.rs`：正式数据生成、暂存和提交流程。
- `src/tool/fixture.rs`：手写 input、validator、checker fixture 的添加、检查和列表。
- `src/tool/run.rs`：程序运行以及可选输出/报告写入。
- `src/tool/stress.rs`、`src/tool/stress_plan.rs`、`src/tool/stress_args.rs`：临时对拍和配置化 stress plan。
- `src/tool/check/`：题包检查、YAML 形状检查、题面和样例审计。
- `src/tool/report/`：证据聚合和渲染。
- `example/`：smoke 测试使用的可执行示例题包。
- `tests/`：通过编译后的 `cptool` 二进制驱动的集成测试。

## 验证

每次更新代码后，提交、交付或声称完成前，**必须**在仓库根目录运行：

```powershell
python scripts/check.py
```

这是唯一的标准本地完成检查入口，会依次运行格式检查、clippy 和完整 Rust 测试套件。只跑 `cargo test`、单个测试文件、手工命令或局部检查都不能替代 `python scripts/check.py`，因为它们可能漏掉格式或 clippy 问题。

迭代时可以先跑更窄的命令来缩短反馈，但最后仍必须回到完整脚本：

```powershell
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

如果改动涉及示例题包、初始化、数据生成、stress plan 或 report evidence，应补充或更新 `tests/` 下对应集成测试，并先运行相关测试文件，再跑完整检查。

## 题包语义

- `pkg init` 生成自包含题包脚手架，包括源码、数据和测试 fixture 目录。
- 顶层默认值，例如运行限制、C++ 编译参数和 `generator`，应由 test cases 和 stress plans 复用；只有确实需要局部差异时才写 override。
- 保留 generator 名 `:file` 表示从题包相对路径读取手写输入 fixture；它不是注册在 `programs` 里的程序。
- 显式 validator 测试和复制 fixture 的生成路径默认会规范化输入换行。源 fixture 不应被回写，除非命令文档明确说明会这样做。
- `pkg clean` 只面向生成数据和本地缓存。validator、checker、corner、failure examples 等长期 fixture 目录不应被清理，除非命令明确拥有它们。
- stress 参数占位符只支持 `{case}` 和 `{case0}`。

## 测试建议

- 纯逻辑优先写就近单元测试；CLI 行为、文件系统副作用、生成文件和诊断信息使用集成测试覆盖。
- 会修改题包的命令应同时覆盖成功路径和失败路径。
- fixture 型测试应保持数据规模小、行为确定。
- 修改 JSON 报告时，断言稳定字段和 issue code，不只检查文本输出。
- 修改 C++ 脚手架行为时，必要时同时用 Rust 单测和 smoke/contract 风格集成测试保护。

## 发布

- 包版本写在 `Cargo.toml` 和 `Cargo.lock`。
- 发布资产由 `scripts/build_release.py` 构建。
- 发布脚本是 `scripts/release.ps1`；它要求工作树干净，并且传入版本与 `Cargo.toml` 一致。
- 不要在 detached HEAD 上打 tag 或发布。
