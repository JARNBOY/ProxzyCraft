#!/usr/bin/env bash
# Non-destructive clean-room release exporter for ProxzyCraft.
#
# Preserves all existing local branches (e.g., feat/safe-proxy-orchestrator, main)
# and NEVER force-pushes. Instead, it exports a clean public tree/repo free of
# historical Rust target/ build artifacts (~551 MiB .git bloat) and dev scratch files.
#
# Usage:
#   ./scripts/export-clean-release.sh [--orphan | --preserve-history] [--dest <dir>] [--tag <tag>]

set -euo pipefail

MODE="orphan"
DEST="/tmp/proxzycraft-public-release"
TAG=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --orphan)
      MODE="orphan"
      shift
      ;;
    --preserve-history)
      MODE="preserve-history"
      shift
      ;;
    --dest)
      DEST="${2:?--dest requires a directory path}"
      shift 2
      ;;
    --tag)
      TAG="${2:?--tag requires a git tag or version string}"
      shift 2
      ;;
    -h|--help)
      sed -n '1,16p' "$0"
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

REPO_ROOT="$(git rev-parse --show-toplevel)"
SOURCE_HEAD="$(git -C "$REPO_ROOT" rev-parse --short HEAD)"
SOURCE_BRANCH="$(git -C "$REPO_ROOT" rev-parse --abbrev-ref HEAD)"

if [[ -n "$(git -C "$REPO_ROOT" status --porcelain)" ]]; then
  echo "Warning: working tree has uncommitted changes; exporter reads committed HEAD ($SOURCE_HEAD on $SOURCE_BRANCH)." >&2
fi

if [[ -e "$DEST" ]]; then
  rm -rf "$DEST"
fi

if [[ "$MODE" == "orphan" ]]; then
  mkdir -p "$DEST"
  git -C "$REPO_ROOT" archive --format=tar HEAD | tar -x -C "$DEST"

  # Defense-in-depth strip of internal workbench paths if any remained in HEAD.
  rm -rf \
    "$DEST/target" \
    "$DEST/app/target" \
    "$DEST/app/shots" \
    "$DEST/checks" \
    "$DEST/shots" \
    "$DEST/attachments" \
    "$DEST/investigation.cany" \
    "$DEST/.scratch" \
    "$DEST/inbox" \
    "$DEST/pinned" \
    "$DEST/capture" \
    "$DEST/reports" \
    "$DEST/build-activity.log" \
    "$DEST/craft.json" \
    "$DEST/feed.json" \
    "$DEST/usage.json" \
    "$DEST/app/Cargo.lock" \
    "$DEST/app/README.md" \
    "$DEST/decisions.tsv"

  git -C "$DEST" init -q -b main
  git -C "$DEST" add -A
  COMMIT_MSG="feat(release): initial clean-room public MIT release (${TAG:-v0.1.0}, from $SOURCE_HEAD)"
  git -C "$DEST" commit -q -m "$COMMIT_MSG"
  if [[ -n "$TAG" ]]; then
    git -C "$DEST" tag "$TAG"
  fi
  git -C "$DEST" gc --aggressive --prune=now >/dev/null 2>&1 || true
else
  if ! command -v git-filter-repo >/dev/null 2>&1; then
    echo "Error: --preserve-history requires 'git-filter-repo' (brew install git-filter-repo)." >&2
    exit 1
  fi
  git clone --no-local --branch "$SOURCE_BRANCH" "$REPO_ROOT" "$DEST"
  git -C "$DEST" filter-repo --force \
    --invert-paths \
    --path target \
    --path app/target \
    --path app/shots \
    --path checks \
    --path shots \
    --path attachments \
    --path investigation.cany \
    --path .scratch \
    --path inbox \
    --path pinned \
    --path capture \
    --path reports \
    --path build-activity.log \
    --path craft.json \
    --path feed.json \
    --path usage.json \
    --path app/Cargo.lock \
    --path app/README.md \
    --path decisions.tsv
  git -C "$DEST" reflog expire --expire=now --all
  git -C "$DEST" gc --aggressive --prune=now
fi

echo "Clean release exported to: $DEST (mode=$MODE)"
du -sh "$DEST/.git"

