#![no_std]
#![no_main]

use core::mem;

use aya_ebpf::{
    bindings::xdp_action::{self, XDP_PASS},
    macros::{map, xdp},
    maps::{Array, PerCpuArray},
    programs::XdpContext,
};
use aya_log_ebpf::info;
use network_types::{
    eth::{EthHdr, EtherType},
    ip::{IpProto, Ipv4Hdr},
    udp::UdpHdr,
};

// Macro for reading map constants
macro_rules! read {
    ($var:expr, $index:expr) => {{
        unsafe {
            match $var.get($index) {
                Some(value) => *value,
                None => 0,
            }
        }
    }};
}

//
// Configuration
//
// Assumptions: Clients/Routers/Servers on different subnets.
//
// XDP does not have access to ARP tables, so our userland controller must provide
// the XDP program with all relevant MAC addresses. We therefore always communicate
// with backend servers through the default GW by putting the MAC address of the GW
// as the destination MAC address on all forwarded packets.
//
// As we must let through traffic from the local subnets (due to EC2 health checks),
// we pass through all traffic from the local subnet. Due to this, clients and servers
// must be on different subnets as the router. As the routers must distinguish
// between client and server packets, we also assume clients and servers are on
// distinct subnets.
//
// Note that backend & local network can overlap, i.e. backend can be whole VPC. The
// local network takes precedence.

// Any packets from this network are passed through XDP filter
#[map]
static mut LOCAL_NET_AND_MASK: Array<u64> = Array::with_max_entries(1, 0);

// Any packets from this network are assumed to be backend server
#[map]
static mut BACKEND_NET_AND_MASK: Array<u64> = Array::with_max_entries(1, 0);

// MAC address of default gateway of router.
#[map]
static mut GATEWAY_MAC_ADDRESS: Array<u64> = Array::with_max_entries(1, 0);

//
// Counters
//

#[map]
static mut TOTAL_PACKETS: PerCpuArray<u64> = PerCpuArray::with_max_entries(1, 0);
#[map]
static mut CLIENT_TO_SERVER_PACKETS: PerCpuArray<u64> = PerCpuArray::with_max_entries(1, 0);
#[map]
static mut SERVER_TO_CLIENT_PACKETS: PerCpuArray<u64> = PerCpuArray::with_max_entries(1, 0);

//
// Router implementation
//

#[xdp]
pub fn udp_router(ctx: XdpContext) -> u32 {
    match try_udp_router(ctx) {
        Ok(ret) => ret,
        Err(_) => xdp_action::XDP_ABORTED,
    }
}

fn try_udp_router(ctx: XdpContext) -> Result<u32, ()> {
    let ethhdr: *mut EthHdr = ptr_at_mut(&ctx, 0)?;
    match unsafe { (*ethhdr).ether_type } {
        EtherType::Ipv4 => {}
        _ => return Ok(XDP_PASS),
    }
    let ipv4hdr: *mut Ipv4Hdr = ptr_at_mut(&ctx, EthHdr::LEN)?;

    // We only care about UDP packets
    if unsafe { (*ipv4hdr).proto != IpProto::Udp } {
        return Ok(XDP_PASS);
    }

    let source_ip = u32::from_be(unsafe { (*ipv4hdr).src_addr });

    if is_link_local_ip(source_ip) || is_from_local_network(source_ip) {
        return Ok(XDP_PASS);
    }

    try_forward_packet(&ctx)
}

fn is_link_local_ip(ip: u32) -> bool {
    let link_local = (169 << 24) + (254 << 16);
    let link_local_mask = 0xffff0000;

    (ip & link_local_mask) == link_local
}

fn is_from_local_network(ip: u32) -> bool {
    let local_and_mask = read!(LOCAL_NET_AND_MASK, 0);
    let local = (local_and_mask >> 32) as u32;
    let local_mask = (local_and_mask & 0xffffffff) as u32;

    (ip & local_mask) == local
}

fn is_from_backend_server(ip: u32) -> bool {
    let net_and_mask = read!(BACKEND_NET_AND_MASK, 0);
    let net = (net_and_mask >> 32) as u32;
    let mask = (net_and_mask & 0xffffffff) as u32;

    (ip & mask) == net
}

fn try_forward_packet(ctx: &XdpContext) -> Result<u32, ()> {
    let ethhdr: *mut EthHdr = ptr_at_mut(ctx, 0)?;
    let ipv4hdr: *mut Ipv4Hdr = ptr_at_mut(ctx, EthHdr::LEN)?;
    let udphdr: *mut UdpHdr = ptr_at_mut(ctx, EthHdr::LEN + Ipv4Hdr::LEN)?;

    //
    // Step 1: Parse IP addresses from packet + payload
    //

    // Get IP addresses for forwarded packet
    let router_ip_be = unsafe { (*ipv4hdr).dst_addr };
    let source_ip_be = unsafe { (*ipv4hdr).src_addr };
    let payload: *mut u32 = ptr_at_mut(&ctx, EthHdr::LEN + Ipv4Hdr::LEN + UdpHdr::LEN)?;
    let target_ip_be = unsafe { *payload };

    //
    // Step 2: Rewrite source and destination IP of forwarded packet
    //

    // Keep track of changes to the data affecting the UDP checksum
    let mut udp_csum_ne = unsafe { u16::from_be((*udphdr).check) };

    unsafe {
        let old = u32::from_be((*ipv4hdr).dst_addr).to_be_bytes();
        let old_upper = u16::from_be_bytes([old[0], old[1]]);
        let old_lower = u16::from_be_bytes([old[2], old[3]]);
        let new = u32::from_be(target_ip_be).to_be_bytes();
        let new_upper = u16::from_be_bytes([new[0], new[1]]);
        let new_lower = u16::from_be_bytes([new[2], new[3]]);

        (*ipv4hdr).dst_addr = target_ip_be;

        // Patch UDP checksum for these changes
        udp_csum_ne = update_udp_checksum(udp_csum_ne, old_upper, new_upper);
        udp_csum_ne = update_udp_checksum(udp_csum_ne, old_lower, new_lower);
    }
    unsafe {
        let old = u32::from_be((*ipv4hdr).src_addr).to_be_bytes();
        let old_upper = u16::from_be_bytes([old[0], old[1]]);
        let old_lower = u16::from_be_bytes([old[2], old[3]]);
        let new = u32::from_be(router_ip_be).to_be_bytes();
        let new_upper = u16::from_be_bytes([new[0], new[1]]);
        let new_lower = u16::from_be_bytes([new[2], new[3]]);

        (*ipv4hdr).src_addr = router_ip_be;

        // Patch UDP checksum for these changes
        udp_csum_ne = update_udp_checksum(udp_csum_ne, old_upper, new_upper);
        udp_csum_ne = update_udp_checksum(udp_csum_ne, old_lower, new_lower);
    }
    unsafe {
        let old = u32::from_be(target_ip_be).to_be_bytes();
        let old_upper = u16::from_be_bytes([old[0], old[1]]);
        let old_lower = u16::from_be_bytes([old[2], old[3]]);
        let new = u32::from_be(source_ip_be).to_be_bytes();
        let new_upper = u16::from_be_bytes([new[0], new[1]]);
        let new_lower = u16::from_be_bytes([new[2], new[3]]);

        *payload = source_ip_be;

        // Patch UDP checksum for these changes
        udp_csum_ne = update_udp_checksum(udp_csum_ne, old_upper, new_upper);
        udp_csum_ne = update_udp_checksum(udp_csum_ne, old_lower, new_lower);
    }

    //
    // Step 3: Rewrite source and destination MAC address of forwarded packet
    //

    unsafe {
        (*ethhdr).src_addr = (*ethhdr).dst_addr;
        (*ethhdr).dst_addr = get_gateway_mac_address();
    }

    //
    // Step 4: Fix checksums
    //

    unsafe {
        (*ipv4hdr).check = calculate_ip_checksum(&*(ipv4hdr.cast()));
        (*udphdr).check = u16::to_be(udp_csum_ne);
    }

    Ok(xdp_action::XDP_TX)
}

#[inline(always)]
fn get_gateway_mac_address() -> [u8; 6] {
    let mac = read!(GATEWAY_MAC_ADDRESS, 0).to_be_bytes();
    [mac[2], mac[3], mac[4], mac[5], mac[6], mac[7]]
}

#[inline(always)]
fn calculate_ip_checksum(ipv4hdr: &[u8; 20]) -> u16 {
    let mut checksum: u32 = 0;
    for i in 0..10 {
        checksum += u16::from_be_bytes([ipv4hdr[2 * i], ipv4hdr[2 * i + 1]]) as u32
    }
    // subtract existing checksum from header
    checksum -= u16::from_be_bytes([ipv4hdr[10], ipv4hdr[11]]) as u32;

    // clear any overflow bits
    checksum = (checksum & 0xffff) + (checksum >> 16);
    checksum = (checksum & 0xffff) + (checksum >> 16);

    u16::to_be(!(checksum as u16))
}

// Calculates updated UDP checksum when changing a word 'old' to 'new' in packet.
// Algorithm: https://www.rfc-editor.org/rfc/rfc1624
#[inline(always)]
fn update_udp_checksum(csum: u16, old: u16, new: u16) -> u16 {
    !((!csum).wrapping_sub(old).wrapping_add(new))
}

//
// Helpers
//

#[inline(always)]
fn ptr_at_mut<T>(ctx: &XdpContext, offset: usize) -> Result<*mut T, ()> {
    let start = ctx.data();
    let end = ctx.data_end();
    let len = mem::size_of::<T>();

    if start + offset + len > end {
        return Err(());
    }

    Ok((start + offset) as *mut T)
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
