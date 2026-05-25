# 测试资产命令

Fixture 是长期保留的手写测试资产。它们应放在 `fixtures/` 下，不应放在 `data/`。

## 添加测试资产

```bash
./cptool fixture add input small -w ./example/a_plus_b --from ./local/small.in
./cptool fixture add validator pass small -w ./example/a_plus_b --from ./local/small.in
./cptool fixture add validator fail malformed -w ./example/a_plus_b --from ./local/bad.in
./cptool fixture add checker fail mismatch -w ./example/a_plus_b --input ./local/input.in --output ./local/wrong.out --answer ./local/answer.ans
```

输入 fixture 放在 `fixtures/input/`，用于正式 `:file` 数据 case。validator fixture 放在 `fixtures/validator/pass` 或 `fixtures/validator/fail`。checker fixture 放在 `fixtures/checker/pass` 或 `fixtures/checker/fail`，每组使用相同文件名前缀的 `<name>.in`、`<name>.out` 和 `<name>.ans`。

## 检查测试资产

```bash
./cptool fixture check -w ./example/a_plus_b
./cptool fixture check -w ./example/a_plus_b --json
```

`fixture check` 会报告不完整的 checker fixture，以及没有被任何 `:file` case 引用的输入 fixture。

## `:file` 用例

保留 generator 名 `:file` 用于把手写输入 fixture 复制进正式数据。它只接受一个位于 `fixtures/input/` 下的 `.in` 路径，路径按题包目录解析：

```yaml
test:
  bundles:
    corner:
      cases:
      - generator: :file
        args: [fixtures/input/small.in]
```

`:file` 可用于正式 bundle case，也可用于 verify-only task 的内联 `cases`；它不会作为普通 generator 程序注册。
