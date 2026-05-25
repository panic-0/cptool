# 题包 YAML

`problem.yaml` 描述程序、generator、数据组、task pass/fail、validator 和 checker。

阅读已有题包时，先运行：

```bash
cptool pkg explain -w <problem_dir>
cptool pkg explain -w <problem_dir> --json
```

它会按 roles、programs、official data、expect checks 和 handwritten inputs 分层展示已解析配置；多 generator 的 bundle 或 task 会列出实际使用到的全部 generator。

## 示例

```yaml
name: a_plus_b
time_limit_secs: 3.0
memory_limit_mb: 512.0
cpp_compile_args: [-O2, -std=c++20]
programs:
  gen: ./src/gen.cpp
  std:
    info: !cpp
      path: ./src/std.cpp
    time_limit_secs: 1.0
  brute: ./src/brute.cpp
  wrong: ./src/wrong.cpp
  val: ./src/val.cpp
solution: std
validator: val
generator: gen
output:
  allow_empty: false
test:
  type: min
  bundles:
    sample:
      - [20]
    main:
      - [10]
      - [10000000]
      - [1000000000]
    corner:
      cases:
      - {generator: ":file", args: [fixtures/input/small.in]}
  tasks:
  - name: sample
    score: 1.0
    bundles: [sample]
  - name: main
    score: 99.0
    bundles: [main, corner]
    dependencies: [sample]
    pass: [brute]
    fail: [wrong]
  - name: proof-only
    cases:
    - [10, 1..100]
    fail: [wrong]
```

## 程序

程序可以直接写路径简写：`.cpp`、`.cc`、`.cxx` 会推断为 C++，`.py` 会推断为 Python。省略的运行限制继承顶层 `time_limit_secs` 和 `memory_limit_mb`；省略的 C++ 编译参数继承顶层 `cpp_compile_args`。

需要自定义 `compile_args`、`extra_args`、运行限制，或需要 `!command` 时，使用完整形式：

```yaml
std:
  info: !cpp
    path: ./src/std.cpp
  time_limit_secs: 1.0
```

C++ generator 应包含 `testlib.h`，调用 `registerGen(argc, argv, 1)`，用 `opt<T>(index)` 解析固定参数，并使用 `rnd`/`println`。`registerGen` 会根据完整命令行唯一设定 `rnd` 种子，因此 generator 不应读取 seed 参数再手动 seed `std::mt19937`。

C++ 编译会自动把源码所在目录加入 include path，因此源码旁边的 `#include "common.hpp"` 可以正常工作。Windows 下，除非已经显式配置，实际编译参数还会包含 `-static`。

## Generator 和用例

顶层 `generator` 被 bundle case 继承。普通 bundle 可以省略 `cases`，直接写 case 列表。使用默认 generator 的测试 case 应写一行 args-only 简写（`- [...]`）。只有 bundle 需要自己的默认 generator，或某个 case 需要不同 generator 时，才写一行 inline mapping：

```yaml
- {generator: other_gen, args: [100]}
```

bundle 级 `generator` 会覆盖顶层 generator，case 级 `generator` 会覆盖前两者。参数 `L..R` 会展开为整数闭区间；多个 range 参数会做笛卡尔积展开。旧 `"{L:R}"` range 仍可读取，重写 YAML 时会输出为 `L..R`。

保留 generator 名 `:file` 用于把手写输入 fixture 复制到正式数据。它只接受一个位于 `fixtures/input/` 下的 `.in` 路径。

## Task

声明了 `test.type` 时，有 `score` 的正式 task 可以省略 `type`。task 级 `type` 会覆盖该默认值。

有 `score` 的 task 会落盘、导出并参与分值；没有 `score` 的 task 是 verify-only，只由 `test expect` 运行，不进入正式数据。`pass` 中的程序必须匹配 `solution`，`fail` 中的程序至少要在该 task 的一个 case 上失败。注册在 `programs` 中但不是 solution/validator/checker/generator 的程序，必须出现在至少一个 `pass` 或 `fail` 中。

verify-only task 可以直接写内联 `cases`，不用为了对拍数据单独声明 bundle。内联 cases 不会落盘，也不会进入导出包。正式 task 必须用 `bundles` 引用正式数据，不能写内联 `cases`；同一个 task 不能同时写 `bundles` 和 `cases`。

旧 `stress.plans` 会在读取时迁移到 task pass/fail；新题包不要再编写 `stress.plans`。
