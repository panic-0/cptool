# 题包 YAML

`problem.yaml` 描述程序、generator、正式数据、task、validator、checker 和 stress plan。

## 示例

```yaml
name: a_plus_b
time_limit_secs: 3.0
memory_limit_mb: 512.0
cpp_compile_args: [-O2, -std=c++20]
programs:
  gen:
    info: !cpp
      path: ./src/gen.cpp
  std:
    info: !cpp
      path: ./src/std.cpp
    time_limit_secs: 1.0
  val:
    info: !cpp
      path: ./src/val.cpp
solution: std
validator: val
generator: gen
output:
  allow_empty: false
test:
  type: min
  bundles:
    sample:
      cases:
      - [20]
    main:
      cases:
      - [10]
      - args: [10000000]
      - [1000000000]
    corner:
      cases:
      - generator: :file
        args: [fixtures/input/small.in]
  tasks:
  - name: sample
    score: 1.0
    bundles: [sample]
  - name: main
    score: 99.0
    bundles: [main, corner]
    dependencies: [sample]
stress:
  plans:
  - name: small
    args: ["10", "{case}"]
    against: [std, brute]
    cases: 100
```

## 程序

程序可以使用 `!cpp`、`!command` 或 `!python`。省略的运行限制继承顶层 `time_limit_secs` 和 `memory_limit_mb`；省略的 C++ 编译参数继承顶层 `cpp_compile_args`。

C++ generator 应包含 `testlib.h`，调用 `registerGen(argc, argv, 1)`，用 `opt<T>(index)` 解析固定参数，并使用 `rnd`/`println`。`registerGen` 会根据完整命令行唯一设定 `rnd` 种子，因此 generator 不应读取 seed 参数再手动 seed `std::mt19937`。

C++ 编译会自动把源码所在目录加入 include path，因此源码旁边的 `#include "common.hpp"` 可以正常工作。Windows 下，除非已经显式配置，实际编译参数还会包含 `-static`。

## Generator 和用例

顶层 `generator` 被正式数据 case 和 stress plan 共同继承。使用默认 generator 的测试 case 应优先写 args-only 简写（`- [...]`）或 args-only mapping（`args: [...]`）。只有某个 case 需要不同 generator 时，才写完整形式：

```yaml
- generator: other_gen
  args: [100]
```

bundle 级 `generator` 会覆盖顶层 generator，case 级 `generator` 会覆盖前两者。

保留 generator 名 `:file` 用于把手写输入 fixture 复制到正式数据。它只接受一个位于 `fixtures/input/` 下的 `.in` 路径。stress plan 不能使用 `:file`。

## Task

声明了 `test.type` 时，单个 task 可以省略 `type`。task 级 `type` 会覆盖该默认值。

## Stress 计划

Stress 计划可以省略 `generator`，此时使用顶层默认 generator。`against` 必须正好包含两份程序或源码。plan 参数支持 `{case}`（从 1 开始）和 `{case0}`（从 0 开始）。plan 默认 `expect: pass`；用 `expect: fail` 记录 wrong 程序证据。
