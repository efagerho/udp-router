use anyhow::Context as _;
use bpf_actor::BpfActorHandle;
use clap::Parser;
#[rustfmt::skip]
use log::debug;
use tokio::signal;

mod bpf_actor;

#[derive(Clone, Debug, Parser)]
struct Opt {
    /// Interface to attach XDP program to
    #[clap(short, long, default_value = "eth0")]
    iface: String,
    /// Path to BPF program
    #[clap(long)]
    bpf_prog: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    let ctrl_c = signal::ctrl_c();
    println!("Waiting for Ctrl-C...");
    ctrl_c.await?;
    println!("Exiting...");

    Ok(())
}
