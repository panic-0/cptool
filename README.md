# CP Tool

CP Tool 是面向算法竞赛题包的确定性命令行工具。它负责创建题包脚手架、生成正式数据、运行已注册程序、检查题包健康状态、管理 fixture，并收集审计证据。

## 快速开始

```bash
# 创建题包
./cptool pkg init a_plus_b --root ./example

# 生成正式数据到 data/
./cptool case gen -w ./example/a_plus_b

# 在一个已生成用例上运行标准程序
./cptool case run std sample[0] -w ./example/a_plus_b

# 检查题包结构、生成数据、样例和 task expect
./cptool pkg check -w ./example/a_plus_b

# 收集机器可读证据
./cptool report evidence -w ./example/a_plus_b --json

# 查看命令和参数帮助
./cptool --help
./cptool case gen --help
```

每次 `case gen` 成功后都会重建输出目录内容。手写输入必须放在 `fixtures/input/`，再通过 `:file` case 进入正式数据。

## 文档

- [题包命令](docs/pkg.md)：`pkg init`、`pkg check`、`pkg clean` 和 `pkg export`。
- [配置命令](docs/add.md)：`add program`、`add bundle`、`add task`、`add validator` 和 `add checker`。
- [用例命令](docs/case.md)：`case gen` 和 `case run`。
- [测试资产命令](docs/fixtures.md)：`fixture add` 和 `fixture check`。
- [测试命令](docs/test.md)：validator/checker 测试、临时 batch 和 task expect。
- [报告命令](docs/report.md)：证据聚合。
- [题包 YAML](docs/problem-yaml.md)：题包配置、generator、bundle 和 task pass/fail。
- [内置 checker](docs/builtin-checkers.md)：checker 选择和语义。

## 开发检查

任何 cptool 改动完成前，都应在 cptool 仓库根目录运行统一检查入口：

```powershell
python scripts/check.py
```

该脚本会依次运行格式检查、clippy 和完整 Rust 测试套件，遇到第一个失败即停止。

## 发布

Windows 下，从干净 checkout 发布 GitHub release：

```powershell
.\scripts\release.ps1 -Version 0.11.0
```

把 `0.11.0` 替换成当前 `Cargo.toml` 版本。脚本会运行 `scripts/check.py`，用 `scripts/build_release.py` 构建发布产物，推送当前分支和 tag，然后创建 GitHub release，并上传生成的压缩包与 `SHA256SUMS.txt`。
