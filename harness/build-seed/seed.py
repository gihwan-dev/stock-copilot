import ctypes
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile


CLONE_NOFOLLOW = 1


def clone_file(source, destination):
    library = ctypes.CDLL(None, use_errno=True)
    clone = library.clonefile
    clone.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int]
    clone.restype = ctypes.c_int
    if clone(os.fsencode(source), os.fsencode(destination), CLONE_NOFOLLOW) != 0:
        number = ctypes.get_errno()
        raise OSError(number, os.strerror(number), str(source))
    if not stat.S_ISREG(os.lstat(destination).st_mode):
        raise OSError("cloned entry is not a regular file")


def clone_tree(source, destination):
    with os.scandir(source) as entries:
        for entry in entries:
            source_entry = Path(entry.path)
            destination_entry = Path(destination) / entry.name
            if entry.is_symlink():
                continue
            if entry.is_dir(follow_symlinks=False):
                destination_entry.mkdir()
                clone_tree(source_entry, destination_entry)
            elif entry.is_file(follow_symlinks=False):
                clone_file(source_entry, destination_entry)


def empty_destination(target):
    if not os.path.lexists(target):
        return True
    if not stat.S_ISDIR(os.lstat(target).st_mode):
        return False
    with os.scandir(target) as entries:
        return next(entries, None) is None


def seed_directory(source, target, prepare):
    source = os.path.realpath(source)
    target = os.path.abspath(target)
    if sys.platform != "darwin" or not os.path.isdir(source):
        return False
    resolved_target = os.path.realpath(target)
    if os.path.commonpath((source, resolved_target)) in (source, resolved_target):
        return False
    if not empty_destination(target):
        return False
    stage = None
    try:
        stage = tempfile.mkdtemp(prefix=".stock-copilot-seed-", dir=os.path.dirname(target))
        clone_tree(source, stage)
        prepare(stage)
        if not empty_destination(target):
            return False
        if os.path.isdir(target):
            os.rmdir(target)
        os.rename(stage, target)
        stage = None
        return True
    except (OSError, subprocess.SubprocessError):
        return False
    finally:
        if stage is not None:
            shutil.rmtree(stage)


def git_path(argument):
    result = subprocess.run(
        ["/usr/bin/git", "rev-parse", "--path-format=absolute", argument],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        env={
            "PATH": "/usr/bin:/bin",
            "GIT_CONFIG_GLOBAL": "/dev/null",
            "GIT_CONFIG_NOSYSTEM": "1",
        },
    )
    return os.path.realpath(result.stdout.strip())


def clear_workspace(worktree, stage):
    toolchain = os.path.join(os.path.expanduser("~"), ".cargo", "bin")
    environment = os.environ.copy()
    environment.update(
        {
            "RUSTC": os.path.join(toolchain, "rustc"),
            "RUSTC_WRAPPER": "",
            "RUSTC_WORKSPACE_WRAPPER": "",
        }
    )
    for name in (
        "CARGO_BUILD_RUSTC",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
        "RUSTUP_TOOLCHAIN",
    ):
        environment.pop(name, None)
    subprocess.run(
        [
            os.path.join(toolchain, "cargo"),
            "clean",
            "--locked",
            "--offline",
            "--workspace",
            "--target-dir",
            stage,
        ],
        cwd=worktree,
        check=True,
        env=environment,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def main():
    requested = os.environ.get("STOCK_COPILOT_SEED_TARGET", "")
    if not requested or os.path.islink(requested):
        return
    try:
        worktree = git_path("--show-toplevel")
        common = git_path("--git-common-dir")
        if os.path.basename(common) != ".git":
            return
        primary_target = os.path.join(os.path.dirname(common), "target")
        if os.path.realpath(requested) != os.path.realpath(primary_target):
            return
        destination = os.path.join(worktree, "target")
        seeded = seed_directory(
            requested, destination, lambda stage: clear_workspace(worktree, stage)
        )
        print("seed-target: 복제 완료" if seeded else "seed-target: 복제 건너뜀")
    except (OSError, subprocess.SubprocessError, ValueError):
        print("seed-target: 복제 건너뜀")


if __name__ == "__main__":
    main()
