use bpf_actor::BpfActorHandle;
use clap::Parser;
#[rustfmt::skip]
use log::debug;
use server::ManagementServer;

mod bpf_actor;
mod server;

#[derive(Clone, Debug, Parser)]
struct Opt {
    /// Interface to attach XDP program to
    #[clap(short, long, default_value = "eth0")]
    iface: String,
    /// Path to BPF program
    #[clap(long)]
    bpf_prog: String,
    /// Address to bind controller to
    #[clap(long, default_value = "127.0.0.1")]
    bind_address: String,
    /// Port to bind controller to
    #[clap(long, default_value_t = 8888)]
    port: u32,
    /// Force binding XDP program in SKB mode
    #[clap(long, default_value_t = false)]
    force_skb_mode: bool,
    /// Force binding XDP program in hardware mode
    #[clap(long, default_value_t = false)]
    force_hw_mode: bool,
    /// Force binding XDP program in driver mode
    #[clap(long, default_value_t = false)]
    force_drv_mode: bool,
    /// Fall-back to SKB mode if HW or DRV not available
    #[clap(long, default_value_t = false)]
    allow_skb_mode: bool,
}

#[tokio::main]
async fn main() {
    let opt = Opt::parse();
    env_logger::init();

    let rlim = libc::rlimit {
        rlim_cur: libc::RLIM_INFINITY,
        rlim_max: libc::RLIM_INFINITY,
    };
    let ret = unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlim) };
    if ret != 0 {
        debug!("Remove limit on locked memory failed, ret is: {}", ret);
    }

    let bpf_actor = BpfActorHandle::new(&opt);
    let server = ManagementServer::new(bpf_actor, &opt.bind_address, opt.port);

    server.start().await;
}
