# Built-in Testlib Checkers

`cptool add checker <name> --builtin <id>` copies a checker from `example/assets/testlib/checkers/<id>.cpp` into the problem package and registers it in `problem.yaml`. The copied file starts with an origin comment and includes the package-local `testlib.h`.

The built-in list is generated at build time by scanning every `.cpp` file in `example/assets/testlib/checkers/`; this document is the agent-facing guide for choosing one safely.

| id | 来源 | 作用 | 适用场景 | 注意事项 |
|---|---|---|---|---|
| `acmp` | testlib `acmp.cpp` | 按双精度浮点序列比较，默认绝对/相对误差约 `1e-6` | 普通浮点答案序列 | 不是精确比较；误差策略不匹配时应写专用 checker |
| `caseicmp` | testlib `caseicmp.cpp` | 大小写不敏感比较整数 token | 整数答案但题面允许大小写变体的特殊输出 | 整数题通常优先用 `icmp`；确认题面确实允许大小写差异 |
| `casencmp` | testlib `casencmp.cpp` | 大小写不敏感比较整数序列 | 纯整数序列且包装文本大小写不重要 | 不适合浮点；普通整数输出优先 `ncmp`/`icmp` |
| `casewcmp` | testlib `casewcmp.cpp` | 大小写不敏感按 token 比较 | 字符串 token 答案且题面允许任意大小写 | 题面要求精确大小写时不要用 |
| `dcmp` | testlib `dcmp.cpp` | 比较 double 值，误差约 `1e-6` | 单个或多个 double 答案 | 与 `rcmp*` 一样属于误差比较，需确认题目误差要求 |
| `fcmp` | testlib `fcmp.cpp` | 文件式 token 比较 | 传统标准答案比较，忽略常规空白差异 | 和 `wcmp` 都是 token 级精确比较；优先按平台/历史约定选择 |
| `hcmp` | testlib `hcmp.cpp` | 十六进制整数比较 | 输出为十六进制数 | 不适合十进制整数或字符串 |
| `icmp` | testlib `icmp.cpp` | 比较整数 token | 单个或多个整数答案 | `icmp` 面向整数 token；`ncmp` 更强调整数序列文件比较，二者都不适合浮点 |
| `lcmp` | testlib `lcmp.cpp` | 按行比较 token 序列 | 行结构有意义但行内空白不重要 | 不适合允许任意重排的输出 |
| `ncmp` | testlib `ncmp.cpp` | 比较整数序列 | 纯整数输出 | 不适合浮点；若答案中有非整数 token 不要用 |
| `nyesno` | testlib `nyesno.cpp` | 比较 NO/YES 风格答案 | 输出为否/是，且需要该 checker 的 NO/YES 约定 | 与 `yesno` 的期望文本顺序不同，使用前检查题面和源码语义 |
| `pointscmp` | testlib `pointscmp.cpp` | 支持部分分/points 输出比较 | 需要 checker 根据输出给分 | 普通 ACM 式题目不要用；确认评测系统支持部分分反馈 |
| `pointsinfo` | testlib `pointsinfo.cpp` | 输出 points 信息的辅助 checker | 调试或部分分信息输出 | 普通标准答案比较不要用 |
| `rcmp` | testlib `rcmp.cpp` | 浮点误差比较，默认相对/绝对误差约 `1e-6` | 浮点题 | 明确题面误差语义；误差不是精确比较 |
| `rcmp4` | testlib `rcmp4.cpp` | 浮点误差比较，约 `1e-4` | 误差要求较宽的浮点题 | 不要用于要求 `1e-6` 或更严的题 |
| `rcmp6` | testlib `rcmp6.cpp` | 浮点误差比较，约 `1e-6` | 常见浮点题 | 需与题面误差一致 |
| `rcmp9` | testlib `rcmp9.cpp` | 浮点误差比较，约 `1e-9` | 高精度浮点题 | 对数值稳定性要求更高；确认正确解可达 |
| `rncmp` | testlib `rncmp.cpp` | 浮点序列比较，误差约 `1e-6` | 多个实数 token 输出 | 不适合整数精确题 |
| `uncmp` | testlib `uncmp.cpp` | 无序 token 集合比较 | 输出顺序任意但 token 多重集应一致 | 不检查结构顺序；行结构有意义时不要用 |
| `wcmp` | testlib `wcmp.cpp` | 按 token 精确比较 | 普通唯一输出、字符串/数字 token | 区分 `YES` 和 `Yes`；浮点题不要用 |
| `yesno` | testlib `yesno.cpp` | 大小写不敏感比较 YES/NO | 题面允许任意大小写的 YES/NO 输出 | 题面要求精确大写时不要用 |

常用选择：

- 唯一标准输出且大小写敏感：`wcmp`。
- 行结构有意义：`lcmp`。
- 纯整数：`icmp` 或 `ncmp`。
- YES/NO 且题面允许任意大小写：`yesno`。
- 浮点：按题面误差选择 `rcmp`、`rcmp6`、`rcmp9` 或写专用 checker。
- 输出顺序任意、答案不唯一、需要验证构造合法性：不要依赖这些简单 checker，写专用 checker。
