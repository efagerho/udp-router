# UDP Router

Implements a stateless UDP router in an eBPF XDP hook. Requires a custom protocol,
which encodes in the packet payload the backend server address. The router works
as follows:

1. Router reads backend server's address from UDP packet payload
1. Router replaces the backend server's IP address with the client's IP address
1. Backend server includes client's IP address in response
1. Router reads the client's IP address from response and routes back to client
