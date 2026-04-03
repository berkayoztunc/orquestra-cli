#!/usr/bin/env python3
"""Update Homebrew formula with new release URLs and SHA256 checksums."""

import argparse
import re
import sys


def update_formula(formula_path: str, tag: str, base_url: str, arm64_sha: str, x86_sha: str) -> None:
    version = tag.lstrip("v")

    with open(formula_path) as f:
        content = f.read()

    # Update version
    content = re.sub(r'version "[^"]*"', f'version "{version}"', content)

    # Update SHA256 placeholders or existing values
    arm64_url = f"{base_url}/orquestra-{tag}-aarch64-apple-darwin.tar.gz"
    x86_url = f"{base_url}/orquestra-{tag}-x86_64-apple-darwin.tar.gz"

    # Replace URL lines
    content = re.sub(
        r'url "https://github\.com/[^"]*aarch64-apple-darwin[^"]*"',
        f'url "{arm64_url}"',
        content,
    )
    content = re.sub(
        r'url "https://github\.com/[^"]*x86_64-apple-darwin[^"]*"',
        f'url "{x86_url}"',
        content,
    )

    # Replace SHA256 lines (one after each url block)
    lines = content.splitlines()
    new_lines = []
    next_is_arm64_sha = False
    next_is_x86_sha = False
    for line in lines:
        if "aarch64-apple-darwin" in line and "url" in line:
            next_is_arm64_sha = True
            new_lines.append(line)
        elif "x86_64-apple-darwin" in line and "url" in line:
            next_is_x86_sha = True
            new_lines.append(line)
        elif next_is_arm64_sha and "sha256" in line:
            new_lines.append(re.sub(r'sha256 "[^"]*"', f'sha256 "{arm64_sha}"', line))
            next_is_arm64_sha = False
        elif next_is_x86_sha and "sha256" in line:
            new_lines.append(re.sub(r'sha256 "[^"]*"', f'sha256 "{x86_sha}"', line))
            next_is_x86_sha = False
        else:
            new_lines.append(line)

    with open(formula_path, "w") as f:
        f.write("\n".join(new_lines) + "\n")

    print(f"Updated {formula_path} to version {version}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Update Homebrew formula")
    parser.add_argument("--formula", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--base-url", required=True)
    parser.add_argument("--arm64-sha", required=True)
    parser.add_argument("--x86-sha", required=True)
    args = parser.parse_args()

    update_formula(args.formula, args.tag, args.base_url, args.arm64_sha, args.x86_sha)


if __name__ == "__main__":
    main()
