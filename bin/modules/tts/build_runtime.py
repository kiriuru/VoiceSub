#!/usr/bin/env python3
"""Build embedded google_tts_fetch runtime (Nuitka onefile, twitchTransFreeNext-style)."""

from __future__ import annotations

import platform
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SOURCE = ROOT / "google_tts_fetch.py"


def platform_target() -> tuple[str, str]:
    system = platform.system().lower()
    machine = platform.machine().lower()
    if system == "windows":
        return "win-x64", "google_tts_fetch.exe"
    if system == "darwin":
        suffix = "arm64" if machine in {"arm64", "aarch64"} else "x64"
        return f"macos-{suffix}", "google_tts_fetch"
    return "linux-x64", "google_tts_fetch"


def build_with_nuitka(out_dir: Path, out_name: str) -> bool:
    cmd = [
        sys.executable,
        "-m",
        "nuitka",
        "--standalone",
        "--onefile",
        "--assume-yes-for-downloads",
        f"--output-dir={out_dir}",
        f"--output-filename={out_name}",
    ]
    if platform.system() == "Windows":
        cmd.append("--windows-console-mode=disable")
    cmd.append(str(SOURCE))
    print("Running:", " ".join(cmd))
    completed = subprocess.run(cmd, check=False)
    return completed.returncode == 0 and (out_dir / out_name).is_file()


def build_with_pyinstaller(out_dir: Path, out_name: str) -> bool:
    work_dir = ROOT / "runtime" / "build" / "pyinstaller"
    work_dir.mkdir(parents=True, exist_ok=True)
    windowed = ["--noconsole"] if platform.system() == "Windows" else []
    cmd = [
        sys.executable,
        "-m",
        "PyInstaller",
        "--noconfirm",
        "--onefile",
        *windowed,
        "--name",
        out_name.removesuffix(".exe"),
        f"--distpath={out_dir}",
        f"--workpath={work_dir}",
        f"--specpath={work_dir}",
        str(SOURCE),
    ]
    print("Running:", " ".join(cmd))
    completed = subprocess.run(cmd, check=False)
    return completed.returncode == 0 and (out_dir / out_name).is_file()


def main() -> int:
    if not SOURCE.is_file():
        print(f"Missing source script: {SOURCE}", file=sys.stderr)
        return 1

    plat_dir, out_name = platform_target()
    out_dir = ROOT / "runtime" / plat_dir
    out_dir.mkdir(parents=True, exist_ok=True)

    if build_with_nuitka(out_dir, out_name):
        print(f"OK (nuitka): {out_dir / out_name}")
        return 0

    print("Nuitka build failed; falling back to PyInstaller...", file=sys.stderr)
    try:
        subprocess.run(
            [sys.executable, "-m", "pip", "install", "pyinstaller"],
            check=False,
        )
    except Exception:
        pass

    if build_with_pyinstaller(out_dir, out_name):
        print(f"OK (pyinstaller): {out_dir / out_name}")
        return 0

    print(f"Build failed; binary not found: {out_dir / out_name}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
