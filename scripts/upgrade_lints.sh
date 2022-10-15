#! /bin/bash

# set -x
set -euo pipefail

if [[ $# -ne 0 ]]; then
    echo "$0: expect no arguments" >&2
    exit 1
fi

SCRIPTS="$(dirname "$(realpath "$0")")"
WORKSPACE="$(realpath "$SCRIPTS"/..)"

cd "$WORKSPACE"

for X in lints/*; do
    cargo dylint upgrade "$X"
done

# smoelius: Ensure the workspace uses the same toolchain as the lints.
cp lints/missing_signer_check/rust-toolchain .
