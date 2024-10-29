use tokio::net::UdpSocket;
use std::error::Error;
use std::net::Ipv4Addr;
use std::str::FromStr;
use clap::Parser;

#[derive(Clone, Debug, Parser)]
struct Opt {
    /// IP address of UDP router
    #[clap(long)]
    proxy: String,
    /// IP address of server to ping
    #[clap(long)]
    server: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opt = Opt::parse();

    // Bind to any available local port
    let socket = UdpSocket::bind("0.0.0.0:0").await?;

    // Define the message to be sent
    let server_ip: u32 = Ipv4Addr::from_str(&opt.server).unwrap().into();
    let payload = server_ip.to_be_bytes();

    println!("payload is: {:?}", payload);

    // Send the UDP packet
    let bytes_sent = socket.send_to(&payload, format!("{}:8888", opt.proxy)).await?;
    println!("Sent packet to {}", opt.proxy);

    // Wait for response
    let mut buffer = [0_u8; 1024];
    let (bytes_received, src_addr) = socket.recv_from(&mut buffer).await?;

    println!("Received response");
    Ok(())
}
