# CP Tool

CP Tool is a command line tool for competitive programming.

## Usage

```bash
# create a cptool/autocpp problem package
./cptool pkg init a_plus_b

# --root accepts either a workspace root or the problems/ directory itself
./cptool pkg init p_agent45 --root ./problems

# generate official data into ./example/a_plus_b/data
./cptool case gen -w ./example/a_plus_b

# generate official data and print one compact audit summary
./cptool case gen -w ./example/a_plus_b --summary-only

# print machine-readable audit evidence
./cptool case gen -w ./example/a_plus_b --summary-only --json

# clean selected stale data before publishing newly generated data
./cptool case gen -w ./example/a_plus_b --bundle main --clean

# clean generated data and local cptool cache
./cptool pkg clean -w ./example/a_plus_b
./cptool pkg clean -w ./example/a_plus_b --data
./cptool pkg clean -w ./example/a_plus_b --cache

# wait up to 10 seconds for an in-progress generation lock
./cptool case gen -w ./example/a_plus_b --wait-for-generation-lock 10

# generate with a custom per-program stdout/stderr limit; default is 32 MiB
./cptool case gen -w ./example/a_plus_b --output-limit-bytes 67108864

# run a configured program on a generated bundle case
./cptool case run std sample[0] -w ./example/a_plus_b
./cptool case run std sample[0] -w ./example/a_plus_b --wait-for-generation-lock 10

# register package entries without hand-editing problem.yaml
./cptool config add program wrong_overflow -w ./example/a_plus_b
./cptool config add validator val -w ./example/a_plus_b
./cptool config add checker chk -w ./example/a_plus_b
./cptool config add checker chk -w ./example/a_plus_b --builtin wcmp

# run validator and checker sanity tests on explicit local files
./cptool test validator -w ./example/a_plus_b --input-path ./data/sample-001.in
./cptool test validator -w ./example/a_plus_b --input-path ./tests/validator/bad.in --expect fail --no-fix-line-endings
./cptool test checker -w ./example/a_plus_b --input-path ./data/sample-001.in --output-path ./tmp/std.out --answer-path ./data/sample-001.ans

# print only a compact run summary for large outputs
./cptool case run std sample[0] -w ./example/a_plus_b --summary-only

# print a machine-readable run summary
./cptool case run std sample[0] -w ./example/a_plus_b --json

# temporarily override configured run limits while debugging
./cptool case run std sample[0] -w ./example/a_plus_b --time-limit-secs 5 --memory-limit-mb 1024

# stress test programs with generated temporary inputs
./cptool test stress -w ./example/a_plus_b --generator gen --against std --against brute --cases 100 -- 10
./cptool test stress -w ./example/a_plus_b --generator gen --against std --against brute --cases 100 -- {seed} {case}
./cptool test stress -w ./example/a_plus_b --generator '$file' --against std --against brute --cases 1 -- tests/corner/small.in

# run stress plans declared in problem.yaml
./cptool test plan -w ./example/a_plus_b --name small
./cptool test plan -w ./example/a_plus_b --summary-only
./cptool test plan -w ./example/a_plus_b --summary-only --json
./cptool test plan -w ./example/a_plus_b --positive-only --summary-only --json
./cptool test plan -w ./example/a_plus_b --negative-only --summary-only --json
./cptool test plan -w ./example/a_plus_b --wait-for-generation-lock 10

# check common package structure and generated data issues
./cptool pkg check -w ./example/a_plus_b
./cptool pkg check -w ./example/a_plus_b --json
./cptool pkg check -w ./example/a_plus_b --json --wait-for-generation-lock 10

# collect check, generation, and stress-plan evidence
./cptool report evidence -w ./example/a_plus_b --json
./cptool report evidence -w ./example/a_plus_b --json --wait-for-generation-lock 10
./cptool report evidence -w ./example/a_plus_b --json --reuse-existing-stress-plan ./stress-plan-summary.json

# export problem to online judge format; currently only support syzoj
./cptool pkg export -w ./example/a_plus_b --oj syzoj

# for more information
./cptool --help
```

`problem.yaml` is the problem description file. The `a_plus_b` example uses:

```yaml
name: a_plus_b
time_limit_secs: 3.0
memory_limit_mb: 512.0
cpp_compile_args: [-O2, -std=c++14, -I../assets/testlib/]
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
      - generator: "$file"
        args: [tests/corner/small.in]
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
    args: ["10", "{seed}", "{case}"]
    against: [std, brute]
    cases: 100
    seed_base: 20260519
  - name: corner-file
    generator: "$file"
    args: ["tests/corner/{case}.in"]
    against: [std, brute]
    cases: 3
```

Programs can also use `!command` or `!python`; omitted program limits inherit top-level `time_limit_secs` and `memory_limit_mb`, and omitted C++ compile args inherit top-level `cpp_compile_args`. Individual programs can override any inherited value, such as the `std` time limit above. The top-level `generator` is shared by official data cases and stress plans. Test cases should use args-only shorthand (`- [...]`) or args-only mapping (`args: [...]`) when they use the default generator; write full form (`generator: other_gen`, `args: [...]`) only when a case needs a different generator. A bundle-level `generator` overrides the top-level generator for that bundle, and a full case `generator` overrides both defaults. Stress plans may omit `generator` to use the top-level default. Tasks can omit `type` when `test.type` is declared; a task-level `type` overrides that default. The reserved generator name `$file` copies a hand-written input fixture instead of running a program; pass exactly one path argument, resolved relative to the package directory. In PowerShell CLI commands, quote it as `'$file'` so it is not expanded as a variable. On Windows, cptool adds `-static` to effective C++ compile args so cached executables do not depend on MinGW runtime DLLs at run time.

## Notes

+ Syzoj export is not fully supported yet.
+ `--version` prints the package version and the git commit embedded at build time, for example `cptool 0.9.0 (commit abc1234)`; local builds from a modified checkout append `-dirty`.
+ Commands are grouped by task: `pkg` manages lifecycle/check/clean/export, `config add` edits package configuration and simple source scaffolds, `case` generates official data and runs programs, `test` runs validator/checker/stress workflows, and `report` collects evidence.
+ `pkg init` creates only the cptool-managed scaffold: `problem.yaml`, `statement.md`, `editorial.md`, `src/`, `data/`, `tests/failures/`, and a package `.gitignore`. By default `--root DIR` creates `DIR/problems/<slug>`; when `DIR` is already named `problems`, it creates `DIR/<slug>` instead to avoid accidental `problems/problems/<slug>` scaffolds. The scaffold writes top-level defaults `time_limit_secs: 3.0`, `memory_limit_mb: 512.0`, and `cpp_compile_args: [-O2, -std=c++20]`; add per-program limits or `compile_args` in `problem.yaml` only when a package needs tighter, looser, or program-specific settings. New packages include a self-contained `src/testlib.h`, a testlib `src/gen.cpp` placeholder, and a testlib `src/val.cpp` placeholder; update both placeholders to match the problem before publishing data.
+ C++ generators should include `testlib.h`, call `registerGen(argc, argv, 1)`, parse fixed arguments with `opt<T>(index)`, and use `rnd`/`println`. `registerGen` seeds `rnd` uniquely from the full command line, so generators should not read a seed argument and reseed `std::mt19937` by hand.
+ `config add validator` registers `validator: <name>` and a matching program. It follows the same source detection as `config add program`: use `src/<name>.cpp`, `src/<name>.py`, or `src/<name>` when exactly one exists, otherwise create an empty `src/<name>.cpp`. If a matching program already exists, it only sets the top-level validator field.
+ `config add checker` registers `checker: <name>` and a matching program. With `--builtin <id>`, it copies a built-in testlib checker to `src/<name>.cpp`. Without `--builtin`, it follows the same source detection as `config add program`: use `src/<name>.cpp`, `src/<name>.py`, or `src/<name>` when exactly one exists, otherwise create an empty `src/<name>.cpp`.
+ `case gen` writes data to `data/` by default. It stages generated files first and moves them into place only after the selected cases succeed. Use `--clean` to remove stale `.in/.ans` files for the selected case, bundle, or known bundles before publishing the newly generated files. Use `--summary-only` to suppress per-file `generated` lines and print cases, bundles, elapsed time, input/answer bytes, and warning counts.
+ `pkg clean` removes generated data and local cache without running generation. With no flags it removes `data/*.in`, `data/*.ans`, and `.cptool/cache`; use `--data` or `--cache` to target only one side. It refuses to remove data while a generation lock or staging directory exists.
+ `case gen`, implicit selector generation in `case run`, `pkg check`, `test plan`, and `report evidence` support `--wait-for-generation-lock <SECONDS>`. Pass a positive timeout such as `--wait-for-generation-lock 10` to poll every 250ms while another generation is in progress. The wait mode never deletes stale locks; it times out with a retry/prewarm hint.
+ `case run` uses a bundle case such as `sample[0]` by default, but can also read `--stdin-path` or `--stdin-text`. Use `--summary-only` to suppress full stdout and print size/line/hash fields, or `--hide-stdout` to keep only the status line while still allowing `--stdout-path`.
+ `case run`, `case gen`, `test stress`, `test plan`, and `pkg check` support `--json` for machine-readable evidence. `--json` can be combined with `--summary-only` where that flag exists; JSON mode suppresses progress/raw-output text on stdout so the result can be parsed directly.
+ `case gen` warns when a non-empty input produces an empty answer. Set `output.allow_empty: true` in `problem.yaml` for tasks where empty output is valid. In `--summary-only` mode, `empty_answer` and generator-output warnings are counted in the summary. JSON reports include `validator_configured` and `validator_calls`; validator failures include the bundle, case, staged input path, generator name, and generator args.
+ `test validator` reads `--input-path` files and, by default, normalizes non-native line endings on disk before running the validator. It samples the first and last lines to detect common hand-written fixture issues and emits `warning: input_line_endings_normalized` when it rewrites the file. Pass `--no-fix-line-endings` when you need to verify the exact bytes without rewriting the fixture.
+ `pkg check` reports common structure, unknown `problem.yaml` fields, program paths, validator declaration, task/bundle coverage, generated data completeness/staleness, stress-plan shape, sample generation, and sample output issues. It exits non-zero when errors are found, and reports `data_generation_in_progress` instead of inspecting data during a concurrent `case gen`. In JSON mode this issue includes `kind: "lock"`, `transient: true`, and `retry_after: "wait_for_generation_then_retry"` so orchestrators can wait and retry instead of treating it as a final package failure. Missing generated data is split into `kind: "not_generated"` when no generated `.in/.ans` files exist and `kind: "missing"` when an existing data set is incomplete; those issues include `next_action`, usually `cptool case gen -w <problem_dir> --clean`. If no `validator` is configured, `pkg check` emits `warning: validator_missing`; set `validator_omitted_reason: "..."` in `problem.yaml` when omission is intentional.
+ `report evidence` aggregates the most common audit commands into one report: `pkg check`, `case gen` summary, and `test plan` summary, plus the cptool version. Use `--json` for machine-readable reports; use `--wait-for-generation-lock <SECONDS>` to pass the same wait timeout through the internal check, generation, and stress-plan steps; use `--skip-gen` or `--skip-stress-plan` when a package intentionally cannot run that section. For recovery or audit reruns, pass `--reuse-existing-stress-plan <PATH>` with JSON previously produced by `test plan --summary-only --json` to fill the stress-plan section without rerunning plans or creating new failure artifacts.
+ `test stress` is for ad-hoc correctness checks. It does not run official bundles and does not assume `brute` is safe on large data. Arguments after `--` support `{seed}`, `{case}` (1-based), and `{case0}` (0-based); fixed args without placeholders are passed literally for every case. `{seed}` is derived deterministically from the stress command and case number. The final summary includes `unique_input_hashes=N`, which is useful for noticing fixed-argument repeated inputs. If `cases > 1` but all generated inputs have the same hash, `stress` still passes but prints `warning: repeated_input ... random_coverage=false`; JSON warnings include `random_coverage: false`. Multiple `cptool test stress` processes can run against the same package concurrently; compile cache hits are reused and failure files are created atomically. If all compared programs succeed but all stdout streams are empty on a non-empty generated input, `stress` still passes but prints `warning: all_empty_output`.
+ `test plan` runs `stress.plans` from `problem.yaml`. Plan args support `{seed}`, `{case}` (1-based), and `{case0}` (0-based); `{seed}` is derived deterministically from the plan name, case number, and optional `seed_base`. Plans default to `expect: pass`; use `expect: fail` for wrong-program evidence. Negative plans run every case, succeed when at least one `wrong_answer` or `program_failed` is observed, save the first failure artifacts, and report `failed_cases`, `passed_cases`, and `failure_ratio`. Use `--positive-only` to run only positive `expect: pass` plans, or `--negative-only` to run only negative `expect: fail` plans. Use `--summary-only` to print one compact line per plan, including unique input hashes, empty stdout case counts, and warning counts.
+ C++ compilation automatically adds the source file's directory to the include path, so `#include "common.hpp"` works for headers beside the source even though cptool compiles a cached copy. On Windows, effective compile args also include `-static` unless already present, avoiding MinGW runtime DLL lookup failures in CI and other clean environments. Compile failures include compiler path/version, flags, static-link status, cache key, and cache exe path. Windows runtime errors for common NTSTATUS exit codes include a diagnostic hint in failure reports.
+ `case run`, `case gen`, `test stress`, and `test plan` default to a 32 MiB per-program stdout/stderr limit; pass `--output-limit-bytes` to override it where supported.

## Development Checks

Before considering any cptool change complete, run the cptool script-level full check from the cptool repository root:

```powershell
python scripts/check.py
```

This is the canonical local verification entrypoint. It runs formatting, clippy, and the full Rust test suite in order, stopping at the first failure. Do not treat a change as verified after only running individual `cargo` commands unless `python scripts/check.py` has also passed.

## Release

On Windows, publish a GitHub release from a clean checkout with:

```powershell
.\scripts\release.ps1 -Version 0.9.0
```

Replace `0.9.0` with the current `Cargo.toml` version. The script runs `scripts/check.py`, builds release artifacts with `scripts/build_release.py`, pushes the current branch and tag, then creates the GitHub release with the generated archives and `SHA256SUMS.txt`.
