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

for Y in . lints/*/ui/*; do
    if ! grep -m 1 "^Only in ../../../../$(dirname "$Y"): $(basename "$Y")$" crate/diffs/* >/dev/null; then
        continue
    fi
    X="$(echo "$Y" | sed -n 's,^\(.*/\(insecure\|recommended\|secure\)\)-[^/]*$,\1,;T;p')"
    if [[ -z "$X" ]]; then
        continue
    fi
    echo
    echo '# '"$Y"
    diff -r -x '*.stderr' "$X" "$Y" || true
done | tail -n+2
