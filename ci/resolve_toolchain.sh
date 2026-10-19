#!/usr/bin/env bash
# Prints the Crubit commit and the matching Rust nightly to build with, in the
# GitHub Actions output format:
#
#   crubit_sha=<commit>
#   toolchain=nightly-YYYY-MM-DD
#
# Usage: ci/resolve_toolchain.sh
#
# Crubit's bindings generator uses unstable compiler internals, so a Crubit
# commit only builds with nightlies close to the one it was tested with.
# Crubit's `latest` tag points at the newest commit whose CI passed, and that CI
# uses the nightly that was current when it ran. So this picks the commit
# behind `latest` and the nightly from the day its CI run passed.
#
# Set GH_TOKEN to avoid GitHub API rate limits.

set -euo pipefail

crubit_sha="$(git ls-remote https://github.com/google/crubit.git refs/tags/latest | cut -f1)"
if [[ -z "${crubit_sha}" ]]; then
  echo "Could not resolve Crubit's latest tag." >&2
  exit 1
fi

auth=()
if [[ -n "${GH_TOKEN:-}" ]]; then
  auth=(-H "Authorization: Bearer ${GH_TOKEN}")
fi
ci_run_date="$(curl -fsSL --retry 3 ${auth[@]+"${auth[@]}"} \
  "https://api.github.com/repos/google/crubit/actions/workflows/rust.yml/runs?head_sha=${crubit_sha}&status=success&per_page=1" |
  jq -r '.workflow_runs[0].run_started_at // empty' | cut -c1-10)"
if [[ -z "${ci_run_date}" ]]; then
  echo "No passing Crubit CI run found for ${crubit_sha}." >&2
  exit 1
fi

echo "Crubit ${crubit_sha} passed CI on ${ci_run_date}." >&2
echo "crubit_sha=${crubit_sha}"
echo "toolchain=nightly-${ci_run_date}"
