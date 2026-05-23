# 内置 Testlib Checker

`cptool add checker <name> --builtin <id>` 会把 `third_party/testlib/checkers/<id>.cpp` 复制进题包并注册到 `problem.yaml`。复制后的文件开头会写来源注释，并使用题包内的 `#include "testlib.h"`。

内置列表由构建脚本扫描 `third_party/testlib/checkers/*.cpp` 自动生成。下表按当前源码行为描述；不要只凭名字猜语义。特别注意：`caseicmp`、`casencmp`、`casewcmp` 的 `case` 指输出形如 `Case k:` 的多测试格式，不表示大小写不敏感。

| id | 源文件 | 实际比较语义 | 适用场景 | 主要注意事项 |
|---|---|---|---|---|
| `acmp` | `acmp.cpp` | 只比较第一个 double，绝对误差 `1.5e-6` | 单个实数答案，只要求绝对误差 | 不比较序列；额外输出不会被它主动检查 |
| `caseicmp` | `caseicmp.cpp` | 比较若干行 `Case k: <int64>`，`Case` 和编号格式必须精确 | 多测试，每个 case 一个整数答案 | 不是大小写不敏感；不适合普通无 `Case k:` 输出 |
| `casencmp` | `casencmp.cpp` | 比较若干段 `Case k: <int64...>` 的整数序列 | 多测试，每个 case 一串整数 | token `Case` 被当作下一段开始；不适合任意文本 token |
| `casewcmp` | `casewcmp.cpp` | 比较若干段 `Case k: <token...>` 的 token 序列 | 多测试，每个 case 一串普通 token | token 本身不能是 `Case`；大小写敏感 |
| `dcmp` | `dcmp.cpp` | 只比较第一个 double，绝对或相对误差 `1e-6` | 单个实数答案，题面接受相对误差 | 不比较序列；额外输出不会被它主动检查 |
| `fcmp` | `fcmp.cpp` | 逐行完整字符串比较 | 行内容必须完全一致的传统文件比较 | 行内空格也有意义；不是 token 比较；可能不主动拒绝额外行 |
| `hcmp` | `hcmp.cpp` | 比较单个规范十进制大整数 token | 超出 `int64` 的单个整数答案 | 只接受 `0` 或无前导零的带符号整数；不是十六进制 |
| `icmp` | `icmp.cpp` | 只比较第一个 32-bit `int` | 单个普通整数答案 | 与 `ncmp` 的关键区别：`icmp` 只读一个 `int`，不检查完整整数序列 |
| `lcmp` | `lcmp.cpp` | 逐行比较行内 token 序列 | 行划分有意义、行内空白不重要 | token 不能跨行重排；可能不主动拒绝额外行 |
| `ncmp` | `ncmp.cpp` | 比较完整有序 `int64` 序列，并检查两边长度 | 一个或多个整数 token 的标准输出 | 与 `icmp` 的关键区别：`ncmp` 读到 EOF，适合整数序列 |
| `nyesno` | `nyesno.cpp` | 比较完整 YES/NO token 序列，大小写不敏感，并检查长度 | 多个 YES/NO 答案 | 单个 YES/NO 用 `yesno` 更直观；题面要求精确大写时不要用 |
| `pointscmp` | `pointscmp.cpp` | 读一个 double，按 `fabs(ans - out)` 调 `quitp` 给分 | 测试 partial score checker 接口 | 示例性质；普通 ACM/ICPC 式题目不要用 |
| `pointsinfo` | `pointsinfo.cpp` | 读输出 double 和答案 double，调 `quitpi` 返回 points_info | 测试 points_info 接口 | 示例性质；不是常规正确性 checker |
| `rcmp` | `rcmp.cpp` | 只比较第一个 double，绝对误差 `1.5e-6` | 单个实数答案，只要求绝对误差 | 名字像相对误差，但源码是单值绝对误差 |
| `rcmp4` | `rcmp4.cpp` | 比较答案中的 double 序列，绝对或相对误差 `1e-4` | 多个实数 token，误差 `1e-4` | 以答案 EOF 为准读取；确认题面误差匹配 |
| `rcmp6` | `rcmp6.cpp` | 比较答案中的 double 序列，绝对或相对误差 `1e-6` | 多个实数 token，误差 `1e-6` | 以答案 EOF 为准读取；确认题面误差匹配 |
| `rcmp9` | `rcmp9.cpp` | 比较答案中的 double 序列，绝对或相对误差 `1e-9` | 多个实数 token，误差 `1e-9` | 精度要求高；确认标准解和输出格式能稳定达到 |
| `rncmp` | `rncmp.cpp` | 比较答案中的 double 序列，绝对误差 `1.5e-5` | 多个实数 token，只要求绝对误差 | 不适合相对误差题；确认误差宽度 |
| `uncmp` | `uncmp.cpp` | 比较无序 `int64` 多重集，并检查数量 | 整数输出顺序任意，但元素多重集唯一 | 不保留行结构；不适合非整数或构造合法性验证 |
| `wcmp` | `wcmp.cpp` | 比较完整 token 序列，大小写敏感，并检查两边 EOF | 普通唯一输出、字符串/整数 token | `YES` 和 `Yes` 不同；浮点误差题不要用 |
| `yesno` | `yesno.cpp` | 只比较第一个 YES/NO token，大小写不敏感 | 单个 YES/NO 答案且题面允许任意大小写 | 不检查 YES/NO 序列；题面要求精确大写时不要用 |

常用选择：

- 单个整数：`icmp`；多个整数：`ncmp`；整数顺序任意：`uncmp`。
- 普通 token 序列：`wcmp`；行结构重要但行内空白不重要：`lcmp`；整行必须完全一致：`fcmp`。
- 单个 YES/NO 且允许大小写不敏感：`yesno`；多个 YES/NO：`nyesno`。
- 浮点：单值看 `acmp`/`dcmp`/`rcmp`，序列看 `rcmp4`/`rcmp6`/`rcmp9`/`rncmp`，必须按题面误差选择。
- 答案不唯一、需要验证构造合法性、需要图/排列/区间等语义检查时，写专用 checker。
