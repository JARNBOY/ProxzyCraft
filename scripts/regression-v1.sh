#!/usr/bin/env bash
# Unified v1.0.0 pre-release regression gate for ProxzyCraft.
#
# Verifies:
#   1. Version parity (app/Cargo.toml == 1.0.1 and packages/proxzy-client/package.json == 1.0.1)
#   2. Full Rust unit & integration test suite (cargo test)
#   3. Compiled binary + TypeScript typecheck (when tsc is available) + live headless E2E via @proxzy/client
#   4. Clean Release export hygiene + leak assertions + offline cargo check in exported tree
#
# Usage:
#   ./scripts/regression-v1.sh [--skip-release-build]

set -euo pipefail

SKIP_RELEASE_BUILD=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-release-build)
      SKIP_RELEASE_BUILD=true
      shift
      ;;
    -h|--help)
      sed -n '1,15p' "$0"
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
EXPECTED_VERSION="1.0.1"

TMP_DIR=""
cleanup() {
  if [[ -n "$TMP_DIR" && -d "$TMP_DIR" ]]; then
    rm -rf "$TMP_DIR"
  fi
}
trap cleanup EXIT

step() {
  printf '\n==> [regression-v1] %s\n' "$1"
}

resolve_target_dir() {
  local meta_target=""
  if command -v node >/dev/null 2>&1; then
    meta_target="$(
      cargo metadata --manifest-path "$REPO_ROOT/app/Cargo.toml" --format-version 1 --no-deps 2>/dev/null \
        | node -e 'const fs = require("fs"); const data = JSON.parse(fs.readFileSync(0, "utf8")); process.stdout.write(data.target_directory || "");' \
        || true
    )"
  fi
  if [[ -n "$meta_target" && -d "$meta_target" ]]; then
    printf '%s\n' "$meta_target"
  elif [[ -d "$REPO_ROOT/target" ]]; then
    printf '%s\n' "$REPO_ROOT/target"
  else
    printf '%s\n' "$REPO_ROOT/app/target"
  fi
}

find_proxzy_binary() {
  local target_dir="$1"
  local profile="$2"
  local candidate=""
  for candidate in \
    "$target_dir/$profile/proxzy" \
    "$target_dir/$profile/proxzy.exe" \
    "$REPO_ROOT/target/$profile/proxzy" \
    "$REPO_ROOT/target/$profile/proxzy.exe" \
    "$REPO_ROOT/app/target/$profile/proxzy" \
    "$REPO_ROOT/app/target/$profile/proxzy.exe"
  do
    if [[ -x "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  return 1
}

step "1/4 Verifying v${EXPECTED_VERSION} version parity"
CARGO_VERSION="$(
  awk '
    /^\[package\]/ { in_pkg = 1; next }
    /^\[/ { in_pkg = 0 }
    in_pkg && /^version[[:space:]]*=/ {
      gsub(/^version[[:space:]]*=[[:space:]]*"/, "", $0)
      gsub(/".*$/, "", $0)
      print $0
      exit
    }
  ' "$REPO_ROOT/app/Cargo.toml"
)"
PKG_VERSION="$(
  node -e 'const fs = require("fs"); const pkg = JSON.parse(fs.readFileSync(process.argv[1], "utf8")); process.stdout.write(pkg.version || "");' \
    "$REPO_ROOT/packages/proxzy-client/package.json"
)"

echo "app/Cargo.toml version: $CARGO_VERSION"
echo "packages/proxzy-client/package.json version: $PKG_VERSION"
if [[ "$CARGO_VERSION" != "$EXPECTED_VERSION" || "$PKG_VERSION" != "$EXPECTED_VERSION" ]]; then
  echo "ERROR: Version parity check failed (expected $EXPECTED_VERSION in both manifests)." >&2
  exit 1
fi

step "2/4 Running Rust test suite"
cargo test --manifest-path "$REPO_ROOT/app/Cargo.toml"

step "3/4 Building binary and running TypeScript + live headless E2E suite"
PROFILE="debug"
cargo build --manifest-path "$REPO_ROOT/app/Cargo.toml"
if [[ "$SKIP_RELEASE_BUILD" == "false" ]]; then
  PROFILE="release"
  cargo build --release --manifest-path "$REPO_ROOT/app/Cargo.toml"
else
  echo "Skipping --release build (--skip-release-build set); using debug binary for E2E."
fi

TARGET_DIR="$(resolve_target_dir)"
BIN_PATH="$(find_proxzy_binary "$TARGET_DIR" "$PROFILE")" || {
  echo "ERROR: Could not locate compiled proxzy binary under $TARGET_DIR/$PROFILE" >&2
  exit 1
}
echo "Using compiled binary: $BIN_PATH"

TSC_BIN=""
if [[ -x "$REPO_ROOT/packages/proxzy-client/node_modules/.bin/tsc" ]]; then
  TSC_BIN="$REPO_ROOT/packages/proxzy-client/node_modules/.bin/tsc"
elif command -v tsc >/dev/null 2>&1; then
  TSC_BIN="$(command -v tsc)"
fi

if [[ -n "$TSC_BIN" ]]; then
  echo "Running TypeScript typecheck via $TSC_BIN..."
  "$TSC_BIN" --noEmit -p "$REPO_ROOT/packages/proxzy-client/tsconfig.json"
else
  echo "SKIP: tsc not installed locally (node native strip-types smoke/E2E still runs)."
fi

PROXZY_E2E_BIN="$BIN_PATH" node "$REPO_ROOT/packages/proxzy-client/test/smoke.ts"

step "4/4 Verifying Clean Release export hygiene and offline build"
if [[ -n "$(git -C "$REPO_ROOT" status --porcelain)" ]]; then
  echo "Note: working tree has uncommitted changes; clean-release export reads committed HEAD."
fi

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/proxzycraft-regression.XXXXXX")"
EXPORT_DIR="$TMP_DIR/clean-release"
"$REPO_ROOT/scripts/export-clean-release.sh" --orphan --dest "$EXPORT_DIR"

FORBIDDEN_PATHS=(
  ".scratch"
  ".ua"
  "target"
  "app/target"
  "build-activity.log"
  "checks"
  "shots"
  "app/shots"
  "attachments"
  "investigation.cany"
  "inbox"
  "pinned"
  "capture"
  "reports"
  "craft.json"
  "feed.json"
  "usage.json"
  "decisions.tsv"
  "app/Cargo.lock"
  "app/README.md"
)

for rel in "${FORBIDDEN_PATHS[@]}"; do
  if [[ -e "$EXPORT_DIR/$rel" ]]; then
    echo "ERROR: Forbidden path leaked into clean release export: $rel" >&2
    exit 1
  fi
done

LEAKED_FILES="$(
  find "$EXPORT_DIR" -path "$EXPORT_DIR/.git" -prune -o \
    \( -name '.DS_Store' -o -name '*.pem' -o -name '*.key' -o -name '*.cer' -o -name 'control.token' -o -name '*.token' \) \
    -print
)"
if [[ -n "$LEAKED_FILES" ]]; then
  echo "ERROR: Sensitive/OS files leaked into clean release export:" >&2
  echo "$LEAKED_FILES" >&2
  exit 1
fi

REQUIRED_DOCS=(
  "README.md"
  "PRIVACY.md"
  "CONTRIBUTING.md"
  "CODE_OF_CONDUCT.md"
  "SECURITY.md"
  "LICENSE"
  "THIRD_PARTY_NOTICES.md"
  "Cargo.toml"
  "Cargo.lock"
  "app/Cargo.toml"
  "packages/proxzy-client/package.json"
  "scripts/export-clean-release.sh"
)

for doc in "${REQUIRED_DOCS[@]}"; do
  if [[ ! -e "$EXPORT_DIR/$doc" ]]; then
    echo "ERROR: Expected public release file missing from export: $doc" >&2
    exit 1
  fi
done

cargo check --manifest-path "$EXPORT_DIR/app/Cargo.toml" --offline

printf '\n[regression-v1] ALL CHECKS PASSED for ProxzyCraft v%s.\n' "$EXPECTED_VERSION"
