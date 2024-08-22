# Autobahn Tests

## Introduction

The **Autobahn**|Testsuite provides a fully automated test suite to verify client and server implementations of [The WebSocket Protocol](http://tools.ietf.org/html/rfc6455) for specification conformance and implementation robustness.

## Running tests on server

For running tests in the server implementation, you will need to run a 
client container from autobahn-testsuite, that will connect to the WS server, and
execute all the test cases, and add the output to a folder. Also, you will need to
provide the container, a configuration file, that states the server hostname and port, the test cases to be executed/ignored,
and another specs. The configuration file can be found at: [fuzzingclient.json](./autobahn/fuzzingclient.json).

In order to execute the tests, first execute [echo_server](./examples/echo_server), and on a separate tab, execute the 
docker container image(root of this repo):

```shell
docker run --rm \                   
    -v "${PWD}/autobahn:/autobahn" \
    --network host \
    crossbario/autobahn-testsuite \
    wstest -m fuzzingclient -s 'autobahn/fuzzingclient.json'
```

If you are executing this on MacOS, even running docker in network host mode, you need to set your hostname in [fuzzingclient.json](./autobahn/fuzzingclient.json), to 
`host.docker.internal`, so the docker container can access your local server.