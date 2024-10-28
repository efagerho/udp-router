use clap::Parser;
use udp_router_protobuf::management::router_service_client::RouterServiceClient;
use udp_router_protobuf::management::{
    GetStatsRequest, SetLocalNetAndMaskRequest, SetBackendNetAndMaskRequest
};
use std::net::Ipv4Addr;
use tonic::Request;

//
// Command line parsing
//

#[derive(Clone, Debug, Parser)]
struct Opt {
    /// Print filter statistics
    #[clap(long, default_value_t = false)]
    stats: bool,
    /// IP of host running filter controller
    #[clap(long, default_value = "127.0.0.1")]
    host: String,
    /// Port where filter controller is running
    #[clap(long, default_value_t = 8888)]
    port: u64,
    /// Set local network & mask (e.g. 10.0.0.0/8)
    #[clap(long, default_value = "")]
    set_local_net_and_mask: String,
    /// Set backend network & mask (e.g. 10.0.0.0/8)
    #[clap(long, default_value = "")]
    set_backend_net_and_mask: String,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let opt = Opt::parse();
    env_logger::init();

    let mut client: RouterServiceClient<tonic::transport::Channel> =
        RouterServiceClient::connect(format!("http://{}:{}", opt.host, opt.port)).await?;

    let (lnet, lmask) = parse_ip_mask(&opt.set_local_net_and_mask);
    let (bnet, bmask) = parse_ip_mask(&opt.set_backend_net_and_mask);

    if opt.stats {
        match client.get_stats(Request::new(GetStatsRequest {})).await {
            Ok(res) => {
                let res = res.into_inner();
                println!("total_packets: {}", res.total_packets);
                println!("client_to_server_packets: {}", res.client_to_server_packets);
                println!("server_to_client_packets: {}", res.server_to_client_packets);
                return Ok(());
            }
            Err(e) => {
                panic!("Error contacting XDP hook: {:?}", e);
            }
        }
    }

    if !opt.set_local_net_and_mask.is_empty() {
        match client.set_local_net_and_mask(Request::new(SetLocalNetAndMaskRequest { net: lnet, mask: lmask })).await {
            Ok(_) => (),
            Err(e) => {
                panic!("Error contacting XDP hook: {:?}", e);
            }
        }
    }

    if !opt.set_backend_net_and_mask.is_empty() {
        match client.set_backend_net_and_mask(Request::new(SetBackendNetAndMaskRequest { net: bnet, mask: bmask })).await {
            Ok(_) => (),
            Err(e) => {
                panic!("Error contacting XDP hook: {:?}", e);
            }
        }
    }

    Ok(())
}

fn parse_ip_mask(s: &str) -> (u32, u32) {
    if s.is_empty() {
        return (0, 0xffffffff);
    }

    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() != 2 {
        panic!("Invalid IP/mask:  {}", s);
    }

    let net = parts[0].parse::<Ipv4Addr>().unwrap().to_bits();
    let mask = 0xffffffff_u32 << (32 - parts[1].parse::<u32>().unwrap());

    (net, mask)
}
