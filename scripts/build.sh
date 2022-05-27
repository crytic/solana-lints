#! /bin/bash
# Builds all of the libraries in `../lints`.

# set -x
set -euo pipefail

if [[ $# -ne 0 ]]; then
    echo "$0: expect no arguments" >&2
    exit 1
fi

cd "$(dirname "$0")"/../lints

DYLINT_LIBRARY_PATH=

for LIBRARY in *; do
    if [[ ! -d "$LIBRARY" || "$LIBRARY" = src ]]; then
        continue
    fi

    pushd "$LIBRARY" >/dev/null
    cargo build

    DEBUG="$PWD/target/debug"
    if [[ -z "$DYLINT_LIBRARY_PATH" ]]; then
        DYLINT_LIBRARY_PATH="$DEBUG"
    else
        DYLINT_LIBRARY_PATH="$DYLINT_LIBRARY_PATH:$DEBUG"
    fi

    popd >/dev/null
done

echo '# If you want to run the lints directly from this repository, you can do so by setting DYLINT_LIBRARY_PATH as follows:'
echo export DYLINT_LIBRARY_PATH=\'"$DYLINT_LIBRARY_PATH"\'
