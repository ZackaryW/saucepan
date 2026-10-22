#!/bin/sh
# Saucepan official installer; rendered from the release commit by scripts/release.py.
# Release-Tag: @SAUCEPAN_RELEASE_TAG@
# Release-Commit: @SAUCEPAN_RELEASE_COMMIT@
set -eu
version='@SAUCEPAN_RELEASE_TAG@'
root=${HOME:+$HOME/.saucepan}
operation=
migrate=false
expected=
base=https://github.com/ZackaryW/saucepan/releases/download
api=
fail() { printf '%s\n' "$*" >&2; exit 1; }
while [ "$#" -gt 0 ]; do
    case "$1" in
        --migrate-unmanaged) migrate=true; shift ;;
        --version|--root|--operation|--expected-sha256|--test-release-base|--test-github-api)
            [ "$#" -ge 2 ] || fail "Missing value for $1"
            case "$1" in
                --version) version=$2 ;; --root) root=$2 ;; --operation) operation=$2 ;;
                --expected-sha256) expected=$2 ;; --test-release-base) base=$2 ;; --test-github-api) api=$2 ;;
            esac
            shift 2 ;;
        *) fail "Unknown installer option: $1" ;;
    esac
done
for tool in curl unzip awk sort uname mktemp chmod; do command -v "$tool" >/dev/null 2>&1 || fail "Missing prerequisite: $tool"; done
if command -v sha256sum >/dev/null 2>&1; then hash() { sha256sum "$1" | awk '{print $1}'; }
elif command -v shasum >/dev/null 2>&1; then hash() { shasum -a 256 "$1" | awk '{print $1}'; }
else fail 'Missing prerequisite: sha256sum or shasum'; fi
printf '%s\n' "$version" | awk '/^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$/ {ok=1} END {exit !ok}' || fail 'Select an exact release with --version vMAJOR.MINOR.PATCH'
[ -n "$root" ] || fail 'Cannot resolve a home; supply an explicit --root PATH'
if [ "$migrate" = true ]; then
    [ -n "$operation" ] || fail 'Migration requires --operation UUID'
    [ "${#expected}" -eq 64 ] || fail 'Migration requires --expected-sha256 HASH'
    case "$expected" in *[!0-9a-fA-F]*) fail 'Invalid SHA256' ;; esac
elif [ -n "$expected" ]; then fail 'Expected digest requires --migrate-unmanaged'; fi
if [ -z "$operation" ]; then
    if [ -r /proc/sys/kernel/random/uuid ]; then operation=$(cat /proc/sys/kernel/random/uuid)
    elif command -v uuidgen >/dev/null 2>&1; then operation=$(uuidgen)
    else fail 'Supply --operation UUID (no UUID generator is available)'; fi
fi
printf '%s\n' "$operation" | awk -F- 'NF==5 && length($1)==8 && length($2)==4 && length($3)==4 && length($4)==4 && length($5)==12 && $0 !~ /[^0-9a-fA-F-]/ {ok=1} END {exit !ok}' || fail 'Operation must be a UUID'
case "$(uname -s)/$(uname -m)" in
    Darwin/arm64|Darwin/aarch64) target=aarch64-apple-darwin ;;
    Darwin/x86_64) target=x86_64-apple-darwin ;;
    Linux/x86_64) target=x86_64-unknown-linux-musl ;;
    *) fail 'Unsupported installer platform' ;;
esac
if [ "$base" != https://github.com/ZackaryW/saucepan/releases/download ]; then
    printf '%s\n' "$base" | awk '/^http:\/\/(127\.0\.0\.1|localhost):[0-9]+(\/[^?#]*)?$/ {ok=1} END {exit !ok}' || fail 'Test release base must be explicit loopback HTTP'
fi
if [ -n "$api" ]; then
    printf '%s\n' "$api" | awk '/^http:\/\/(127\.0\.0\.1|localhost):[0-9]+$/ {ok=1} END {exit !ok}' || fail 'Test API must be explicit loopback HTTP'
fi
scratch=$(mktemp -d "${TMPDIR:-/tmp}/saucepan-setup.XXXXXXXX")
scratch=$(CDPATH= cd -P "$scratch" && pwd -P)
cleanup() { case "$scratch" in */saucepan-setup.*) rm -rf -- "$scratch" || printf 'Could not remove installer scratch: %s\n' "$scratch" >&2 ;; esac; }
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
# Resolve existing ancestors without creating any part of the selected store.
case "$root" in /*) normalized=$root ;; *) normalized=$(pwd -P)/$root ;; esac
ancestor=$normalized
suffix=
while [ ! -d "$ancestor" ]; do suffix=/$(basename "$ancestor")$suffix; ancestor=$(dirname "$ancestor"); done
normalized=$(CDPATH= cd -P "$ancestor" && pwd -P)$suffix
case "$scratch/" in "$normalized/"*) fail 'Installer staging must be outside the selected store' ;; esac
printf 'Saucepan setup operation: %s\n' "$operation" >&2
asset=saucepan-$version-$target.zip
curl --fail --location --silent --show-error --max-time 120 "$base/$version/SHA256SUMS" -o "$scratch/SHA256SUMS"
curl --fail --location --silent --show-error --max-time 120 "$base/$version/$asset" -o "$scratch/archive.zip"
checksum=$(awk -v name="$asset" 'length($1)==64 && $1 !~ /[^0-9a-fA-F]/ && $2==name {n++; result=tolower($1)} END {if(n!=1) exit 1; print result}' "$scratch/SHA256SUMS") || fail 'Expected exactly one archive checksum'
[ "$(hash "$scratch/archive.zip")" = "$checksum" ] || fail 'Release archive checksum mismatch'
members=$(unzip -Z1 "$scratch/archive.zip" | LC_ALL=C sort)
[ "$members" = "$(printf '%s\n' LICENSE saucepan saucepan-shim | LC_ALL=C sort)" ] || fail 'Archive has missing, extra, duplicate or unsafe entries'
unzip -l "$scratch/archive.zip" | awk '$4=="LICENSE" || $4=="saucepan" || $4=="saucepan-shim" {n++; if($1<=0 || $1>536870912) bad=1} END {exit (n!=3 || bad)}' || fail 'Invalid archive entry size'
unzip -Z -l "$scratch/archive.zip" | awk '/^l/ {bad=1} END {exit bad}' || fail 'Archive contains links'
for name in saucepan saucepan-shim LICENSE; do unzip -p "$scratch/archive.zip" "$name" > "$scratch/$name"; done
chmod 755 "$scratch/saucepan" "$scratch/saucepan-shim"
"$scratch/saucepan" setup --help > "$scratch/help" 2>&1 || fail 'Selected manager lacks managed setup; obtain a setup-capable release'
awk '/--migrate-unmanaged/ {ok=1} END {exit !ok}' "$scratch/help" || fail 'Selected manager lacks managed setup'
set -- --root "$root"
if [ -n "$api" ]; then set -- "$@" --test-github-api "$api"; fi
set -- "$@" setup "$scratch/saucepan-shim" --tag "$version" --operation "$operation"
if [ "$migrate" = true ]; then set -- "$@" --migrate-unmanaged --expected-sha256 "$expected"; fi
code=0
"$scratch/saucepan" "$@" || code=$?
if [ "$code" -eq 0 ]; then printf 'Add the managed bin directory to PATH yourself: %s/bin\n' "$root" >&2; fi
exit "$code"
