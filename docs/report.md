# 报告命令

`report` 命令用于收集常见题包检查证据。

## 证据

```bash
./cptool report evidence -w ./example/a_plus_b --json
./cptool report evidence -w ./example/a_plus_b --markdown
./cptool report evidence -w ./example/a_plus_b --json --wait-for-generation-lock 10
./cptool report evidence -w ./example/a_plus_b --json --reuse-existing-stress-plan ./stress-plan-summary.json
```

`report evidence` 会聚合：

- cptool 版本
- `pkg check`
- `case gen --summary-only`
- `test plan --summary-only`

如果题包有意不能运行某一部分，使用 `--skip-gen` 或 `--skip-stress-plan`。使用 `--out <path>` 可以把与 stdout 相同的报告内容写入 sidecar 文件。

恢复或审计重跑时，可以传 `--reuse-existing-stress-plan <PATH>`，复用之前由 `test plan --summary-only --json` 生成的 JSON。
