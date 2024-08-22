# TODO - Items


## NEXT STEPS

- add compression to client and server
- implement server example using ws_connection internally for sending custom data
- unit tests
- integration tests
- test at scale
- SSL support

## DONE

- Wait the client to receive close from server before closing the connection
- Client should not answer with a close upon receiving a close (add a if clause on close handling in read.rs)
- Convert all channels to bounded channels from Tokio
- Get rid of error channel, use the read_tx that is sent to read.rs for delivering a Result, involve it on a Arc, and send clones
  to the spawns that verifies errors for the read.rs and write.rs inside handshake function
- remove unbounded channel comments
- handle case of big json payload, it's throwing errors for texts that have more than 8 lines, if you put the json less than 8 lines, even if it's bigger, it still works
- implement methods for close and send messages inside ws_connection, so the end user can't do mistakes with the exported channel,
  for this, we could use a channel that will receive the close success from server, and the close method from ws_connection will be waiting
  on that channel, so we don't close the connection before the server
- Lint the application
- implement and test Continue Opcodes for client and server
- create README.md for the repo (Documentation)
- autobahn tests for server
- autobahn tests for client