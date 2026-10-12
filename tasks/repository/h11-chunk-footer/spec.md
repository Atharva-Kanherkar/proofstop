Repair h11/_readers.py in the provided h11 0.14.0 runtime package.

For every nonzero chunk, exactly CRLF must follow the chunk payload. Reject other footer bytes with RemoteProtocolError through h11.Connection.next_event(). Handle receive_data fragmentation correctly, including a CR and LF arriving separately. An incomplete valid footer should wait for more data; premature EOF is still a protocol error. Preserve valid requests, multiple chunks, chunk extensions, trailers, Content-Length handling, and the bytes left after one complete message. Do not change other package files or bypass the protocol parser.

Visible examples use the h11.Connection server API. Each input is an array of hex-encoded receive_data fragments; an empty fragment means EOF. The consumer drains events after each fragment, concatenates Data payloads, records whether EndOfMessage occurred, and reports trailing_data bytes after the final fragment. Expected error means the named exception propagates through next_event(). All execution happens without networking.

Return the complete replacement h11/_readers.py file in exactly one fenced python block. Keep its imports and unrelated behavior compatible with the supplied package.
