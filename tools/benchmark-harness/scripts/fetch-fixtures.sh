#!/usr/bin/env bash
# Fetch an external fixture corpus for the benchmark harness.
#
# Kreuzberg's corpus is the reference we track (see PLAN.md §scoring),
# but individual PDFs inside it carry varied licenses, so we don't
# vendor them — the script clones the upstream and symlinks the
# markdown-ground-truth subset into ./fixtures/kreuzberg.
#
# Re-run any time; idempotent.

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
DEST="${SCRIPT_DIR}/../fixtures/kreuzberg"
UPSTREAM_DIR="${SCRIPT_DIR}/../.fixture-src/kreuzberg"
UPSTREAM_URL="https://github.com/Goldziher/kreuzberg.git"
# Pin so scoring numbers don't drift with upstream fixture churn.
UPSTREAM_REF="${KREUZBERG_REF:-main}"

mkdir -p "${DEST}" "$(dirname "${UPSTREAM_DIR}")"

if [[ ! -d "${UPSTREAM_DIR}/.git" ]]; then
  echo "cloning ${UPSTREAM_URL} → ${UPSTREAM_DIR}"
  git clone --depth 1 --branch "${UPSTREAM_REF}" "${UPSTREAM_URL}" "${UPSTREAM_DIR}"
else
  echo "updating ${UPSTREAM_DIR} to ${UPSTREAM_REF}"
  git -C "${UPSTREAM_DIR}" fetch --depth 1 origin "${UPSTREAM_REF}"
  git -C "${UPSTREAM_DIR}" checkout "${UPSTREAM_REF}"
fi

# Kreuzberg fixtures live under tools/benchmark-harness/fixtures/
# with parallel *.pdf and *.md files. Symlink so we don't duplicate
# hundreds of MB in our repo, and so re-running this script with a
# different UPSTREAM_REF works in place.
SRC="${UPSTREAM_DIR}/tools/benchmark-harness/fixtures"
if [[ ! -d "${SRC}" ]]; then
  echo "error: ${SRC} not found — upstream layout changed?" >&2
  exit 1
fi

rm -f "${DEST}"
ln -s "${SRC}" "${DEST}"

printf 'linked %s → %s\n' "${DEST}" "${SRC}"
printf 'fixture count (pdf): %d\n' \
  "$(find -L "${DEST}" -type f -name '*.pdf' | wc -l)"
printf 'ground-truth count (md): %d\n' \
  "$(find -L "${DEST}" -type f -name '*.md' | wc -l)"
