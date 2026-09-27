#!/usr/bin/env bash
# DORMANT: no workflow invokes this — build.yml produces artifacts only and
# KOS-156 policy forbids publishing GitHub Releases without Jack's OK.
# Kept (and covered by scripts/test_release.py) for the future publish
# decision; wiring it up pushes a version commit + v* tag to main.
#
# Called only after every platform has built SOURCE_SHA with RELEASE_VERSION.
set -euo pipefail

git fetch origin "$DEFAULT_BRANCH" --tags
python3 scripts/release.py set "$RELEASE_VERSION"
git config user.name 'github-actions[bot]'
git config user.email '41898282+github-actions[bot]@users.noreply.github.com'
git add Cargo.toml Cargo.lock
tag="v$RELEASE_VERSION"
remote=$(git rev-parse "origin/$DEFAULT_BRANCH")
if [ "$remote" != "$SOURCE_SHA" ]; then
  # Permit only our own completed version commit when retrying publication.
  if [ "$(git rev-parse "$remote^")" != "$SOURCE_SHA" ] ||
     [ "$(git rev-parse "$remote^{tree}")" != "$(git write-tree)" ] ||
     [ "$(git rev-parse "$tag^{commit}" 2>/dev/null)" != "$remote" ]; then
    echo 'Default branch changed during the build. Rerun on the new commit.' >&2
    exit 1
  fi
  commit="$remote"
else
  if ! git diff --cached --quiet; then
    git -c core.hooksPath=/dev/null commit -m "chore: release v$RELEASE_VERSION"
  fi
  commit=$(git rev-parse HEAD)
fi
if git rev-parse --verify "refs/tags/$tag" >/dev/null 2>&1; then
  test "$(git rev-parse "$tag^{commit}")" = "$commit"
else
  git tag "$tag" "$commit"
fi
git push --atomic origin "$commit:refs/heads/$DEFAULT_BRANCH" "refs/tags/$tag"
