# UDP Router

Implements a stateless UDP router in an eBPF XDP hook. Requires a custom protocol,
which encodes in the packet payload the backend server address. The router works
as follows:

1. Router reads backend server's address from UDP packet payload
1. Router replaces the backend server's IP address with the client's IP address
1. Backend server includes client's IP address in response
1. Router reads the client's IP address from response and routes back to client

## Testing

To test create a `terraform.tfvars` file as follows in `terraform/`

```
aws_profile = "<INSERT YOUR AWS PROFILE>"
public_key = "<INSERT YOUR PUBLIC SSH KEY>"

router_instance_type = "c7gn.4xlarge"
client_instance_type = "c7gn.4xlarge"
server_instance_type = "c7gn.4xlarge"

num_client_instances = 1
num_server_instances = 1
```

and run `terraform apply`. This will setup the servers with and install all
necessary dependencies and create 3 types of EC2 instancies:

1. Clients for running `udp-router-client`
1. Router for running the eBPF filter
1. Servers for running the `udp-router-server`

Currently, the `user_data` scripts for the EC2 instances will not download the
`udp-router` repository source code, so you need to upload the source code
manually to the servers.

### Compiling the Tools

To compile the code on the client and server instances:

```
tar xvzf udp-router.tar.gz
cd udp-router
cargo build --release
```

It takes a long time to run the `user_data` script on the router instance, since
it compiles LLVM from source (needed for ARM). You need to check with `top`
when there are no compilation processes running, so you know you can proceed.
To compile the code on the router instances:

```
rustup toolchain install nightly --component rust-src
cargo install --no-default-features bpf-linker

tar xvzf udp-router.tar.gz
cd udp-router/udp-router-ebpf
cargo build --release
cd ..
cargo build --release
```

### Running Tests

On the server EC2 instances, run:

```
cd udp-router
target/release/udp-router-server
```

On the routers you first need to lookup the MAC address of the default GW on the
router subnet:

```
arp -n 10.0.2.1
```

Start the UDP router XDP hook:

```
cd udp-router
RUST_LOG=info target/release/udp-router --iface ens5 --bpf-prog target/bpfel-unknown-none/release/udp-router --allow-skb-mode
```

Next configure the UDP router:

```
cd udp-router
target/release/udp-router-tool --set-local-net-and-mask 10.0.2.0/24 --set-backend-net-and-mask 10.0.3.0/24 --set-gateway-mac-address <GW MAC>
```

Run the client:

```
cd udp-router
target/release/udp-router-client --proxy 10.0.2.10 --server 10.0.3.10
```

This will give output like:

```
Measuring direct ping to server
p50 direct latency: 141
p99 direct latency: 196
p99.9 direct latency: 240
Measuring ping through proxy
p50 latency through proxy: 171
p99 latency through proxy: 259
p99.9 latency through proxy: 305
```

which tells the latency between the client and server in microseconds when connecting
directly vs. through the UDP router.


To run performance tests, you should start additional routers and servers and bombard
the router with `iperf` to see how traffic amounts impact the latency and packet loss.
