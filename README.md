# simple-websocket


NEXT STEPS

- Wait the client to receive close from server before closing the connection
- Client should not answer with a close upon receiving a close (add a if clause on close handling in read.rs)
- implement methods for close and send messages inside ws_connection, so the end user can't do mistakes with the exported channel,
for this, we could use a channel that will receive the close success from server, and the close method from ws_connection will be waiting
on that channel, so we don't close the connection before the server
- Get rid of error channel, use the read_tx that is sent to read.rs for delivering a Result, involve it on a Arc, and send clones 
to the spawns that verifies errors for the read.rs and write.rs inside handshake function

DONE

- Convert all channels to bounded channels from Tokio