#! /bin/bash
# Updates `../README.md` and `../lints/*/README.md`.

# NOTE: Before commiting, review your changes to those READMEs!

# set -x
set -euo pipefail

if [[ $# -ne 0 ]]; then
    echo "$0: expect no arguments" >&2
    exit 1
fi

SCRIPTS="$(dirname "$(realpath "$0")")"
WORKSPACE="$(realpath "$SCRIPTS"/..)"

cd "$WORKSPACE"/lints

TMP="$(mktemp)"

LISTED=

IFS=
cat ../README.md |
while read X; do
    if [[ "$X" =~ ^\| ]]; then
        if [[ -z "$LISTED" ]]; then
            echo '| Library | Description |'
            echo '| - | - |'
            grep -H '^description = "[^"]*"$' */Cargo.toml |
            sed 's,^\([^/]*\)/Cargo.toml:description = "\([^"]*\)"$,| [`\1`](lints/\1) | \2 |,'
            LISTED=1
        fi
        continue
    fi
    echo "$X"
done |
cat > "$TMP"

mv "$TMP" ../README.md

prettier --write ../README.md

for LIBRARY in *; do
    pushd "$LIBRARY" >/dev/null

    (
        echo "# $LIBRARY"
        echo
        cat src/*.rs |
        sed -n '/^[a-z_:]*_lint! {$/,/^}$/p' |
        sed -n 's,^[[:space:]]*///\([[:space:]]\(.*\)\)\?$,\2,;T;p'
    ) > README.md

    # prettier --write README.md

    popd >/dev/null
done
