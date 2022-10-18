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

for X in . lints/*; do
    pushd "$X"
    # smoelius: `--all-targets` can't be used here because the lint examples would fail.
    cargo clippy --workspace --tests -- \
        -D warnings \
        -W clippy::pedantic
    popd
done
