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

## 临时 Batch

```bash
./cptool test batch -w ./example/a_plus_b --generator gen --pass brute -- 10 "{1:100}"
./cptool test batch -w ./example/a_plus_b --generator gen --fail wrong -- 10 "{1:100}"
```

`test batch` 生成临时输入并运行临时 expect。它不运行正式 bundle，也不把数据写入 `data/`。默认参考答案是 `solution`，也可以用 `--answer PROGRAM` 覆盖。

`--` 后的参数支持完整字符串 range：`"{L:R}"`。多个 range 做笛卡尔积展开；不含 range 的参数只生成一个临时 case。

同一组 batch/expect 失败产物使用稳定文件名前缀；重复运行同一组检查时，只保留该组最新一次运行观察到的第一个失败样例。

## Expect 检查

```bash
./cptool test expect -w ./example/a_plus_b --name small
./cptool test expect -w ./example/a_plus_b --summary-only
./cptool test expect -w ./example/a_plus_b --summary-only --json
./cptool test expect -w ./example/a_plus_b --wait-for-generation-lock 10
```

`test expect` 运行 `problem.yaml` 中 `test.tasks[].pass` 和 `test.tasks[].fail`。有 `score` 的 task 仍然是正式数据；无 `score` 的 task 是 verify-only，不落盘、不导出。verify-only task 可以直接写 `cases`，这些临时 case 只服务于 `test expect`。

`fail` 在至少观察到一个 WA/RE/TLE/OLE/UKE 时成功，并报告 `failed_cases`、`passed_cases` 和 `failure_ratio`。旧 `stress.plans` 会在读取时迁移到 task pass/fail。
