#!/usr/bin/env bash
# Prints the Crubit commit and a matching passing Rust nightly to build with, in
# the GitHub Actions output format:
#
#   crubit_sha=<commit>
#   toolchain=nightly-YYYY-MM-DD
#
# Usage: [CRUBIT_REPO=<url>] [TEST_MATRIX_DEPTH=<n>] ci/resolve_toolchain.sh
#
# Crubit's bindings generator uses unstable compiler internals, so a Crubit
# commit only builds with nightlies it was tested against. Crubit's daily
# `test-matrix` workflow tests a Crubit submodule commit against recent Rust
# nightlies, records pass/fail results in `matrix.json` on the `test-matrix`
# branch, and updates the `latest` tag to that commit. This resolves `latest`,
# finds the matching `test-matrix` commit, and selects the newest passing
# `nightly-YYYY-MM-DD` entry from its `matrix.json`.

set -euo pipefail

readonly CRUBIT_REPO="${CRUBIT_REPO:-https://github.com/google/crubit.git}"
readonly TEST_MATRIX_DEPTH="${TEST_MATRIX_DEPTH:-50}"

tmp_dir=""
cleanup() {
  if [[ -n "${tmp_dir}" ]]; then
    rm -rf "${tmp_dir}"
  fi
}
trap cleanup EXIT

main() {
  local ls_remote_out
  # git ls-remote sorts output by refname, so `refs/tags/latest^{}` (the peeled
  # commit on annotated tags) follows `refs/tags/latest`; `tail -n 1` selects it.
  if ! ls_remote_out="$(git ls-remote "${CRUBIT_REPO}" refs/tags/latest 'refs/tags/latest^{}')"; then
    echo "Could not query refs/tags/latest from ${CRUBIT_REPO}." >&2
    exit 1
  fi

  local crubit_sha
  crubit_sha="$(tail -n 1 <<<"${ls_remote_out}" | cut -f1)"
  if [[ ! "${crubit_sha}" =~ ^[0-9a-f]{40}$ ]]; then
    echo "Could not resolve Crubit's latest tag from ${CRUBIT_REPO}." >&2
    exit 1
  fi

  tmp_dir="$(mktemp -d)"
  git -C "${tmp_dir}" init -q
  if ! git -C "${tmp_dir}" fetch -q --depth="${TEST_MATRIX_DEPTH}" "${CRUBIT_REPO}" refs/heads/test-matrix; then
    echo "Could not fetch test-matrix branch from ${CRUBIT_REPO}." >&2
    exit 1
  fi

  local matched_commit=""
  local commit
  local sub_sha
  while IFS= read -r commit; do
    [[ -z "${commit}" ]] && continue
    if sub_sha="$(git -C "${tmp_dir}" rev-parse --verify -q "${commit}:crubit" 2>/dev/null)" &&
      [[ "${sub_sha}" == "${crubit_sha}" ]]; then
      matched_commit="${commit}"
      break
    fi
  done < <(git -C "${tmp_dir}" rev-list FETCH_HEAD)

  if [[ -z "${matched_commit}" ]]; then
    echo "Could not find Crubit ${crubit_sha} in last ${TEST_MATRIX_DEPTH} commits of test-matrix." >&2
    exit 1
  fi

  local matrix_json
  if ! matrix_json="$(git -C "${tmp_dir}" show "${matched_commit}:matrix.json")"; then
    echo "Could not read matrix.json from test-matrix commit ${matched_commit}." >&2
    exit 1
  fi

  local toolchain
  if ! toolchain="$(jq -r '
    [
      (if type == "object" then .include[]? else empty end)
      | select(
          type == "object"
          and (.version | type == "string")
          and (.version | test("\\Anightly-[0-9]{4}-[0-9]{2}-[0-9]{2}\\z"))
          and .passing == true
        )
      | .version
    ] | sort | .[-1] // empty
  ' <<<"${matrix_json}")"; then
    echo "Invalid JSON in matrix.json from test-matrix commit ${matched_commit}." >&2
    exit 1
  fi

  if [[ ! "${toolchain}" =~ ^nightly-[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]]; then
    echo "No passing Rust nightly found in matrix.json for Crubit ${crubit_sha}." >&2
    exit 1
  fi

  echo "Resolved Crubit ${crubit_sha} with ${toolchain} (from test-matrix ${matched_commit:0:10})." >&2
  echo "crubit_sha=${crubit_sha}"
  echo "toolchain=${toolchain}"
}

main "$@"
