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

for X in $(find . -name Cargo.lock); do
    pushd "$(dirname "$X")"
    rm -f Cargo.lock
    cargo build --workspace --tests
    cargo upgrade --workspace --to-lockfile --offline
    popd
done
