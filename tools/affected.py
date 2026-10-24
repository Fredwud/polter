#!/usr/bin/env python3
# This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of
# the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.
"""The workspace's packages a change touches, and every package that depends on them.

    python3 tools/affected.py [REV]
    python3 tools/affected.py --files PATH ...   (paths in the workspace, to check the tool)

Changed files are those that differ from REV (default HEAD: what is not committed yet, staged or
not) plus untracked ones. Each file belongs to the package whose folder holds it most closely; a
file in no member's folder but in the workspace's own (Cargo.toml, Cargo.lock, tools/) touches
every package. A package's tests, examples and benches touch it alone; its sources touch every
package that depends on it too (a change to the engine touches every game; a change to one game
touches that game and what is built on it). Prints the package names, one a line. Prints nothing when nothing in
the workspace changed. Used by `tools/check.sh --changed`."""

import json
import os
import shutil
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
# cargo where rustup puts it, when it is not on the PATH
CARGO = shutil.which("cargo") or os.path.expanduser("~/.cargo/bin/cargo")


def packages():
    """name -> (folder, names of the workspace packages it depends on)."""
    meta = json.loads(subprocess.run(
        [CARGO, "metadata", "--format-version", "1", "--no-deps"],
        cwd=ROOT, check=True, capture_output=True, text=True).stdout)
    names = {p["name"] for p in meta["packages"]}
    return {p["name"]: (os.path.dirname(p["manifest_path"]),
                        {d["name"] for d in p["dependencies"] if d["name"] in names})
            for p in meta["packages"]}


def changed(rev):
    """Absolute paths of the files that changed since `rev`, and the untracked ones, in the
    workspace's folder."""
    top = subprocess.run(["git", "rev-parse", "--show-toplevel"], cwd=ROOT, check=True,
                         capture_output=True, text=True).stdout.strip()
    out = []
    for cmd in (["git", "diff", "--name-only", rev, "--", "."],
                ["git", "ls-files", "--others", "--exclude-standard", "--", "."]):
        listed = subprocess.run(cmd, cwd=ROOT, check=True, capture_output=True, text=True).stdout
        out += [os.path.join(top, line) if cmd[1] == "diff" else os.path.join(ROOT, line)
                for line in listed.splitlines() if line]
    return sorted(set(out))


def affected(pkgs, files):
    # the members' folders, most deeply nested first; the package in the workspace's own folder
    # (the engine) owns only its sources there, the rest of that folder is the workspace's
    folders = sorted(((folder, name) for name, (folder, _) in pkgs.items() if folder != ROOT),
                     key=lambda f: -len(f[0]))
    root_pkg = next((name for name, (folder, _) in pkgs.items() if folder == ROOT), None)
    # a package's tests, examples and benches touch it alone; anything else in it (its sources,
    # its manifest, its assets) touches what depends on it too
    touched, alone = set(), set()
    own_only = ("tests", "examples", "benches")
    for path in files:
        owner = next((name for folder, name in folders if path.startswith(folder + os.sep)), None)
        if owner is not None:
            first = os.path.relpath(path, pkgs[owner][0]).split(os.sep)[0]
        else:
            if not path.startswith(ROOT + os.sep):
                continue                  # outside the workspace: nothing
            first = os.path.relpath(path, ROOT).split(os.sep)[0]
            if root_pkg and first in ("src",) + own_only:
                owner = root_pkg
            else:
                return set(pkgs)          # the workspace's own files: everything
        (alone if first in own_only else touched).add(owner)
    # and everything that depends on what was touched, however far up
    grew = True
    while grew:
        grew = False
        for name, (_, deps) in pkgs.items():
            if name not in touched and deps & touched:
                touched.add(name)
                grew = True
    return touched | alone


def main():
    pkgs = packages()
    if sys.argv[1:2] == ["--files"]:
        # for checking this tool: the files given (relative to the workspace) instead of git's
        files = [os.path.normpath(os.path.join(ROOT, f)) for f in sys.argv[2:]]
    else:
        files = changed(sys.argv[1] if len(sys.argv) > 1 else "HEAD")
    for name in sorted(affected(pkgs, files)):
        print(name)


if __name__ == "__main__":
    main()
