# CP Tool

CP Tool is a command line tool for competitive programming.

## Usage

```bash
# create a cptool/autocpp problem package
./cptool init a_plus_b

# generate official data into ./example/a_plus_b/data
./cptool gen -w ./example/a_plus_b

# generate with a custom per-program stdout/stderr limit; default is 32 MiB
./cptool gen -w ./example/a_plus_b --output-limit-bytes 67108864

# run a configured program on a generated bundle case
./cptool run std sample[0] -w ./example/a_plus_b

# print only a compact run summary for large outputs
./cptool run std sample[0] -w ./example/a_plus_b --summary-only

# stress test programs with generated temporary inputs
./cptool stress -w ./example/a_plus_b --generator gen --against std --against brute --cases 100 -- 10

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
```

Programs can also use `!command` or `!python`; omitted C++ compile args default to C++20 with warnings.

## Notes

+ Syzoj export is not fully supported yet.
+ `init` creates only the cptool-managed scaffold: `problem.yaml`, `statement.md`, `editorial.md`, `src/`, `data/`, `tests/failures/`, and a package `.gitignore`.
+ `gen` writes data to `data/` by default. `run`, `gen`, and `stress` default to a 32 MiB per-program stdout/stderr limit; pass `--output-limit-bytes` to override it.
+ `run` uses a bundle case such as `sample[0]` by default, but can also read `--stdin-path` or `--stdin-text`. Use `--summary-only` to suppress full stdout and print size/line/hash fields, or `--hide-stdout` to keep only the status line while still allowing `--stdout-path`.
+ `gen` warns when a non-empty input produces an empty answer. Set `output.allow_empty: true` in `problem.yaml` for tasks where empty output is valid.
+ `stress` is for ad-hoc correctness checks. It does not run official bundles and does not assume `brute` is safe on large data. Multiple `cptool stress` processes can run against the same package concurrently; compile cache hits are reused and failure files are created atomically.
