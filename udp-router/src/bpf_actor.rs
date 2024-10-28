use aya::{
    maps::{Array, MapData, PerCpuArray, PerCpuValues},
    programs::{Xdp, XdpFlags},
    Ebpf,
};
use aya_log::EbpfLogger;
use log::warn;
use tokio::sync::{mpsc, oneshot};

use crate::Opt;

//
// Public API
//

#[derive(Clone, Debug)]
pub struct BpfActorHandle {
    sender: mpsc::Sender<BpfActorMessage>,
}

impl BpfActorHandle {
    pub fn new(opt: &Opt) -> Self {
        let (sender, receiver) = mpsc::channel(8);
        tokio::spawn(run_actor(receiver, opt.clone()));
        Self { sender }
    }

    pub async fn get_router_stats(&self) -> RouterStatistics {
        let (send, recv) = oneshot::channel();
        let msg = BpfActorMessage::GetStats { respond_to: send };

        let _ = self.sender.send(msg).await;
        recv.await.expect("BPF actor has been killed")
    }

    pub async fn set_local_net_mask(&self, net: u32, mask: u32) {
        let msg = BpfActorMessage::SetLocalNetAndMask { net, mask };
        let _ = self.sender.send(msg).await;
    }

    pub async fn set_backend_net_mask(&self, net: u32, mask: u32) {
        let msg = BpfActorMessage::SetBackendNetAndMask { net, mask };
        let _ = self.sender.send(msg).await;
    }

    pub async fn set_gateway_mac_address(&self, mac: u64) {
        let msg = BpfActorMessage::SetGatewayMacAddress { mac };
        let _ = self.sender.send(msg).await;
    }
}

//
// Statistics
//

macro_rules! read_metric {
    ($var:expr) => {{
        let values: PerCpuValues<u64> = $var.get(&0, 0).expect("unable to read metric");
        let mut tmp = 0;
        for cpu_val in values.iter() {
            tmp += *cpu_val;
        }
        tmp
    }};
}

struct StatsMaps {
    total_packets: PerCpuArray<MapData, u64>,
    client_to_server_packets: PerCpuArray<MapData, u64>,
    server_to_client_packets: PerCpuArray<MapData, u64>,
}
struct ConfigMaps {
    local_net_and_mask: Array<MapData, u64>,
    backend_net_and_mask: Array<MapData, u64>,
    gateway_mac_address: Array<MapData, u64>,
}

#[derive(Clone, Debug)]
pub struct RouterStatistics {
    pub total_packets: u64,
    pub client_to_server_packets: u64,
    pub server_to_client_packets: u64,
}

//
// BPF Actor
//

pub enum BpfActorMessage {
    GetStats {
        respond_to: oneshot::Sender<RouterStatistics>,
    },
    SetLocalNetAndMask {
        net: u32,
        mask: u32,
    },
    SetBackendNetAndMask {
        net: u32,
        mask: u32,
    },
    SetGatewayMacAddress {
        mac: u64,
    },
}

struct BpfActor {
    receiver: mpsc::Receiver<BpfActorMessage>,
    stats: StatsMaps,
    configs: ConfigMaps
}

impl BpfActor {
    fn new(receiver: mpsc::Receiver<BpfActorMessage>, stats: StatsMaps, configs: ConfigMaps) -> Self {
        Self { receiver, stats, configs }
    }

    fn handle_message(&mut self, msg: BpfActorMessage) {
        match msg {
            BpfActorMessage::GetStats { respond_to } => {
                let _ = respond_to.send(self.get_stats());
            }
            BpfActorMessage::SetLocalNetAndMask { net, mask } => {
                self.set_local_net_mask(net, mask);
            }
            BpfActorMessage::SetBackendNetAndMask { net, mask } => {
                self.set_backend_net_mask(net, mask);
            }
            BpfActorMessage::SetGatewayMacAddress { mac } => {
                self.set_gateway_mac_address(mac);
            }
        }
    }


    fn get_stats(&self) -> RouterStatistics {
        println!("Requesting stats from eBPF hook");

        let total_packets = read_metric!(self.stats.total_packets);
        let client_to_server_packets = read_metric!(self.stats.client_to_server_packets);
        let server_to_client_packets = read_metric!(self.stats.server_to_client_packets);

        RouterStatistics {
            total_packets,
            client_to_server_packets,
            server_to_client_packets
        }
    }

    fn set_local_net_mask(&mut self, net: u32, mask: u32) {
        let net_and_mask = ((net as u64) << 32) | (mask as u64);

        write_map!(self.configs.local_net_and_mask, 0, net_and_mask);
        println!("Setting network to {:#04x} and mask to {:#04x} ", net, mask);
    }

    fn set_backend_net_mask(&mut self, net: u32, mask: u32) {
        let net_and_mask = ((net as u64) << 32) | (mask as u64);

        write_map!(self.configs.backend_net_and_mask, 0, net_and_mask);
        println!("Setting network to {:#04x} and mask to {:#04x} ", net, mask);
    }

    fn set_gateway_mac_address(&mut self, mac: u64) {
        write_map!(self.configs.gateway_mac_address, 0, mac);
        println!("Setting gateway MAC address to to {:#04x}", mac);
    }
}

async fn run_actor(receiver: mpsc::Receiver<BpfActorMessage>, opt: Opt) {
    println!("Loading XDP program from file: {}", opt.bpf_prog);
    let mut bpf = Ebpf::load_file(opt.bpf_prog).unwrap();

    if let Err(e) = EbpfLogger::init(&mut bpf) {
        warn!("Failed to initialize eBPF logger: {}", e);
    }

    let program: &mut Xdp = bpf.program_mut("udp_router").unwrap().try_into().unwrap();
    program.load().expect("Failed to load XDP program");

    if opt.force_skb_mode {
        program.attach(&opt.iface, XdpFlags::SKB_MODE).expect("Failed to attach program in SKB mode");
        println!("Attached XDP program in SKB mode.");
    } else if opt.force_hw_mode {
        program.attach(&opt.iface, XdpFlags::HW_MODE).expect("Failed to attach program in HW mode");
        println!("Attached XDP program in HW mode.");
    } else if opt.force_drv_mode {
        program.attach(&opt.iface, XdpFlags::DRV_MODE).expect("Failed to attach program in DRV mode");
        println!("Attached XDP program in DRV mode.");
    } else if program.attach(&opt.iface, XdpFlags::HW_MODE).is_ok() {
        println!("Attached XDP program in HW mode.");
    } else if program.attach(&opt.iface, XdpFlags::DRV_MODE).is_ok() {
        println!("Attached XDP program in driver mode.");
    } else if opt.allow_skb_mode && program.attach(&opt.iface, XdpFlags::SKB_MODE).is_ok() {
        println!("Attached XDP program in SKB mode.");
    } else {
        panic!("Failed to bind XDP program in HW or DRV mode. You might want to try --allow-skb-mode");
    }

    let stats = StatsMaps {
        total_packets: PerCpuArray::try_from(bpf.take_map("TOTAL_PACKETS").unwrap()).unwrap(),
        client_to_server_packets: PerCpuArray::try_from(bpf.take_map("TOTAL_CLIENT_TO_SERVER_PACKETS").unwrap()).unwrap(),
        server_to_client_packets: PerCpuArray::try_from(bpf.take_map("TOTAL_SERVER_TO_CLIENT_PACKETS").unwrap()).unwrap(),
    };


    let configs = ConfigMaps {
        local_net_and_mask: Array::try_from(bpf.take_map("LOCAL_NET_AND_MASK").unwrap()).unwrap(),
        backend_net_and_mask: Array::try_from(bpf.take_map("BACKEND_NET_AND_MASK").unwrap()).unwrap(),
        gateway_mac_address: Array::try_from(bpf.take_map("GATEWAY_MAC_ADDRESS").unwrap()).unwrap(),
    };


    let mut actor = BpfActor::new(receiver, stats, configs);
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg);
    }
}
