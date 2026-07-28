"""Resolve and download the saucepan release binary for the host platform.

Vendor this single file into another repository (e.g. as a git submodule) to
fetch a prebuilt `saucepan` binary without a Rust toolchain. Standard library
only - no pip install step required.

    python saucepan_resolver.py v0.2.0 [dest]

or as a module:

    from saucepan_resolver import resolve
    path = resolve("v0.2.0")
"""

import os
import platform
import stat
import sys
import urllib.request

OWNER = "ZackaryW"
REPO = "saucepan"

# (platform.system(), platform.machine()) -> release asset name
_ASSET_NAMES = {
    ("Darwin", "arm64"): "saucepan-aarch64-apple-darwin",
    ("Darwin", "x86_64"): "saucepan-x86_64-apple-darwin",
    ("Windows", "AMD64"): "saucepan-x86_64-pc-windows-msvc.exe",
    ("Windows", "ARM64"): "saucepan-aarch64-pc-windows-msvc.exe",
    ("Windows", "x86"): "saucepan-i686-pc-windows-msvc.exe",
    ("Linux", "x86_64"): "saucepan-x86_64-unknown-linux-musl",
}


def _asset_name(system=None, machine=None):
    system = system if system is not None else platform.system()
    machine = machine if machine is not None else platform.machine()
    try:
        return _ASSET_NAMES[(system, machine)]
    except KeyError:
        raise RuntimeError(
            f"no saucepan release asset for platform ({system}, {machine})"
        )


def resolve(version, dest=None):
    """Download the saucepan binary for `version` on the host platform.

    `version` must be an explicit release tag (e.g. "v0.2.0") - this never
    resolves "latest". Downloads directly from the release-asset URL (no
    GitHub REST API call, so no auth/rate-limit concerns). Returns the local
    path to the downloaded, executable binary.
    """
    if not version:
        raise ValueError('resolve() requires an explicit version, e.g. "v0.2.0"')

    asset = _asset_name()
    url = f"https://github.com/{OWNER}/{REPO}/releases/download/{version}/{asset}"

    if dest is None:
        dest = asset

    urllib.request.urlretrieve(url, dest)

    if platform.system() != "Windows":
        mode = os.stat(dest).st_mode
        os.chmod(dest, mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)

    return dest


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    if not argv:
        print("usage: saucepan_resolver.py <version> [dest]", file=sys.stderr)
        return 2

    version = argv[0]
    dest = argv[1] if len(argv) > 1 else None
    path = resolve(version, dest)
    print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
