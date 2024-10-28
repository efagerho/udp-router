use aya::{
    programs::{Xdp, XdpFlags},
    Ebpf,
};
use aya_log::EbpfLogger;
use log::warn;
use tokio::sync::mpsc;

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
    total_client_to_server_packets: PerCpuArray<MapData, u64>,
    total_server_to_client_packets: PerCpuArray<MapData, u64>,
}
struct ConfigMaps {
    local_net_and_mask: Array<MapData, u64>,
    backend_net_and_mask: Array<MapData, u64>,
}

#[derive(Clone, Debug)]
pub struct RouterStatistics {
    pub total_packets: u64,
    pub total_client_to_server_packets: u64,
    pub total_server_to_client_packets: u64,
}

//
// BPF Actor
//

#[derive(Clone, Debug)]
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
}

struct BpfActor {
    receiver: mpsc::Receiver<BpfActorMessage>,
}

impl BpfActor {
    fn new(receiver: mpsc::Receiver<BpfActorMessage>) -> Self {
        Self { receiver }
    }

    fn handle_message(&mut self, msg: BpfActorMessage) {}
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
        total_client_to_server_packets: PerCpuArray::try_from(bpf.take_map("TOTAL_CLIENT_TO_SERVER_PACKETS").unwrap()).unwrap(),
        total_server_to_client_packets: PerCpuArray::try_from(bpf.take_map("TOTAL_SERVER_TO_CLIENT_PACKETS").unwrap()).unwrap(),
    };

    let mut actor = BpfActor::new(receiver);
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg);
    }
}
