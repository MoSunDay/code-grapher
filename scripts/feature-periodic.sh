#!/bin/sh
# Periodic feature draft refresh: git_url -> repo_id, mirror SOURCE_DIR onto
# the mr/<FEATURE_NAME> branch, stop on NOOP. Never approves and never merges:
# stable state only enters via human review.
set -eu
if [ "$#" -lt 3 ] || [ "$#" -gt 4 ]; then
    echo "usage: $0 GIT_URL FEATURE_NAME SOURCE_DIR [FEATURE_ROOT]" >&2
    exit 64
fi
git_url=$1
feature_name=$2
source_dir=$3
feature_root=${4:-}
feature_bin=${CG_FEATURE_BIN:-cg-feature}
set -- "$feature_bin"
if [ "$feature_root" != "" ]; then
    set -- "$@" "--feature-root=$feature_root"
fi
draft_output=$("$@" draft "--expected-feature-name=$feature_name" -- "$git_url" "$feature_name" "$source_dir")
printf '%s\n' "$draft_output"
if [ "$draft_output" = "NOOP" ]; then
    exit 0
fi
exit 0
