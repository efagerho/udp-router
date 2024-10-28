use aya::{
    programs::{Xdp, XdpFlags},
    Ebpf,
};
use aya_log::EbpfLogger;
use log::warn;
use tokio::sync::mpsc;

use crate::Opt;

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
}

#[derive(Clone, Debug)]
pub enum BpfActorMessage {}

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

    // TODO: Make attachment mode configurable
    program.attach(&opt.iface, XdpFlags::SKB_MODE).unwrap();

    let mut actor = BpfActor::new(receiver);
    while let Some(msg) = actor.receiver.recv().await {
        actor.handle_message(msg);
    }
}
