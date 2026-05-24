# 测试命令

`test` 命令用于测试 validator、checker，以及做程序对比。

## Validator 测试

```bash
./cptool test validator -w ./example/a_plus_b
./cptool test validator -w ./example/a_plus_b --input ./data/sample-0.in
./cptool test validator -w ./example/a_plus_b --json
```

不指定输入时，`test validator` 会运行所有 validator fixture。运行前它会把非本平台换行规范化到磁盘上；普通规范化不作为 warning。需要验证精确字节时，传 `--no-fix-line-endings`。

## Checker 测试

```bash
./cptool test checker -w ./example/a_plus_b
./cptool test checker -w ./example/a_plus_b --input ./data/sample-0.in --output ./tmp/std.out --answer ./data/sample-0.ans
./cptool test checker -w ./example/a_plus_b --json
```

不指定文件组时，`test checker` 会运行所有 checker fixture。显式测试 checker 时，必须同时提供 `--input`、`--output` 和 `--answer`。

## 临时 Stress

```bash
./cptool test stress -w ./example/a_plus_b --generator gen --cases 100 std brute -- 10
./cptool test stress -w ./example/a_plus_b --generator gen --cases 100 std brute -- 10 {case}
```

`test stress` 生成临时输入并比较两份已注册程序或源码。它不运行正式 bundle，也不假设 `brute` 能处理大数据。

`--` 后的参数支持 `{case}`（从 1 开始）和 `{case0}`（从 0 开始）。不含占位符的固定参数会在每个 case 中原样传入。如果多个 case 的生成输入完全相同，stress 仍会通过，但会报告重复输入 warning。

## Stress 计划

```bash
./cptool test plan -w ./example/a_plus_b --name small
./cptool test plan -w ./example/a_plus_b --summary-only
./cptool test plan -w ./example/a_plus_b --summary-only --json
./cptool test plan -w ./example/a_plus_b --positive-only --summary-only --json
./cptool test plan -w ./example/a_plus_b --negative-only --summary-only --json
./cptool test plan -w ./example/a_plus_b --wait-for-generation-lock 10
```

`test plan` 运行 `problem.yaml` 中的 `stress.plans`。plan 默认 `expect: pass`；用 `expect: fail` 记录 wrong 程序证据。负向 plan 在至少观察到一个 `wrong_answer` 或 `program_failed` 时成功，并报告 `failed_cases`、`passed_cases` 和 `failure_ratio`。
