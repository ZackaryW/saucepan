"""CI-only release packaging; Saucepan and its shim remain Rust-only binaries."""

import argparse
import hashlib
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import tomllib
import zipfile
import sdk_release


TARGETS = (
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
    "i686-pc-windows-msvc",
    "x86_64-unknown-linux-musl",
    "universal-apple-darwin",
)
INSTALLERS = ('install.ps1', 'install.sh')
# Verification-only second builds of the candidate (distinct build ID); never published.
TWIN_TARGETS = ("x86_64-pc-windows-msvc", "aarch64-apple-darwin", "x86_64-unknown-linux-musl")
TWIN_PREFIX = "verify-twin-"


def installer_bytes(name, tag, commit):
    validate_tag(tag)
    if name not in INSTALLERS or not re.fullmatch(r'[0-9a-f]{40}', commit):
        raise ValueError('invalid installer identity')
    source = (Path('scripts') / name).read_text(encoding='utf-8')
    if '@SAUCEPAN_RELEASE_TAG@' not in source or '@SAUCEPAN_RELEASE_COMMIT@' not in source:
        raise ValueError('installer source lacks release identity markers')
    return source.replace('@SAUCEPAN_RELEASE_TAG@', tag).replace('@SAUCEPAN_RELEASE_COMMIT@', commit).encode('utf-8')


def installers(tag, directory, commit):
    payloads = {name: installer_bytes(name, tag, commit) for name in INSTALLERS}
    directory.mkdir(parents=True, exist_ok=True)
    if any((directory / name).exists() for name in INSTALLERS):
        raise FileExistsError('installer assets already exist')
    for name, content in payloads.items():
        with (directory / name).open('xb') as output:
            output.write(content)


def validate_tag(tag, manifest=Path("Cargo.toml")):
    version = tomllib.loads(manifest.read_text(encoding="utf-8"))["package"]["version"]
    if not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?", tag):
        raise ValueError("expected vMAJOR.MINOR.PATCH or a prerelease tag")
    if tag != f"v{version}":
        raise ValueError(f"tag {tag!r} does not match Cargo.toml version {version!r}")
    return version


def asset_name(tag, target):
    if target not in TARGETS:
        raise ValueError(f"unsupported release target: {target}")
    return f"saucepan-{tag}-{target}.zip"


def package(tag, target, binary_dir, destination, twin=False):
    validate_tag(tag)
    if twin and target not in TWIN_TARGETS:
        raise ValueError(f"no verification twin for target: {target}")
    suffix = ".exe" if "windows" in target else ""
    inputs = [
        (binary_dir / f"{name}{suffix}", f"{name}{suffix}", 0o755)
        for name in ("saucepan", "saucepan-shim")
    ] + [(Path("LICENSE"), "LICENSE", 0o644)]
    for source, _, _ in inputs:
        if source.is_symlink() or not source.is_file() or source.stat().st_size == 0:
            raise ValueError(f"missing or invalid package input: {source}")
    destination.mkdir(parents=True, exist_ok=True)
    archive = destination / (f"{TWIN_PREFIX}{target}.zip" if twin else asset_name(tag, target))
    # Set Unix mode explicitly: artifact transfer and Windows builds lose it.
    with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_DEFLATED) as output:
        for source, name, mode in inputs:
            entry = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            entry.create_system = 3
            entry.external_attr = (stat.S_IFREG | mode) << 16
            entry.compress_type = zipfile.ZIP_DEFLATED
            with source.open("rb") as reader, output.open(entry, "w") as writer:
                shutil.copyfileobj(reader, writer)
    return archive


def checksums(tag, directory, commit=None):
    validate_tag(tag)
    if commit is None:
        commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
    binaries = {asset_name(tag, target) for target in TARGETS}
    expected = binaries | set(INSTALLERS) | sdk_release.verify_manifest(tag, commit, directory)
    actual = {p.name for p in directory.iterdir()}
    if any(name.startswith(TWIN_PREFIX) for name in actual):
        raise ValueError("verification twins must not be published")
    if actual != expected:
        raise ValueError(f"release assets differ: missing={expected - actual}, extra={actual - expected}")
    lines = []
    for name in sorted(expected):
        path = directory / name
        if path.is_symlink() or not path.is_file():
            raise ValueError('release asset is not a regular file')
        if name in INSTALLERS and path.read_bytes() != installer_bytes(name, tag, commit):
            raise ValueError('installer differs from the release tag/commit source')
        suffix = ".exe" if "windows" in name else ""
        if name in binaries:
            with zipfile.ZipFile(path) as archive:
                names = [f"saucepan{suffix}", f"saucepan-shim{suffix}", "LICENSE"]
                if sorted(archive.namelist()) != sorted(names) or archive.testzip() is not None:
                    raise ValueError(f"invalid archive contents: {path}")
                for entry in archive.infolist():
                    mode = entry.external_attr >> 16
                    if entry.file_size == 0 or not stat.S_ISREG(mode):
                        raise ValueError(f"invalid archive member: {entry.filename}")
                    if entry.filename != "LICENSE" and mode & 0o111 != 0o111:
                        raise ValueError(f"missing executable permissions: {entry.filename}")
        with path.open("rb") as reader:
            digest = hashlib.file_digest(reader, "sha256").hexdigest()
        lines.append(f"{digest}  {name}\n")
    (directory / "SHA256SUMS").write_text("".join(lines), encoding="utf-8", newline="\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("metadata")
    build = commands.add_parser("package")
    build.add_argument("--target", required=True, choices=TARGETS)
    build.add_argument("--binary-dir", type=Path, required=True)
    build.add_argument("--destination", type=Path, default=Path("dist"))
    build.add_argument("--twin", action="store_true", help="package a verification twin, never published")
    check = commands.add_parser("checksums")
    check.add_argument("--directory", type=Path, default=Path("dist"))
    sdk = commands.add_parser('sdk-manifest')
    sdk.add_argument('--directory', type=Path, default=Path('dist'))
    installer = commands.add_parser('installers')
    installer.add_argument('--directory', type=Path, default=Path('dist'))
    args = parser.parse_args()
    tag = os.environ["RELEASE_TAG"]
    validate_tag(tag)
    if args.command == "metadata":
        commit = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
        with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
            output.write(f"tag={tag}\ncommit={commit}\nprerelease={str('-' in tag).lower()}\n")
    elif args.command == "package":
        print(package(tag, args.target, args.binary_dir, args.destination, twin=args.twin))
    else:
        commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
        if os.environ.get('RELEASE_COMMIT', commit) != commit:
            raise ValueError('checkout differs from requested release commit')
        if args.command == 'sdk-manifest':
            sdk_release.write_manifest(tag, commit, args.directory)
        elif args.command == 'installers':
            installers(tag, args.directory, commit)
        else:
            checksums(tag, args.directory, commit)


if __name__ == "__main__":
    main()
