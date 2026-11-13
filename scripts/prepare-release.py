#!/usr/bin/env python3

"""Update the workspace and current installation examples for a stable release."""

import argparse
from datetime import date
from pathlib import Path
import re
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version")
    args = parser.parse_args()
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", args.version):
        parser.error("version must be a stable version such as 2.2.0")

    root = Path(__file__).resolve().parent.parent
    manifest = tomllib.loads((root / "Cargo.toml").read_text())
    previous = manifest["workspace"]["package"]["version"]
    if tuple(map(int, args.version.split("."))) <= tuple(map(int, previous.split("."))):
        parser.error(f"version must be newer than {previous}")

    updates = {}
    for name in [
        "Cargo.toml",
        "walrus/Cargo.toml",
        "README.md",
        "cli/README.md",
        "sdk/README.md",
        "toolkit-rust/README.md",
        "walrus/README.md",
        "gcp-kms/README.md",
    ]:
        path = root / name
        updates[path] = path.read_text().replace(previous, args.version)

    lock_path = root / "Cargo.lock"
    lock = lock_path.read_text()
    for member in manifest["workspace"]["members"]:
        package = tomllib.loads((root / member / "Cargo.toml").read_text())["package"]
        if package["version"] != {"workspace": True}:
            parser.error(f"{member} must inherit the workspace version")
        pattern = rf'(name = "{re.escape(package["name"])}"\nversion = "){re.escape(previous)}(")'
        lock, count = re.subn(pattern, lambda match: match[1] + args.version + match[2], lock)
        if count != 1:
            parser.error(f"expected one lockfile entry for {package['name']} {previous}")
        lock = lock.replace(f'"{package["name"]} {previous}"', f'"{package["name"]} {args.version}"')
    updates[lock_path] = lock

    changelog_path = root / "CHANGELOG.md"
    changelog = changelog_path.read_text()
    heading = f"## [`{args.version}`]"
    if heading in changelog or changelog.count("## Unreleased\n") != 1:
        parser.error("expected one Unreleased section and no existing release heading")
    unreleased = changelog.split("## Unreleased\n", 1)[1].split("\n## ", 1)[0]
    if not unreleased.strip():
        parser.error("add release notes under Unreleased first")
    updates[changelog_path] = changelog.replace(
        "## Unreleased\n", f"## Unreleased\n\n{heading} - {date.today().isoformat()}\n", 1
    )

    for path, content in updates.items():
        if content != path.read_text():
            path.write_text(content)
            print(path.relative_to(root))


if __name__ == "__main__":
    main()
