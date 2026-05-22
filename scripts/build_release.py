#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import os
import platform
import shutil
import shlex
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
DIST_DIR = REPO_ROOT / "dist"


@dataclass(frozen=True)
class ReleaseTarget:
    name: str
    platform_name: str
    host_system: str
    binary_name: str
    target_dir_name: str
    archive_suffix: str
    executable: bool


TARGETS = {
    "windows": ReleaseTarget(
        name="windows",
        platform_name="windows-x86_64",
        host_system="Windows",
        binary_name="cptool.exe",
        target_dir_name="release-windows",
        archive_suffix=".zip",
        executable=False,
    ),
    "linux": ReleaseTarget(
        name="linux",
        platform_name="linux-x86_64",
        host_system="Linux",
        binary_name="cptool",
        target_dir_name="release-linux",
        archive_suffix=".tar.gz",
        executable=True,
    ),
}


def usage(program_name: str) -> str:
    return "\n".join(
        [
            f"Usage: {program_name} [--target windows|linux|all] [--version VERSION]",
            "",
            "Options:",
            "  --target TARGET     Release target to build. Defaults to the current host target.",
            "  --version VERSION   Release version. Defaults to VERSION env or Cargo.toml.",
        ]
    )


def parse_args(argv: list[str]) -> tuple[str, argparse.Namespace]:
    program_name = "python scripts/build_release.py"
    args = list(argv)
    if len(args) >= 2 and args[0] == "--usage-name":
        program_name = args[1]
        args = args[2:]

    parser = argparse.ArgumentParser(
        prog=program_name,
        usage=usage(program_name).splitlines()[0].removeprefix("Usage: "),
        add_help=False,
    )
    parser.add_argument("--target", choices=["windows", "linux", "all"], default=default_target_name())
    parser.add_argument("--version", default="")
    parser.add_argument("--checksums-only", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--print-version", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("-h", "--help", action="store_true")
    parsed, unknown = parser.parse_known_args(args)
    if parsed.help:
        print(usage(program_name))
        raise SystemExit(0)
    if unknown:
        raise ValueError(f"Unknown argument: {unknown[0]}")
    return program_name, parsed


def run(command: list[str], cwd: Path = REPO_ROOT, env: dict[str, str] | None = None) -> None:
    subprocess.run(command, cwd=cwd, env=env, check=True)


def read_cargo_version() -> str:
    cargo_toml = REPO_ROOT / "Cargo.toml"
    for line in cargo_toml.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if not stripped.startswith("version"):
            continue
        key, separator, value = stripped.partition("=")
        if key.strip() == "version" and separator:
            return value.strip().strip('"')
    raise ValueError("Could not read package version from Cargo.toml")


def resolve_version(explicit_version: str) -> str:
    version = explicit_version.strip() or os.environ.get("VERSION", "").strip()
    return version or read_cargo_version()


def selected_targets(target: str) -> list[ReleaseTarget]:
    if target == "all":
        return [TARGETS["windows"], TARGETS["linux"]]
    return [TARGETS[target]]


def default_target_name() -> str:
    if platform.system() == "Windows":
        return "windows"
    return "linux"


def assert_host_supported(target: ReleaseTarget) -> None:
    current = platform.system()
    if current == target.host_system:
        return
    if current == "Windows" and target.name == "linux":
        return
    raise RuntimeError(
        f"{target.name} release builds must run on {target.host_system}; "
        f"current host is {current}."
    )


def build_release_target(version: str, target: ReleaseTarget) -> Path:
    if platform.system() == target.host_system:
        return build_target(version, target)
    if platform.system() == "Windows" and target.name == "linux":
        return build_linux_with_wsl(version)
    raise RuntimeError(
        f"{target.name} release builds must run on {target.host_system}; "
        f"current host is {platform.system()}."
    )


def build_linux_with_wsl(version: str) -> Path:
    target = TARGETS["linux"]
    wsl_repo_root = convert_to_wsl_path(REPO_ROOT)
    if not wsl_repo_root:
        raise RuntimeError("Could not translate repository path for WSL")

    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO_ROOT, text=True).strip()
    linux_env = {"VERSION": version}
    wsl_cargo_home = convert_cargo_home_to_wsl_path()
    if wsl_cargo_home:
        linux_env["CARGO_HOME"] = wsl_cargo_home

    env_prefix = " ".join(f"{name}={shlex.quote(value)}" for name, value in linux_env.items())
    artifact_name = f"{package_name(version, target)}{target.archive_suffix}"
    script = "\n".join(
        [
            "set -euo pipefail",
            'export PATH="$HOME/.cargo/bin:$PATH"',
            "tmp_dir=$(mktemp -d)",
            'trap \'rm -rf "$tmp_dir"\' EXIT',
            f"git clone --quiet {shlex.quote(wsl_repo_root)} \"$tmp_dir/repo\"",
            'cd "$tmp_dir/repo"',
            f"git checkout --quiet {shlex.quote(commit)}",
            "git submodule update --init --recursive",
            f"{env_prefix} python3 scripts/build_release.py --target linux --version {shlex.quote(version)}",
            f"mkdir -p {shlex.quote(wsl_repo_root + '/dist')}",
            f"cp {shlex.quote('dist/' + artifact_name)} {shlex.quote(wsl_repo_root + '/dist/')}",
            "",
        ]
    )

    temp_script = None
    try:
        with tempfile.NamedTemporaryFile("w", encoding="ascii", newline="\n", delete=False) as handle:
            handle.write(script)
            temp_script = Path(handle.name)
        run(["wsl", "bash", convert_to_wsl_path(temp_script)])
    finally:
        if temp_script is not None:
            temp_script.unlink(missing_ok=True)

    return DIST_DIR / artifact_name


def convert_to_wsl_path(path: Path) -> str:
    raw_path = str(path)
    drive = path.drive
    if drive and drive.endswith(":"):
        relative = raw_path[len(drive) :].lstrip("\\/").replace("\\", "/")
        return f"/mnt/{drive[0].lower()}/{relative}"

    try:
        return subprocess.check_output(["wsl", "wslpath", "-a", raw_path], text=True).strip()
    except (OSError, subprocess.CalledProcessError) as exc:
        raise RuntimeError(
            f"Could not translate path for WSL: {raw_path}"
        ) from exc


def convert_cargo_home_to_wsl_path() -> str:
    cargo_home = os.environ.get("CARGO_HOME")
    if cargo_home:
        path = Path(cargo_home)
    else:
        user_profile = os.environ.get("USERPROFILE")
        if not user_profile:
            return ""
        path = Path(user_profile) / ".cargo"

    if not path.exists():
        return ""
    return convert_to_wsl_path(path)


def remove_path(path: Path) -> None:
    if not path.exists():
        return
    if path.is_dir():
        shutil.rmtree(path)
    else:
        path.unlink()


def package_name(version: str, target: ReleaseTarget) -> str:
    return f"cptool-v{version}-{target.platform_name}"


def build_target(version: str, target: ReleaseTarget) -> Path:
    DIST_DIR.mkdir(parents=True, exist_ok=True)
    name = package_name(version, target)
    package_dir = DIST_DIR / name
    archive = DIST_DIR / f"{name}{target.archive_suffix}"
    target_dir = REPO_ROOT / "target" / target.target_dir_name

    remove_path(package_dir)
    remove_path(archive)

    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target_dir)
    run(["cargo", "build", "--release"], env=env)

    package_dir.mkdir(parents=True, exist_ok=True)
    binary = package_dir / target.binary_name
    shutil.copy2(target_dir / "release" / target.binary_name, binary)
    if target.executable:
        binary.chmod(binary.stat().st_mode | 0o755)
    shutil.copy2(REPO_ROOT / "README.md", package_dir / "README.md")

    create_archive(package_dir, archive, target)
    run([str(binary), "--version"])
    print(f"created {archive}")
    return archive


def create_archive(package_dir: Path, archive: Path, target: ReleaseTarget) -> None:
    if target.archive_suffix == ".zip":
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as zip_file:
            for path in sorted(package_dir.rglob("*")):
                if path.is_file():
                    zip_file.write(path, path.relative_to(package_dir.parent))
        return

    if target.archive_suffix == ".tar.gz":
        with tarfile.open(archive, "w:gz") as tar_file:
            tar_file.add(package_dir, arcname=package_dir.name)
        return

    raise AssertionError(f"unsupported archive type: {target.archive_suffix}")


def write_checksums(version: str) -> Path | None:
    DIST_DIR.mkdir(parents=True, exist_ok=True)
    files = sorted(path for path in DIST_DIR.glob(f"cptool-v{version}-*") if path.is_file())
    if not files:
        return None

    lines = []
    for path in files:
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        lines.append(f"{digest}  {path.name}")

    checksum_path = DIST_DIR / "SHA256SUMS.txt"
    checksum_path.write_text("\n".join(lines) + "\n", encoding="ascii")
    print(f"created {checksum_path}")
    return checksum_path


def main(argv: list[str]) -> int:
    try:
        program_name, args = parse_args(argv)
        version = resolve_version(args.version)
        if args.print_version:
            print(version)
            return 0
        if not args.checksums_only:
            targets = selected_targets(args.target)
            for target in targets:
                assert_host_supported(target)
            for target in targets:
                build_release_target(version, target)
        write_checksums(version)
        return 0
    except ValueError as exc:
        print(exc, file=sys.stderr)
        print(usage(program_name), file=sys.stderr)
        return 2
    except (OSError, RuntimeError, subprocess.CalledProcessError) as exc:
        print(exc, file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
