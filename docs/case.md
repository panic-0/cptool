# 用例命令

`case` 命令用于生成正式数据，并在题包输入上运行已注册程序。

## 生成数据

```bash
./cptool case gen -w ./example/a_plus_b
./cptool case gen -w ./example/a_plus_b --bundle main
./cptool case gen -w ./example/a_plus_b --case sample[0]
./cptool case gen -w ./example/a_plus_b --summary-only
./cptool case gen -w ./example/a_plus_b --summary-only --json
./cptool case gen -w ./example/a_plus_b --wait-for-generation-lock 10
./cptool case gen -w ./example/a_plus_b --output-limit-bytes 67108864
```

`case gen` 默认把正式 `.in` 和 `.ans` 数据写到 `data/`。默认选择正式 task 引用的 bundle，并按 task 顺序去重；verify-only inline cases 不会落盘。它会先写入 staging 目录，只有所选用例全部成功后才移动到最终位置。

每次成功生成都会重建输出目录内容，只保留本轮新生成的文件。不要把手写输入放在 `data/`；长期保留的人工输入应放在 `fixtures/input/`，并通过 `:file` case 引用。

常见 warning：

- `generator_output_suspicious`：生成的输入为空。
- `empty_answer`：非空输入得到空答案，且没有启用 `output.allow_empty`。

## 运行程序

```bash
./cptool case run std sample[0] -w ./example/a_plus_b
./cptool case run std sample[0] -w ./example/a_plus_b --summary-only
./cptool case run std sample[0] -w ./example/a_plus_b --json
./cptool case run std sample[0] -w ./example/a_plus_b --wait-for-generation-lock 10
./cptool case run std sample[0] -w ./example/a_plus_b --time-limit-secs 5 --memory-limit-mb 1024
./cptool case run std -w ./example/a_plus_b --stdin-path ./local/input.in
```

不指定 program 时，`case run` 默认运行配置中的 `solution`。输入可以来自 bundle selector、`--stdin-path` 或 `--stdin-text`。输出较大时，用 `--summary-only` 或 `--hide-stdout` 更适合排查。

`case run`、`case gen`、`test batch` 和 `test expect` 默认给每个程序 32 MiB stdout/stderr 捕获上限。支持的命令可以用 `--output-limit-bytes` 覆盖。
