#!/usr/bin/env bash
# Prints the bindings that CI should build, as a JSON array in the GitHub
# Actions output format:
#
#   bindings=["elf","regex"]
#
# Usage: ci/changed_bindings.sh [BASE_SHA [HEAD_SHA]]
#
# A binding is a top-level directory containing a CMakeLists.txt.
#
# - Without a usable BASE_SHA (scheduled and manual runs, new branches), every
#   binding is built.
# - Otherwise only the bindings whose directories changed since BASE_SHA are
#   built. A change to anything outside a binding directory, such as
#   rust-toolchain.toml, .github/, ci/ or crubit_helpers/, builds every binding
#   because it can affect all of them. Top-level documentation builds nothing.

set -euo pipefail
cd "$(dirname "$0")/.."

base="${1:-}"
head="${2:-HEAD}"

all_bindings() {
  for cmake_file in */CMakeLists.txt; do
    echo "${cmake_file%/CMakeLists.txt}"
  done
}

print_output() {
  echo "bindings=$(jq -Rnc '[inputs | select(length > 0)]')"
}

if [[ -z "${base}" || "${base}" =~ ^0+$ ]] ||
  ! git cat-file -e "${base}^{commit}" 2>/dev/null; then
  all_bindings | print_output
  exit 0
fi

# Top-level entries touched by the change, ignoring top-level documentation.
touched="$(git diff --name-only "${base}...${head}" |
  sed -E '/^(README\.md|CONTRIBUTING\.md|LICENSE)$/d' |
  cut -d/ -f1 | sort -u)"

while IFS= read -r entry; do
  if [[ -n "${entry}" && ! -f "${entry}/CMakeLists.txt" ]]; then
    echo "${entry} is shared by all bindings; building every binding." >&2
    all_bindings | print_output
    exit 0
  fi
done <<<"${touched}"

printf '%s\n' "${touched}" | print_output
