#!/usr/bin/env bash
# Unit tests for ci/resolve_toolchain.sh using local git fixtures.
#
# Usage: bash ci/resolve_toolchain_test.sh
#
# Optional environment variables:
#   GIT_TARBALL  Path to a relocatable git tarball containing bin/git.
#   JQ_BIN       Path to a jq-compatible executable (for example, jaq).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
readonly RESOLVE_SCRIPT="${SCRIPT_DIR}/resolve_toolchain.sh"

WORK_DIR=""
RUN_STATUS=0
RUN_STDOUT=""
RUN_STDERR=""

cleanup() {
  if [[ -n "${WORK_DIR}" ]]; then
    rm -rf "${WORK_DIR}"
  fi
}
trap cleanup EXIT

# Prepares an isolated temp directory, git environment, and optional tool PATH.
setup_environment() {
  local tmp_root="${TEST_TMPDIR:-${TMPDIR:-/tmp}}"
  WORK_DIR="$(mktemp -d "${tmp_root%/}/resolve_toolchain_test.XXXXXX")"
  if [[ -n "${TEST_TMPDIR:-}" ]]; then
    export TMPDIR="${TEST_TMPDIR}"
  fi

  export HOME="${WORK_DIR}/home"
  mkdir -p "${HOME}"
  export GIT_CONFIG_NOSYSTEM=1
  export GIT_CONFIG_GLOBAL=/dev/null
  export GIT_AUTHOR_NAME="Test Author"
  export GIT_AUTHOR_EMAIL="test@example.com"
  export GIT_COMMITTER_NAME="Test Author"
  export GIT_COMMITTER_EMAIL="test@example.com"
  unset CRUBIT_REPO TEST_MATRIX_DEPTH

  local tool_bin="${WORK_DIR}/bin"
  mkdir -p "${tool_bin}"

  if [[ -n "${GIT_TARBALL:-}" ]]; then
    local git_dir="${WORK_DIR}/git"
    mkdir -p "${git_dir}"
    tar -xzf "${GIT_TARBALL}" -C "${git_dir}"
    if [[ -d "${git_dir}/lib/git-core" ]]; then
      export GIT_EXEC_PATH="${git_dir}/lib/git-core"
    fi
    if [[ -d "${git_dir}/share/git-core/templates" ]]; then
      export GIT_TEMPLATE_DIR="${git_dir}/share/git-core/templates"
    fi
    PATH="${git_dir}/bin:${PATH}"
  fi

  if [[ -n "${JQ_BIN:-}" ]]; then
    if [[ ! -x "${JQ_BIN}" ]]; then
      echo "JQ_BIN is not executable: ${JQ_BIN}" >&2
      exit 1
    fi
    local jq_abs
    jq_abs="$(cd "$(dirname "${JQ_BIN}")" && pwd)/$(basename "${JQ_BIN}")"
    ln -s "${jq_abs}" "${tool_bin}/jq"
    PATH="${tool_bin}:${PATH}"
  fi
  export PATH
}

# Initializes a fresh local git repository and prints its path.
init_repo() {
  local repo
  repo="$(mktemp -d "${WORK_DIR}/repo.XXXXXX")"
  git init -q "${repo}"
  echo "${repo}"
}

# Appends a commit on refs/heads/main and prints its 40-character SHA.
add_crubit_commit() {
  local repo="$1"
  local message="$2"
  local empty_tree
  empty_tree="$(git -C "${repo}" mktree </dev/null)"

  local parent_args=()
  if git -C "${repo}" rev-parse --verify -q refs/heads/main >/dev/null; then
    local parent_sha
    parent_sha="$(git -C "${repo}" rev-parse refs/heads/main)"
    parent_args=(-p "${parent_sha}")
  fi

  local sha
  sha="$(git -C "${repo}" commit-tree "${empty_tree}" \
    ${parent_args[@]+"${parent_args[@]}"} -m "${message}")"
  git -C "${repo}" update-ref refs/heads/main "${sha}"
  echo "${sha}"
}

# Appends an orphan-history commit on refs/heads/test-matrix with a 160000
# `crubit` gitlink entry and a `matrix.json` blob.
add_test_matrix_commit() {
  local repo="$1"
  local crubit_sha="$2"
  local matrix_json="$3"

  local blob_sha
  blob_sha="$(git -C "${repo}" hash-object -w --stdin <<<"${matrix_json}")"

  local tree_sha
  tree_sha="$(printf '160000 commit %s\tcrubit\n100644 blob %s\tmatrix.json\n' \
    "${crubit_sha}" "${blob_sha}" | git -C "${repo}" mktree)"

  local parent_args=()
  if git -C "${repo}" rev-parse --verify -q refs/heads/test-matrix \
    >/dev/null; then
    local parent_sha
    parent_sha="$(git -C "${repo}" rev-parse refs/heads/test-matrix)"
    parent_args=(-p "${parent_sha}")
  fi

  local commit_sha
  commit_sha="$(git -C "${repo}" commit-tree "${tree_sha}" \
    ${parent_args[@]+"${parent_args[@]}"} -m "test-matrix for ${crubit_sha}")"
  git -C "${repo}" update-ref refs/heads/test-matrix "${commit_sha}"
}

# Executes resolve_toolchain.sh with the given environment assignments.
# Populates RUN_STATUS, RUN_STDOUT, and RUN_STDERR.
run_resolve() {
  local out_file="${WORK_DIR}/last_stdout"
  local err_file="${WORK_DIR}/last_stderr"
  RUN_STATUS=0
  env "$@" bash "${RESOLVE_SCRIPT}" >"${out_file}" 2>"${err_file}" \
    || RUN_STATUS=$?
  RUN_STDOUT="$(cat "${out_file}")"
  RUN_STDERR="$(cat "${err_file}")"
}

assert_eq() {
  local expected="$1"
  local actual="$2"
  local context="$3"
  if [[ "${expected}" != "${actual}" ]]; then
    echo "FAIL (${context}):" >&2
    echo "  expected: [${expected}]" >&2
    echo "  actual:   [${actual}]" >&2
    if [[ -n "${RUN_STDERR}" ]]; then
      echo "  stderr:   [${RUN_STDERR}]" >&2
    fi
    exit 1
  fi
}

assert_ne() {
  local unexpected="$1"
  local actual="$2"
  local context="$3"
  if [[ "${unexpected}" == "${actual}" ]]; then
    echo "FAIL (${context}): value unexpectedly equal to [${unexpected}]" >&2
    if [[ -n "${RUN_STDOUT}" ]]; then
      echo "  stdout:   [${RUN_STDOUT}]" >&2
    fi
    if [[ -n "${RUN_STDERR}" ]]; then
      echo "  stderr:   [${RUN_STDERR}]" >&2
    fi
    exit 1
  fi
}

assert_contains() {
  local haystack="$1"
  local needle="$2"
  local context="$3"
  if [[ "${haystack}" != *"${needle}"* ]]; then
    echo "FAIL (${context}):" >&2
    echo "  expected substring: [${needle}]" >&2
    echo "  actual:             [${haystack}]" >&2
    exit 1
  fi
}

# Scenario 1: Failing nightly-0 falls back to the newest passing nightly-1.
test_failing_nightly_0_picks_passing_nightly_1() {
  local repo
  repo="$(init_repo)"
  local crubit_sha
  crubit_sha="$(add_crubit_commit "${repo}" "crubit commit 1")"
  git -C "${repo}" tag latest "${crubit_sha}"

  local matrix_json
  matrix_json="$(cat <<'EOF'
{
  "include": [
    {"name": "nightly-0", "version": "nightly-2026-04-10", "passing": false},
    {"name": "nightly-1", "version": "nightly-2026-04-09", "passing": true},
    {"name": "nightly-2", "version": "nightly-2026-04-08", "passing": true}
  ]
}
EOF
)"
  add_test_matrix_commit "${repo}" "${crubit_sha}" "${matrix_json}"

  run_resolve "CRUBIT_REPO=file://${repo}"

  local expected_stdout
  expected_stdout="$(printf 'crubit_sha=%s\ntoolchain=nightly-2026-04-09' \
    "${crubit_sha}")"
  assert_eq "0" "${RUN_STATUS}" "exit status"
  assert_eq "${expected_stdout}" "${RUN_STDOUT}" "stdout"
}

# Scenario 2: Mid-run skew where test-matrix HEAD moved ahead of latest tag.
test_mid_run_test_matrix_skew_matches_latest_tag() {
  local repo
  repo="$(init_repo)"
  local old_sha
  old_sha="$(add_crubit_commit "${repo}" "crubit older")"
  local new_sha
  new_sha="$(add_crubit_commit "${repo}" "crubit newer")"
  git -C "${repo}" tag latest "${old_sha}"

  local old_matrix
  old_matrix="$(cat <<'EOF'
{
  "include": [
    {"name": "nightly-0", "version": "nightly-2026-04-05", "passing": true}
  ]
}
EOF
)"
  local new_matrix
  new_matrix="$(cat <<'EOF'
{
  "include": [
    {"name": "nightly-0", "version": "nightly-2026-04-10", "passing": true}
  ]
}
EOF
)"
  add_test_matrix_commit "${repo}" "${old_sha}" "${old_matrix}"
  add_test_matrix_commit "${repo}" "${new_sha}" "${new_matrix}"

  run_resolve "CRUBIT_REPO=file://${repo}"

  local expected_stdout
  expected_stdout="$(printf 'crubit_sha=%s\ntoolchain=nightly-2026-04-05' \
    "${old_sha}")"
  assert_eq "0" "${RUN_STATUS}" "exit status"
  assert_eq "${expected_stdout}" "${RUN_STDOUT}" "stdout"
}

# Scenario 3: Annotated refs/tags/latest tag is peeled to the target commit SHA.
test_annotated_latest_tag_is_peeled() {
  local repo
  repo="$(init_repo)"
  local crubit_sha
  crubit_sha="$(add_crubit_commit "${repo}" "crubit annotated release")"
  git -C "${repo}" tag -a latest -m "Release latest" "${crubit_sha}"

  local tag_obj_sha
  tag_obj_sha="$(git -C "${repo}" rev-parse refs/tags/latest)"
  assert_ne "${crubit_sha}" "${tag_obj_sha}" "annotated tag object differs"

  local matrix_json
  matrix_json="$(cat <<'EOF'
{
  "include": [
    {"name": "nightly-0", "version": "nightly-2026-04-07", "passing": true}
  ]
}
EOF
)"
  add_test_matrix_commit "${repo}" "${crubit_sha}" "${matrix_json}"

  run_resolve "CRUBIT_REPO=file://${repo}"

  local expected_stdout
  expected_stdout="$(printf 'crubit_sha=%s\ntoolchain=nightly-2026-04-07' \
    "${crubit_sha}")"
  assert_eq "0" "${RUN_STATUS}" "exit status"
  assert_eq "${expected_stdout}" "${RUN_STDOUT}" "stdout"
}

# Scenario 4: Unsorted/chronological .include and leading non-date nightly-0.
test_unsorted_include_and_non_date_entries() {
  local repo
  repo="$(init_repo)"
  local crubit_sha
  crubit_sha="$(add_crubit_commit "${repo}" "crubit unsorted matrix")"
  git -C "${repo}" tag latest "${crubit_sha}"

  local matrix_json
  matrix_json="$(cat <<'EOF'
{
  "include": [
    {"name": "nightly-0", "version": "nightly-0", "passing": true},
    {"name": "nightly-3", "version": "nightly-2026-04-01", "passing": true},
    {"name": "nightly-1", "version": "nightly-2026-04-12", "passing": true},
    {"name": "nightly-2", "version": "nightly-2026-04-05", "passing": true},
    {"name": "nightly-bad-1", "version": "nightly-latest", "passing": true},
    {
      "name": "nightly-bad-2",
      "version": "nightly-2026-04-99\n",
      "passing": true
    },
    {
      "name": "nightly-bad-3",
      "version": "nightly-2026-04-20",
      "passing": "true"
    },
    {"name": "nightly-bad-4", "version": 20260421, "passing": true},
    "not-an-object"
  ]
}
EOF
)"
  add_test_matrix_commit "${repo}" "${crubit_sha}" "${matrix_json}"

  run_resolve "CRUBIT_REPO=file://${repo}"

  local expected_stdout
  expected_stdout="$(printf 'crubit_sha=%s\ntoolchain=nightly-2026-04-12' \
    "${crubit_sha}")"
  assert_eq "0" "${RUN_STATUS}" "exit status"
  assert_eq "${expected_stdout}" "${RUN_STDOUT}" "stdout"
}

# Scenario 5: All nightlies failing while stable-0 passes must fail.
test_all_nightlies_failing_with_passing_stable_fails() {
  local repo
  repo="$(init_repo)"
  local crubit_sha
  crubit_sha="$(add_crubit_commit "${repo}" "crubit no passing nightly")"
  git -C "${repo}" tag latest "${crubit_sha}"

  local matrix_json
  matrix_json="$(cat <<'EOF'
{
  "include": [
    {"name": "nightly-0", "version": "nightly-2026-04-10", "passing": false},
    {"name": "nightly-1", "version": "nightly-2026-04-09", "passing": false},
    {"name": "stable-0", "version": "1.86.0", "passing": true},
    {"name": "stable-1", "version": "stable-2026-04-10", "passing": true}
  ]
}
EOF
)"
  add_test_matrix_commit "${repo}" "${crubit_sha}" "${matrix_json}"

  run_resolve "CRUBIT_REPO=file://${repo}"

  assert_ne "0" "${RUN_STATUS}" "exit status"
  assert_eq "" "${RUN_STDOUT}" "stdout should be empty on failure"
  assert_contains "${RUN_STDERR}" "No passing Rust nightly found" "stderr"
}

# Scenario 6a: Missing refs/tags/latest fails with an actionable error.
test_missing_latest_tag_fails() {
  local repo
  repo="$(init_repo)"
  local crubit_sha
  crubit_sha="$(add_crubit_commit "${repo}" "crubit untagged")"

  local matrix_json
  matrix_json="$(cat <<'EOF'
{"include": [{"version": "nightly-2026-04-10", "passing": true}]}
EOF
)"
  add_test_matrix_commit "${repo}" "${crubit_sha}" "${matrix_json}"

  run_resolve "CRUBIT_REPO=file://${repo}"

  assert_ne "0" "${RUN_STATUS}" "exit status"
  assert_eq "" "${RUN_STDOUT}" "stdout should be empty on failure"
  assert_contains "${RUN_STDERR}" "Could not resolve Crubit's latest tag" \
    "stderr"
}

# Scenario 6b: TEST_MATRIX_DEPTH=1 misses an older matching commit, while
# depth=2 finds it.
test_shallow_depth_miss_fails() {
  local repo
  repo="$(init_repo)"
  local old_sha
  old_sha="$(add_crubit_commit "${repo}" "crubit depth older")"
  local new_sha
  new_sha="$(add_crubit_commit "${repo}" "crubit depth newer")"
  git -C "${repo}" tag latest "${old_sha}"

  local old_matrix
  old_matrix='{"include": [{"version": "nightly-2026-04-05", "passing": true}]}'
  local new_matrix
  new_matrix='{"include": [{"version": "nightly-2026-04-10", "passing": true}]}'
  add_test_matrix_commit "${repo}" "${old_sha}" "${old_matrix}"
  add_test_matrix_commit "${repo}" "${new_sha}" "${new_matrix}"

  run_resolve "CRUBIT_REPO=file://${repo}" "TEST_MATRIX_DEPTH=1"

  assert_ne "0" "${RUN_STATUS}" "depth=1 exit status"
  assert_eq "" "${RUN_STDOUT}" "depth=1 stdout should be empty"
  assert_contains "${RUN_STDERR}" \
    "Could not find Crubit ${old_sha} in last 1 commits of test-matrix." \
    "depth=1 stderr"

  run_resolve "CRUBIT_REPO=file://${repo}" "TEST_MATRIX_DEPTH=2"

  local expected_stdout
  expected_stdout="$(printf 'crubit_sha=%s\ntoolchain=nightly-2026-04-05' \
    "${old_sha}")"
  assert_eq "0" "${RUN_STATUS}" "depth=2 exit status"
  assert_eq "${expected_stdout}" "${RUN_STDOUT}" "depth=2 stdout"
}

# Scenario 7: Malformed JSON in matrix.json fails cleanly.
test_malformed_matrix_json_fails() {
  local repo
  repo="$(init_repo)"
  local crubit_sha
  crubit_sha="$(add_crubit_commit "${repo}" "crubit malformed json")"
  git -C "${repo}" tag latest "${crubit_sha}"

  add_test_matrix_commit "${repo}" "${crubit_sha}" '{"include": [bad json'

  run_resolve "CRUBIT_REPO=file://${repo}"

  assert_ne "0" "${RUN_STATUS}" "exit status"
  assert_eq "" "${RUN_STDOUT}" "stdout should be empty on failure"
  assert_contains "${RUN_STDERR}" "Invalid JSON in matrix.json" "stderr"
}

# Scenario 8: Valid non-object JSON root fails with no passing nightly error.
test_non_object_matrix_json_fails_cleanly() {
  local repo
  repo="$(init_repo)"
  local crubit_sha
  crubit_sha="$(add_crubit_commit "${repo}" "crubit array matrix")"
  git -C "${repo}" tag latest "${crubit_sha}"

  add_test_matrix_commit "${repo}" "${crubit_sha}" '[]'

  run_resolve "CRUBIT_REPO=file://${repo}"

  assert_ne "0" "${RUN_STATUS}" "exit status"
  assert_eq "" "${RUN_STDOUT}" "stdout should be empty on failure"
  assert_contains "${RUN_STDERR}" "No passing Rust nightly found" "stderr"
}

# Scenario 9: Multi-document JSON stream in matrix.json is rejected cleanly.
test_multi_document_matrix_json_fails() {
  local repo
  repo="$(init_repo)"
  local crubit_sha
  crubit_sha="$(add_crubit_commit "${repo}" "crubit multi-doc matrix")"
  git -C "${repo}" tag latest "${crubit_sha}"

  local matrix_json
  matrix_json="$(cat <<'EOF'
{"include": [{"version": "nightly-2026-04-09", "passing": true}]}
{"include": [{"version": "nightly-2026-04-10", "passing": true}]}
EOF
)"
  add_test_matrix_commit "${repo}" "${crubit_sha}" "${matrix_json}"

  run_resolve "CRUBIT_REPO=file://${repo}"

  assert_ne "0" "${RUN_STATUS}" "exit status"
  assert_eq "" "${RUN_STDOUT}" "stdout should be empty on failure"
  assert_contains "${RUN_STDERR}" "No passing Rust nightly found" "stderr"
}

FAILURES=0

run_test() {
  local name="$1"
  echo "=== RUN ${name}"
  local status=0
  set +e
  ( set -e; "${name}" )
  status=$?
  set -e
  if [[ "${status}" -eq 0 ]]; then
    echo "--- PASS ${name}"
  else
    echo "--- FAIL ${name}" >&2
    FAILURES=$((FAILURES + 1))
  fi
}

main() {
  setup_environment
  run_test test_failing_nightly_0_picks_passing_nightly_1
  run_test test_mid_run_test_matrix_skew_matches_latest_tag
  run_test test_annotated_latest_tag_is_peeled
  run_test test_unsorted_include_and_non_date_entries
  run_test test_all_nightlies_failing_with_passing_stable_fails
  run_test test_missing_latest_tag_fails
  run_test test_shallow_depth_miss_fails
  run_test test_malformed_matrix_json_fails
  run_test test_non_object_matrix_json_fails_cleanly
  run_test test_multi_document_matrix_json_fails
  if [[ "${FAILURES}" -ne 0 ]]; then
    echo "FAIL: ${FAILURES} resolve_toolchain test(s) failed." >&2
    exit 1
  fi
  echo "PASS: all resolve_toolchain tests passed."
}

main "$@"
