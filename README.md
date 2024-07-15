# simple-websocket


TODO Items

- Implement continue Opcode case
- Implement all remaining Opcode cases
- Improve framing / Check refactoring options
- Validate if there are more Websockets protocol patterns to handle
- Add a channel to receive messages and a channel to write messages to the buf_writer,
the main function should return these channels, and should not block the process