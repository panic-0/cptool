# 配置命令

`add` 命令用于注册题包条目，避免手工编辑 `problem.yaml`。

```bash
./cptool add program wrong_overflow -w ./example/a_plus_b
./cptool add bundle main -w ./example/a_plus_b --generator gen --case 10 --case 100
./cptool add task main -w ./example/a_plus_b --score 100 --bundle main
./cptool add validator val -w ./example/a_plus_b
./cptool add checker chk -w ./example/a_plus_b
./cptool add checker chk -w ./example/a_plus_b --builtin wcmp
```

## 程序

`add program` 注册已有源码。不传 `--path` 时，它会在下列位置中识别唯一存在的文件：

- `src/<name>.cpp`
- `src/<name>.py`
- `src/<name>`

传 `--path` 时，指定的源码文件必须已经存在。程序的运行限制和 C++ 编译参数默认继承 `problem.yaml` 顶层配置，除非在单个 program 上显式覆盖。

## Validator

`add validator` 会注册 `validator: <name>` 和对应的已有程序源码。如果匹配的 program 已经存在，它只补顶层 validator 字段。显式 validator 测试通常应使用 fixture 命令，不要手工执行本地可执行文件。

## Checker

`add checker` 会注册 `checker: <name>` 和对应程序。传 `--builtin <id>` 时，它会把内置 testlib checker 复制到 `src/<name>.cpp`。不传 `--builtin` 时，它遵循与 `add program` 相同的源码识别规则。

修改默认 checker 前，先阅读[内置 checker](builtin-checkers.md)。
