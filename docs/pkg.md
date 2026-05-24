# 题包命令

`pkg` 命令负责题包生命周期、健康检查、清理和导出。

## 初始化

```bash
./cptool pkg init a_plus_b
./cptool pkg init p_agent45 --root ./problems
```

`--root` 是直接接收题包的目录。默认情况下，`pkg init` 会创建 `./<slug>`；传 `--root ./problems` 时创建 `./problems/<slug>`，不会自动追加额外的 `problems/`。

脚手架包含 `problem.yaml`、`statement.md`、`editorial.md`、`src/`、`data/`、`fixtures/`、`.cptool/failures/` 和题包 `.gitignore`。新题包包含自带的 testlib 模板，并写入顶层默认配置：

```yaml
time_limit_secs: 3.0
memory_limit_mb: 512.0
cpp_compile_args: [-O2, -std=c++20]
```

## 检查

```bash
./cptool pkg check -w ./example/a_plus_b
./cptool pkg check -w ./example/a_plus_b --json
./cptool pkg check -w ./example/a_plus_b --json --wait-for-generation-lock 10
```

`pkg check` 会检查题包结构、未知 `problem.yaml` 字段、程序路径、validator 声明、task 和 bundle 覆盖、生成数据完整性与陈旧文件、stress plan 结构、样例生成和样例输出。

如果数据正在并发生成，它会报告 `data_generation_in_progress`，而不是读取可能不完整的 `data/`。JSON 模式下，该 issue 包含 `kind: "lock"`、`transient: true` 和 `retry_after: "wait_for_generation_then_retry"`。

生成数据缺失时，issue 会给出 `next_action`，通常是：

```bash
cptool case gen -w <problem_dir>
```

## 清理

```bash
./cptool pkg clean -w ./example/a_plus_b
./cptool pkg clean -w ./example/a_plus_b --data
./cptool pkg clean -w ./example/a_plus_b --cache
./cptool pkg clean -w ./example/a_plus_b --json
```

不带额外参数时，`pkg clean` 会删除生成数据文件、`.cptool/cache` 和 `.cptool/failures`。用 `--data` 或 `--cache` 可以只清理一侧。如果存在生成锁或 staging 目录，它会拒绝清理数据。

`pkg clean` 不会删除长期 fixture 目录。

## 导出

```bash
./cptool pkg export -w ./example/a_plus_b --oj syzoj
```

目前只支持 Syzoj 导出，且该能力尚未完全完善。
