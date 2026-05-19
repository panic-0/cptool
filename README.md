# CP Tool

CP Tool is a command line tool for competitive programming.

## Usage

```bash
# create a cptool/autocpp problem package
./cptool init a_plus_b

# generate official data into ./example/a_plus_b/data
./cptool gen -w ./example/a_plus_b

# run a configured program on a generated bundle case
./cptool run std sample[0] -w ./example/a_plus_b

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
      path: ./gen.cpp
    time_limit_secs: 1.0
    memory_limit_mb: 512.0
  std:
    info: !cpp
      path: ./std.cpp
      compile_args: [-O2, -std=c++14]
    time_limit_secs: 1.0
    memory_limit_mb: 512.0
solution: std
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
+ `gen` writes data to `data/` by default.
+ `run` uses a bundle case such as `sample[0]` by default, but can also read `--stdin-path` or `--stdin-text`.
+ `stress` is for ad-hoc correctness checks. It does not run official bundles and does not assume `brute` is safe on large data.
