import ws from 'k6/ws';
import { check } from 'k6';

export let options = {
    scenarios: {
        socket_flow_test: {
            executor: 'constant-vus',
            exec: 'testSocketFlow',
            vus: 50, // Number of virtual users
            duration: '30s',
        },
    },
};

export function testSocketFlow() {
    const url = 'wss://app.merigafy.com/socket-flow';
    const params = { tags: { name: 'SocketFlow' } };

    const res = ws.connect(url, params, function (socket) {
        socket.on('open', function () {
            console.log('Connected to SocketFlow');
            socket.send('Hello WebSocket');
        });

        socket.on('message', function (message) {
            console.log(`Message from SocketFlow: ${message}`);
            socket.close();
        });

        socket.on('close', function () {
            console.log('Disconnected from SocketFlow');
        });

        socket.on('error', function (error) {
            console.log(`Error with SocketFlow: ${error}`);
        });
    });

    check(res, { 'status is 101': (r) => r && r.status === 101 });
}