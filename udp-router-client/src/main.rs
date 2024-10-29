use clap::Parser;
use hdrhistogram::Histogram;
use std::error::Error;
use std::net::Ipv4Addr;
use std::str::FromStr;
use std::time::Instant;
use tokio::net::UdpSocket;

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
    let direct_socket = UdpSocket::bind("0.0.0.0:0").await?;
    let proxy_socket = UdpSocket::bind("0.0.0.0:0").await?;

    // Define the message to be sent
    let server_ip: u32 = Ipv4Addr::from_str(&opt.server).unwrap().into();
    let payload = server_ip.to_be_bytes();

    let mut buffer = [0_u8; 1024];

    println!("Measuring direct ping to server");
    let mut hist = Histogram::<u64>::new_with_bounds(1, 100 * 1000, 2).unwrap();

    for _ in 0..10000 {
        let start = Instant::now();
        let _ = direct_socket
            .send_to(&payload, format!("{}:8888", opt.server))
            .await?;
        let _ = direct_socket.recv_from(&mut buffer).await?;
        let duration = start.elapsed();
        let _ = hist.record(duration.as_micros() as u64);
    }

    println!("p50 direct latency: {}", hist.value_at_quantile(0.50));
    println!("p99 direct latency: {}", hist.value_at_quantile(0.99));
    println!("p99.9 direct latency: {}", hist.value_at_quantile(0.999));

    println!("Measuring ping through proxy");
    let mut hist = Histogram::<u64>::new_with_bounds(1, 100 * 1000, 2).unwrap();

    for _ in 0..10000 {
        let start = Instant::now();
        let _ = proxy_socket
            .send_to(&payload, format!("{}:8888", opt.proxy))
            .await?;
        let _ = proxy_socket.recv_from(&mut buffer).await?;
        let duration = start.elapsed();
        let _ = hist.record(duration.as_micros() as u64);
    }

    println!(
        "p50 latency through proxy: {}",
        hist.value_at_quantile(0.50)
    );
    println!(
        "p99 latency through proxy: {}",
        hist.value_at_quantile(0.99)
    );
    println!(
        "p99.9 latency through proxy: {}",
        hist.value_at_quantile(0.999)
    );

    Ok(())
}
