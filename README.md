# CP Tool

CP Tool is a command line tool for competitive programming.

## Usage

```bash
# create a cptool/autocpp problem package
./cptool init a_plus_b

# --root accepts either a workspace root or the problems/ directory itself
./cptool init p_agent45 --root ./problems

# generate official data into ./example/a_plus_b/data
./cptool gen -w ./example/a_plus_b

# generate official data and print one compact audit summary
./cptool gen -w ./example/a_plus_b --summary-only

# print machine-readable audit evidence
./cptool gen -w ./example/a_plus_b --summary-only --json

# clean selected stale data before publishing newly generated data
./cptool gen -w ./example/a_plus_b --bundle main --clean

# wait up to 10 seconds for an in-progress generation lock
./cptool gen -w ./example/a_plus_b --wait-for-generation-lock 10

# generate with a custom per-program stdout/stderr limit; default is 32 MiB
./cptool gen -w ./example/a_plus_b --output-limit-bytes 67108864

# run a configured program on a generated bundle case
./cptool run std sample[0] -w ./example/a_plus_b
./cptool run std sample[0] -w ./example/a_plus_b --wait-for-generation-lock 10

# print only a compact run summary for large outputs
./cptool run std sample[0] -w ./example/a_plus_b --summary-only

# print a machine-readable run summary
./cptool run std sample[0] -w ./example/a_plus_b --json

# stress test programs with generated temporary inputs
./cptool stress -w ./example/a_plus_b --generator gen --against std --against brute --cases 100 -- 10
./cptool stress -w ./example/a_plus_b --generator gen --against std --against brute --cases 100 -- {seed} {case}

# run stress plans declared in problem.yaml
./cptool stress-plan -w ./example/a_plus_b --name small
./cptool stress-plan -w ./example/a_plus_b --summary-only
./cptool stress-plan -w ./example/a_plus_b --summary-only --json
./cptool stress-plan -w ./example/a_plus_b --positive-only --summary-only --json
./cptool stress-plan -w ./example/a_plus_b --negative-only --summary-only --json
./cptool stress-plan -w ./example/a_plus_b --wait-for-generation-lock 10

# check common package structure and generated data issues
./cptool check -w ./example/a_plus_b
./cptool check -w ./example/a_plus_b --json
./cptool check -w ./example/a_plus_b --json --wait-for-generation-lock 10

# collect check, generation, and stress-plan evidence
./cptool evidence -w ./example/a_plus_b --json
./cptool evidence -w ./example/a_plus_b --json --wait-for-generation-lock 10
./cptool evidence -w ./example/a_plus_b --json --reuse-existing-stress-plan ./stress-plan-summary.json

# export problem to online judge format; currently only support syzoj
./cptool export -w ./example/a_plus_b --oj syzoj

# for more information
./cptool --help
```

`problem.yaml` is the problem description file. The `a_plus_b` example uses:

```yaml
name: a_plus_b
programs:
  gen:
    info: !cpp
      path: ./src/gen.cpp
    time_limit_secs: 3.0
    memory_limit_mb: 512.0
  std:
    info: !cpp
      path: ./src/std.cpp
      compile_args: [-O2, -std=c++14]
    time_limit_secs: 3.0
    memory_limit_mb: 512.0
solution: std
output:
  allow_empty: false
test:
  bundles:
    sample:
      cases:
      - generator: gen
        args: [20]
    main:
      cases:
      - generator: gen
        args: [10]
      - generator: gen
        args: [10000000]
      - generator: gen
        args: [1000000000]
  tasks:
  - name: sample
    score: 1.0
    type: min
    bundles: [sample]
  - name: main
    score: 99.0
    type: min
    bundles: [main]
    dependencies: [sample]
stress:
  plans:
  - name: small
    generator: gen
    args: ["10", "{seed}", "{case}"]
    against: [std, brute]
    cases: 100
    seed_base: 20260519
```

Programs can also use `!command` or `!python`; omitted C++ compile args default to C++20 with warnings. On Windows, cptool adds `-static` to effective C++ compile args so cached executables do not depend on MinGW runtime DLLs at run time.

## Notes

+ Syzoj export is not fully supported yet.
+ `--version` prints the package version and the git commit embedded at build time, for example `cptool 0.6.0 (commit abc1234)`; local builds from a modified checkout append `-dirty`.
+ `init` creates only the cptool-managed scaffold: `problem.yaml`, `statement.md`, `editorial.md`, `src/`, `data/`, `tests/failures/`, and a package `.gitignore`. By default `--root DIR` creates `DIR/problems/<slug>`; when `DIR` is already named `problems`, it creates `DIR/<slug>` instead to avoid accidental `problems/problems/<slug>` scaffolds. The scaffold sets `gen`, `std`, and `brute` time limits to 3 seconds; adjust per program in `problem.yaml` when a package needs tighter or looser limits.
+ `gen` writes data to `data/` by default. It stages generated files first and moves them into place only after the selected cases succeed. Use `--clean` to remove stale `.in/.ans` files for the selected case, bundle, or known bundles before publishing the newly generated files. Use `--summary-only` to suppress per-file `generated` lines and print cases, bundles, elapsed time, input/answer bytes, and warning counts.
+ `gen`, implicit selector generation in `run`, `check`, `stress-plan`, and `evidence` support `--wait-for-generation-lock <SECONDS>`. Pass a positive timeout such as `--wait-for-generation-lock 10` to poll every 250ms while another generation is in progress. The wait mode never deletes stale locks; it times out with a retry/prewarm hint.
+ `run` uses a bundle case such as `sample[0]` by default, but can also read `--stdin-path` or `--stdin-text`. Use `--summary-only` to suppress full stdout and print size/line/hash fields, or `--hide-stdout` to keep only the status line while still allowing `--stdout-path`.
+ `run`, `gen`, `stress`, `stress-plan`, and `check` support `--json` for machine-readable evidence. `--json` can be combined with `--summary-only` where that flag exists; JSON mode suppresses progress/raw-output text on stdout so the result can be parsed directly.
+ `gen` warns when a non-empty input produces an empty answer. Set `output.allow_empty: true` in `problem.yaml` for tasks where empty output is valid. In `--summary-only` mode, `empty_answer` and generator-output warnings are counted in the summary. JSON reports include `validator_configured` and `validator_calls`; validator failures include the bundle, case, staged input path, generator name, and generator args.
+ `check` reports common structure, program path, validator declaration, generated data, sample generation, and sample output issues. It exits non-zero when errors are found, and reports `data_generation_in_progress` instead of inspecting data during a concurrent `gen`. In JSON mode this issue includes `kind: "lock"`, `transient: true`, and `retry_after: "wait_for_generation_then_retry"` so orchestrators can wait and retry instead of treating it as a final package failure. If no `validator` is configured, `check` emits `warning: validator_missing`; set `validator_omitted_reason: "..."` in `problem.yaml` when omission is intentional.
+ `evidence` aggregates the most common audit commands into one report: `check`, `gen` summary, and `stress-plan` summary, plus the cptool version. Use `--json` for machine-readable reports; use `--wait-for-generation-lock <SECONDS>` to pass the same wait timeout through the internal check, generation, and stress-plan steps; use `--skip-gen` or `--skip-stress-plan` when a package intentionally cannot run that section. For recovery or audit reruns, pass `--reuse-existing-stress-plan <PATH>` with JSON previously produced by `stress-plan --summary-only --json` to fill the stress-plan section without rerunning plans or creating new failure artifacts.
+ `stress` is for ad-hoc correctness checks. It does not run official bundles and does not assume `brute` is safe on large data. Arguments after `--` support `{seed}`, `{case}` (1-based), and `{case0}` (0-based); fixed args without placeholders are passed literally for every case. `{seed}` is derived deterministically from the stress command and case number. The final summary includes `unique_input_hashes=N`, which is useful for noticing fixed-argument repeated inputs. If `cases > 1` but all generated inputs have the same hash, `stress` still passes but prints `warning: repeated_input ... random_coverage=false`; JSON warnings include `random_coverage: false`. Multiple `cptool stress` processes can run against the same package concurrently; compile cache hits are reused and failure files are created atomically. If all compared programs succeed but all stdout streams are empty on a non-empty generated input, `stress` still passes but prints `warning: all_empty_output`.
+ `stress-plan` runs `stress.plans` from `problem.yaml`. Plan args support `{seed}`, `{case}` (1-based), and `{case0}` (0-based); `{seed}` is derived deterministically from the plan name, case number, and optional `seed_base`. Plans default to `expect: pass`; use `expect: fail` for wrong-program evidence. Negative plans run every case, succeed when at least one `wrong_answer` or `program_failed` is observed, save the first failure artifacts, and report `failed_cases`, `passed_cases`, and `failure_ratio`. Use `--positive-only` to run only positive `expect: pass` plans, or `--negative-only` to run only negative `expect: fail` plans. Use `--summary-only` to print one compact line per plan, including unique input hashes, empty stdout case counts, and warning counts.
+ C++ compilation automatically adds the source file's directory to the include path, so `#include "common.hpp"` works for headers beside the source even though cptool compiles a cached copy. On Windows, effective compile args also include `-static` unless already present, avoiding MinGW runtime DLL lookup failures in CI and other clean environments. Compile failures include compiler path/version, flags, static-link status, cache key, and cache exe path. Windows runtime errors for common NTSTATUS exit codes include a diagnostic hint in failure reports.
+ `run`, `gen`, `stress`, and `stress-plan` default to a 32 MiB per-program stdout/stderr limit; pass `--output-limit-bytes` to override it where supported.

## Release

On Windows, publish a GitHub release from a clean checkout with:

```powershell
.\scripts\release.ps1 -Version 0.6.0
```

Replace `0.6.0` with the current `Cargo.toml` version. The script checks `fmt`, tests, clippy, builds release artifacts with `scripts/build-release.ps1`, pushes the current branch and tag, then creates the GitHub release with the generated archives and `SHA256SUMS.txt`.
