# CP Tool

CP Tool is a command line tool for competitive programming.

## Usage

```bash
# create a cptool/autocpp problem package
./cptool init a_plus_b

# generate official data into ./example/a_plus_b/data
./cptool gen -w ./example/a_plus_b

# generate official data and print one compact audit summary
./cptool gen -w ./example/a_plus_b --summary-only

# clean selected stale data before publishing newly generated data
./cptool gen -w ./example/a_plus_b --bundle main --clean

# generate with a custom per-program stdout/stderr limit; default is 32 MiB
./cptool gen -w ./example/a_plus_b --output-limit-bytes 67108864

# run a configured program on a generated bundle case
./cptool run std sample[0] -w ./example/a_plus_b

# print only a compact run summary for large outputs
./cptool run std sample[0] -w ./example/a_plus_b --summary-only

# stress test programs with generated temporary inputs
./cptool stress -w ./example/a_plus_b --generator gen --against std --against brute --cases 100 -- 10
./cptool stress -w ./example/a_plus_b --generator gen --against std --against brute --cases 100 -- {seed} {case}

# run stress plans declared in problem.yaml
./cptool stress-plan -w ./example/a_plus_b --name small
./cptool stress-plan -w ./example/a_plus_b --summary-only

# check common package structure and generated data issues
./cptool check -w ./example/a_plus_b

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
    time_limit_secs: 1.0
    memory_limit_mb: 512.0
  std:
    info: !cpp
      path: ./src/std.cpp
      compile_args: [-O2, -std=c++14]
    time_limit_secs: 1.0
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

Programs can also use `!command` or `!python`; omitted C++ compile args default to C++20 with warnings.

## Notes

+ Syzoj export is not fully supported yet.
+ `--version` prints the package version and the git commit embedded at build time, for example `cptool 0.5.0 (commit abc1234)`; local builds from a modified checkout append `-dirty`.
+ `init` creates only the cptool-managed scaffold: `problem.yaml`, `statement.md`, `editorial.md`, `src/`, `data/`, `tests/failures/`, and a package `.gitignore`.
+ `gen` writes data to `data/` by default. It stages generated files first and moves them into place only after the selected cases succeed. Use `--clean` to remove stale `.in/.ans` files for the selected case, bundle, or known bundles before publishing the newly generated files. Use `--summary-only` to suppress per-file `generated` lines and print cases, bundles, elapsed time, input/answer bytes, and warning counts.
+ `run` uses a bundle case such as `sample[0]` by default, but can also read `--stdin-path` or `--stdin-text`. Use `--summary-only` to suppress full stdout and print size/line/hash fields, or `--hide-stdout` to keep only the status line while still allowing `--stdout-path`.
+ `gen` warns when a non-empty input produces an empty answer. Set `output.allow_empty: true` in `problem.yaml` for tasks where empty output is valid. In `--summary-only` mode, `empty_answer` and generator-output warnings are counted in the summary.
+ `check` reports common structure, program path, validator declaration, generated data, sample generation, and sample output issues. It exits non-zero when errors are found, and reports `data_generation_in_progress` instead of inspecting data during a concurrent `gen`. If no `validator` is configured, `check` emits `warning: validator_missing`; set `validator_omitted_reason: "..."` in `problem.yaml` when omission is intentional.
+ `stress` is for ad-hoc correctness checks. It does not run official bundles and does not assume `brute` is safe on large data. Arguments after `--` support `{seed}`, `{case}` (1-based), and `{case0}` (0-based); fixed args without placeholders are passed literally for every case. `{seed}` is derived deterministically from the stress command and case number. The final summary includes `unique_input_hashes=N`, which is useful for noticing fixed-argument repeated inputs. If `cases > 1` but all generated inputs have the same hash, `stress` still passes but prints `warning: repeated_input`. Multiple `cptool stress` processes can run against the same package concurrently; compile cache hits are reused and failure files are created atomically. If all compared programs succeed but all stdout streams are empty on a non-empty generated input, `stress` still passes but prints `warning: all_empty_output`.
+ `stress-plan` runs `stress.plans` from `problem.yaml`. Plan args support `{seed}`, `{case}` (1-based), and `{case0}` (0-based); `{seed}` is derived deterministically from the plan name, case number, and optional `seed_base`. Plans default to `expect: pass`; use `expect: fail` for wrong-program evidence where the first observed `wrong_answer` or `program_failed` should make the plan succeed and save the failure artifacts. Use `--summary-only` to print one compact line per plan, including unique input hashes, empty stdout case counts, and warning counts.
+ C++ compile failures include compiler path/version, flags, static-link status, cache key, and cache exe path. Windows runtime errors for common NTSTATUS exit codes include a diagnostic hint in failure reports.
+ `run`, `gen`, `stress`, and `stress-plan` default to a 32 MiB per-program stdout/stderr limit; pass `--output-limit-bytes` to override it where supported.
