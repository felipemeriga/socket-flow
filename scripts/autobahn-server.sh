#!/usr/bin/env bash
# Author michael <themichaeleden@gmail.com>
set -euo pipefail
set -x
SOURCE_DIR=$(readlink -f "${BASH_SOURCE[0]}")
SOURCE_DIR=$(dirname "$SOURCE_DIR")
cd "${SOURCE_DIR}/.."

function cleanup() {
    kill -9 ${WSSERVER_PID}
}
trap cleanup TERM EXIT

function test_diff() {
    jq -S 'del(."socket-flow" | .. | .duration?)' 'autobahn/expected-results-server.json' >> jq-server-expected.json
    jq -S 'del(."socket-flow" | .. | .duration?)' 'autobahn/server/index.json' >> jq-server-index.json

    # Compare files
    if diff -q "jq-server-expected.json" "jq-server-index.json" >/dev/null 2>&1
    then
        echo "Files are the same"
    else
        echo "Files are different"
        diff "jq-server-expected.json" "jq-server-index.json"
        exit 64
    fi
}

cargo run --release --example internal_server & WSSERVER_PID=$!
sleep 5

docker run --rm \
    -v "${PWD}/autobahn:/autobahn" \
    --network host \
    crossbario/autobahn-testsuite \
    wstest -m fuzzingclient -s 'autobahn/fuzzingclient.json'

test_diff
