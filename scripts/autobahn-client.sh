#!/usr/bin/env bash
# Author michael <themichaeleden@gmail.com>
set -euo pipefail
set -x
SOURCE_DIR=$(readlink -f "${BASH_SOURCE[0]}")
SOURCE_DIR=$(dirname "$SOURCE_DIR")
cd "${SOURCE_DIR}/.."

CONTAINER_NAME=fuzzingserver
function cleanup() {
    docker container stop "${CONTAINER_NAME}"
}
trap cleanup TERM EXIT

function test_diff() {
    jq -S 'del(."socket-flow" | .. | .duration?)' 'autobahn/expected-results-client.json' >> jq-client-expected.json
    jq -S 'del(."socket-flow" | .. | .duration?)' 'autobahn/client/index.json' >> jq-client-index.json

    # Compare files
    if diff -q "jq-client-expected.json" "jq-client-index.json" >/dev/null 2>&1
    then
        echo "Files are the same"
    else
        echo "Files are different"
        diff "jq-client-expected.json" "jq-client-index.json"
        exit 64
    fi
}

docker run -d --rm \
    -v "${PWD}/autobahn:/autobahn" \
    -p 9001:9001 \
    --init \
    --name "${CONTAINER_NAME}" \
    crossbario/autobahn-testsuite \
    wstest -m fuzzingserver -s 'autobahn/fuzzingserver.json'

sleep 5
cargo run --release --example autobahn_client
test_diff
