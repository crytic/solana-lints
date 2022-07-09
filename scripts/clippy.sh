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
    cargo clippy --manifest-path "$X"/Cargo.toml --workspace --tests -- \
        -D warnings \
        -W clippy::pedantic
done
