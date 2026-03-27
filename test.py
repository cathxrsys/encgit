#!/usr/bin/env python3

# tests written by claude sonnet 4.6

"""Integration tests for encgit.

Scenario A - init / push / pull:
    A1. Reset remote to empty
    A2. encgit init git@github.com:cathxrsys/test
    A3. Add tracked and ignored files
    A4. encgit push
    A5. Add local junk and delete tracked files
    A6. encgit pull
    A7. Assert normal pull restores files without deleting local junk
    A8. encgit pull --force and answer n
    A9. Assert force pull cancellation leaves working tree unchanged
    A10. encgit pull --force and answer y
    A11. Assert forced pull restores exact archived state

Scenario B - clone:
    B1. encgit clone git@github.com:cathxrsys/test
    B2. Assert tracked files are present and ignored files are absent

Scenario C - clone failure cleanup:
    C1. encgit clone with wrong password
    C2. Assert target directory was not created

Scenario D - init failure cleanup:
    D1. encgit init against non-empty remote
    D2. Assert target directory was not created

Requirements:
    - pexpect  (pip install pexpect)
    - SSH access to git@github.com:cathxrsys/test
    - encgit binary built at target/debug/encgit  (run `cargo build` first)

Usage:
    python3 test.py [--password <password>]
    Password defaults to TEST_PASSWORD env var or "testpassword".
"""

import argparse
import os
import shutil
import shlex
import subprocess
import sys
import tempfile

try:
    import pexpect
except ImportError:
    sys.exit("pexpect is not installed. Run: pip install pexpect")

REPO_URL = "git@github.com:cathxrsys/test"
ENCGIT   = os.path.join(os.path.dirname(os.path.abspath(__file__)),
                         "target", "debug", "encgit")
GIT_ENV = {
    **os.environ,
    "GIT_AUTHOR_NAME": os.environ.get("GIT_AUTHOR_NAME", "encgit-test"),
    "GIT_AUTHOR_EMAIL": os.environ.get("GIT_AUTHOR_EMAIL", "encgit-test@example.com"),
    "GIT_COMMITTER_NAME": os.environ.get("GIT_COMMITTER_NAME", "encgit-test"),
    "GIT_COMMITTER_EMAIL": os.environ.get("GIT_COMMITTER_EMAIL", "encgit-test@example.com"),
}


def parse_args():
    p = argparse.ArgumentParser(description="encgit integration test")
    p.add_argument("--password", default=os.environ.get("TEST_PASSWORD", "testpassword"),
                   help="Password used for all encgit operations")
    return p.parse_args()


def run_encgit(
    args: list[str],
    password: str,
    cwd: str | None = None,
    timeout: int = 180,
    confirmations: list[str] | None = None,
    require_confirmation: bool = False,
) -> int:
    """Spawn encgit, answer password and confirmation prompts, return exit status."""
    print(f"\n$ {' '.join(shlex.quote(arg) for arg in args)}")
    child = pexpect.spawn(args[0], args[1:], cwd=cwd, timeout=timeout, encoding="utf-8")
    child.logfile_read = sys.stdout
    confirmations = list(confirmations or [])
    saw_confirmation = False

    while True:
        idx = child.expect([
            "Enter password: ",
            "Confirm password: ",
            r"Continue\? \[y/N\]: ",
            pexpect.EOF,
            pexpect.TIMEOUT,
        ])
        if idx == 0:
            child.sendline(password)
        elif idx == 1:
            child.sendline(password)
        elif idx == 2:
            saw_confirmation = True
            reply = confirmations.pop(0) if confirmations else "n"
            child.sendline(reply)
        elif idx == 3:
            break
        else:
            print("\n[TIMEOUT] Command did not finish in time")
            child.terminate(force=True)
            return 1

    child.wait()
    if require_confirmation and not saw_confirmation:
        print("\n[FAIL] Force-pull confirmation prompt was not shown")
        return 1
    return child.exitstatus if child.exitstatus is not None else 0


def git(*args, cwd=None):
    """Run a git command, raising on failure."""
    subprocess.run(
        ["git", *args],
        cwd=cwd,
        env=GIT_ENV,
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def append_text(path: str, text: str) -> None:
    with open(path, "a", encoding="utf-8") as file_obj:
        file_obj.write(text)


def write_text(path: str, text: str) -> None:
    with open(path, "w", encoding="utf-8") as file_obj:
        file_obj.write(text)


def read_text(path: str) -> str:
    with open(path, encoding="utf-8") as file_obj:
        return file_obj.read()


def reset_remote(repo_url: str) -> None:
    """Force-push a single empty orphan commit to clear the remote."""
    print(f"\n[setup] Resetting remote {repo_url} to empty state...")
    tmp = tempfile.mkdtemp(prefix="encgit_reset_")
    try:
        git("init", cwd=tmp)
        git("checkout", "-b", "main", cwd=tmp)
        git("commit", "--allow-empty", "-m", "reset", cwd=tmp)
        git("remote", "add", "origin", repo_url, cwd=tmp)
        git("push", "--force", "origin", "main", cwd=tmp)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    print("[setup] Remote reset done.")


def check(condition: bool, description: str) -> None:
    if condition:
        print(f"  [PASS] {description}")
    else:
        print(f"  [FAIL] {description}")
        sys.exit(1)


def main():
    args = parse_args()
    password = args.password

    check(os.path.isfile(ENCGIT), f"encgit binary exists at {ENCGIT}")

    workdir_a = tempfile.mkdtemp(prefix="encgit_test_a_")
    workdir_b = tempfile.mkdtemp(prefix="encgit_test_b_")
    workdir_c = tempfile.mkdtemp(prefix="encgit_test_c_")
    workdir_d = tempfile.mkdtemp(prefix="encgit_test_d_")
    repo_dir_a = os.path.join(workdir_a, "test")
    repo_dir_b = os.path.join(workdir_b, "test")
    repo_dir_c = os.path.join(workdir_c, "test")
    repo_dir_d = os.path.join(workdir_d, "test")
    test_file_a = os.path.join(repo_dir_a, "hello.txt")
    test_file_b = os.path.join(repo_dir_b, "hello.txt")
    ignored_file_a = os.path.join(repo_dir_a, "ignored.log")
    ignored_file_b = os.path.join(repo_dir_b, "ignored.log")
    junk_file_a = os.path.join(repo_dir_a, "junk.tmp")
    stale_dir_a = os.path.join(repo_dir_a, "build")
    stale_file_a = os.path.join(stale_dir_a, "artifact.bin")
    gitignore_path_a = os.path.join(repo_dir_a, ".gitignore")
    test_content = "Hello, encgit zip!\n"
    ignored_content = "This file must stay local only.\n"
    junk_content = "This file should be removed by force pull.\n"

    print(f"\nTemp workdir A (init/push/pull): {workdir_a}")
    print(f"Temp workdir B (clone):          {workdir_b}")
    print(f"Temp workdir C (failed clone):   {workdir_c}")
    print(f"Temp workdir D (failed init):    {workdir_d}")

    try:
        print("\n" + "=" * 60)
        print("  SCENARIO A: init -> push -> soft pull -> force pull")
        print("=" * 60)

        reset_remote(REPO_URL)

        print("\n=== [A2] encgit init ===")
        status = run_encgit([ENCGIT, "--workdir", workdir_a, "init", REPO_URL], password)
        check(status == 0, "encgit init exited 0")
        check(os.path.isdir(repo_dir_a), "repo directory created")
        check(os.path.isdir(os.path.join(repo_dir_a, ".encgit")), ".encgit dir present")

        print("\n=== [A3] Create tracked and ignored files ===")
        append_text(gitignore_path_a, "*.log\n")
        write_text(test_file_a, test_content)
        write_text(ignored_file_a, ignored_content)
        check(os.path.exists(test_file_a), "hello.txt created locally")
        check(os.path.exists(ignored_file_a), "ignored.log created locally")

        print("\n=== [A4] encgit push ===")
        status = run_encgit([ENCGIT, "push"], password, cwd=repo_dir_a)
        check(status == 0, "encgit push exited 0")

        print("\n=== [A5] Delete tracked files and add local junk ===")
        os.remove(test_file_a)
        os.remove(ignored_file_a)
        write_text(junk_file_a, junk_content)
        os.makedirs(stale_dir_a, exist_ok=True)
        write_text(stale_file_a, "stale build artifact\n")
        check(not os.path.exists(test_file_a), "hello.txt deleted locally")
        check(not os.path.exists(ignored_file_a), "ignored.log deleted locally")
        check(os.path.exists(junk_file_a), "junk.tmp created locally")
        check(os.path.exists(stale_file_a), "stale artifact created locally")

        print("\n=== [A6] encgit pull ===")
        status = run_encgit([ENCGIT, "pull"], password, cwd=repo_dir_a)
        check(status == 0, "encgit pull exited 0")

        print("\n=== [A7] Verify normal pull keeps local junk ===")
        check(os.path.exists(test_file_a), "hello.txt exists after normal pull")
        check(read_text(test_file_a) == test_content, "hello.txt content matches original after normal pull")
        check(not os.path.exists(ignored_file_a), "ignored.log was not restored from archive")
        check(os.path.exists(junk_file_a), "junk.tmp preserved by normal pull")
        check(os.path.exists(stale_file_a), "stale artifact preserved by normal pull")

        print("\n=== [A8] encgit pull --force and answer n ===")
        status = run_encgit(
            [ENCGIT, "pull", "--force"],
            password,
            cwd=repo_dir_a,
            confirmations=["n"],
            require_confirmation=True,
        )
        check(status != 0, "encgit pull --force is cancelled on 'n'")

        print("\n=== [A9] Verify cancelled force pull left working tree unchanged ===")
        check(os.path.exists(test_file_a), "hello.txt still exists after cancelled force pull")
        check(read_text(test_file_a) == test_content, "hello.txt content unchanged after cancelled force pull")
        check(os.path.exists(junk_file_a), "junk.tmp still exists after cancelled force pull")
        check(os.path.exists(stale_file_a), "stale artifact still exists after cancelled force pull")

        print("\n=== [A10] encgit pull --force and answer y ===")
        status = run_encgit(
            [ENCGIT, "pull", "--force"],
            password,
            cwd=repo_dir_a,
            confirmations=["y"],
            require_confirmation=True,
        )
        check(status == 0, "encgit pull --force exited 0")

        print("\n=== [A11] Verify force pull restored exact archived state ===")
        check(os.path.exists(test_file_a), "hello.txt exists after force pull")
        check(read_text(test_file_a) == test_content, "hello.txt content matches original after force pull")
        check(not os.path.exists(ignored_file_a), "ignored.log was not restored from archive after force pull")
        check(not os.path.exists(junk_file_a), "junk.tmp removed by force pull")
        check(not os.path.exists(stale_file_a), "stale artifact removed by force pull")

        print("\n" + "=" * 60)
        print("  SCENARIO B: clone")
        print("=" * 60)

        print("\n=== [B1] encgit clone ===")
        status = run_encgit([ENCGIT, "--workdir", workdir_b, "clone", REPO_URL], password)
        check(status == 0, "encgit clone exited 0")
        check(os.path.isdir(repo_dir_b), "cloned repo directory created")
        check(os.path.isdir(os.path.join(repo_dir_b, ".encgit")), ".encgit dir present in clone")

        print("\n=== [B2] Verify archive contents in cloned repo ===")
        check(os.path.exists(test_file_b), "hello.txt present in cloned repo")
        check(read_text(test_file_b) == test_content, "hello.txt content matches in clone")
        check(not os.path.exists(ignored_file_b), "ignored.log absent in cloned repo")

        print("\n" + "=" * 60)
        print("  SCENARIO C: failed clone cleanup")
        print("=" * 60)

        print("\n=== [C1] encgit clone with wrong password ===")
        status = run_encgit([ENCGIT, "--workdir", workdir_c, "clone", REPO_URL], "wrong-password")
        check(status != 0, "encgit clone fails with wrong password")

        print("\n=== [C2] Verify failed clone left no target directory ===")
        check(not os.path.exists(repo_dir_c), "failed clone cleaned up target directory")

        print("\n" + "=" * 60)
        print("  SCENARIO D: failed init cleanup")
        print("=" * 60)

        print("\n=== [D1] encgit init against non-empty remote ===")
        status = run_encgit([ENCGIT, "--workdir", workdir_d, "init", REPO_URL], password)
        check(status != 0, "encgit init fails when remote already contains data")

        print("\n=== [D2] Verify failed init left no target directory ===")
        check(not os.path.exists(repo_dir_d), "failed init cleaned up target directory")

        print("\n" + "=" * 60)
        print("  ALL TESTS PASSED")
        print("=" * 60)

    finally:
        shutil.rmtree(workdir_a, ignore_errors=True)
        shutil.rmtree(workdir_b, ignore_errors=True)
        shutil.rmtree(workdir_c, ignore_errors=True)
        shutil.rmtree(workdir_d, ignore_errors=True)
        print(f"\nCleaned up {workdir_a}, {workdir_b}, {workdir_c}, and {workdir_d}")


if __name__ == "__main__":
    main()
