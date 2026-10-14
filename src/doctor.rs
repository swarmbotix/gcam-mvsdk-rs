//! Host diagnostics behind `gcam-mvsdk-rs doctor`.
//!
//! GigE Vision discovery is a UDP broadcast answered by every camera on the
//! same L2 segment. It works from an unprivileged process; what it needs is
//! `net.ipv4.conf.all.rp_filter=0`, an open firewall for UDP 3956 and the
//! stream, and a host NIC with a static address in the camera subnet. The
//! checks here detect and explain those conditions; they change nothing.

use std::fmt;
use std::net::Ipv4Addr;

/// Severity of a check result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Ok,
    Info,
    Warn,
    Fail,
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Level::Ok => "ok",
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Fail => "FAIL",
        })
    }
}

/// One diagnostic finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub level: Level,
    pub title: String,
    pub detail: Vec<String>,
}

impl Check {
    pub fn new(level: Level, title: impl Into<String>) -> Self {
        Check { level, title: title.into(), detail: Vec::new() }
    }

    pub fn with(mut self, line: impl Into<String>) -> Self {
        self.detail.push(line.into());
        self
    }
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:>4}] {}", self.level, self.title)?;
        for d in &self.detail {
            write!(f, "\n       {d}")?;
        }
        Ok(())
    }
}

/// An IPv4 address on a host interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    pub name: String,
    pub ip: Ipv4Addr,
    pub prefix_len: u32,
    pub up: bool,
    pub loopback: bool,
    pub mtu: Option<u32>,
}

impl Interface {
    /// Subnet mask derived from the prefix length.
    pub fn mask(&self) -> Ipv4Addr {
        Ipv4Addr::from(prefix_to_mask(self.prefix_len))
    }

    /// Whether `addr` is in this interface's subnet.
    pub fn contains(&self, addr: Ipv4Addr) -> bool {
        let m = prefix_to_mask(self.prefix_len);
        u32::from(addr) & m == u32::from(self.ip) & m
    }
}

/// Converts a prefix length to a mask (`24` to `0xffffff00`).
pub fn prefix_to_mask(prefix_len: u32) -> u32 {
    match prefix_len {
        0 => 0,
        n if n >= 32 => u32::MAX,
        n => u32::MAX << (32 - n),
    }
}

/// IPv4 addresses of every host interface.
#[cfg(unix)]
pub fn interfaces() -> Vec<Interface> {
    use std::ffi::CStr;
    let mut out = Vec::new();
    // SAFETY: getifaddrs allocates a linked list that we walk read-only and
    // free once with freeifaddrs.
    unsafe {
        let mut ifap: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut ifap) != 0 {
            return out;
        }
        let mut cur = ifap;
        while !cur.is_null() {
            let ifa = &*cur;
            cur = ifa.ifa_next;
            if ifa.ifa_addr.is_null() || i32::from((*ifa.ifa_addr).sa_family) != libc::AF_INET {
                continue;
            }
            let sin = &*(ifa.ifa_addr as *const libc::sockaddr_in);
            let ip = Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
            let mask = if ifa.ifa_netmask.is_null() {
                0
            } else {
                let m = &*(ifa.ifa_netmask as *const libc::sockaddr_in);
                u32::from_be(m.sin_addr.s_addr)
            };
            let name = CStr::from_ptr(ifa.ifa_name).to_string_lossy().into_owned();
            let flags = ifa.ifa_flags as libc::c_int;
            out.push(Interface {
                mtu: read_mtu(&name),
                name,
                ip,
                prefix_len: mask.leading_ones(),
                up: flags & libc::IFF_UP != 0,
                loopback: flags & libc::IFF_LOOPBACK != 0,
            });
        }
        libc::freeifaddrs(ifap);
    }
    out
}

/// IPv4 addresses of every host interface (not implemented on this platform).
#[cfg(not(unix))]
pub fn interfaces() -> Vec<Interface> {
    Vec::new()
}

#[cfg(unix)]
fn read_mtu(name: &str) -> Option<u32> {
    std::fs::read_to_string(format!("/sys/class/net/{name}/mtu"))
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

fn read_sysctl(path: &str) -> Option<u32> {
    std::fs::read_to_string(path).ok().and_then(|s| s.trim().parse().ok())
}

/// Reverse-path filtering: the vendor requires it off on the camera NIC.
pub fn rp_filter_check(ifaces: &[Interface]) -> Check {
    let Some(all) = read_sysctl("/proc/sys/net/ipv4/conf/all/rp_filter") else {
        return Check::new(Level::Info, "rp_filter: not readable on this platform");
    };
    let mut names: Vec<&str> = ifaces.iter().filter(|i| !i.loopback).map(|i| i.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    let mut offenders = Vec::new();
    for n in names {
        let own = read_sysctl(&format!("/proc/sys/net/ipv4/conf/{n}/rp_filter")).unwrap_or(0);
        // The kernel applies the larger of the "all" and per-interface values.
        let effective = own.max(all);
        if effective != 0 {
            offenders.push(format!("{n}: effective rp_filter={effective} (all={all}, {n}={own})"));
        }
    }
    if offenders.is_empty() {
        return Check::new(Level::Ok, format!("rp_filter is 0 on every interface (all={all})"));
    }
    let mut c = Check::new(Level::Warn, "rp_filter is enabled; GigE camera replies may be dropped");
    for o in offenders {
        c = c.with(o);
    }
    c.with("Fix: sudo sysctl -w net.ipv4.conf.all.rp_filter=0 net.ipv4.conf.<nic>.rp_filter=0")
        .with("Persist it in /etc/sysctl.d/ to survive reboots.")
}

fn process_running(name: &str) -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    entries.flatten().any(|e| {
        std::fs::read_to_string(e.path().join("comm"))
            .map(|c| c.trim() == name)
            .unwrap_or(false)
    })
}

/// Firewall hint: GVCP uses UDP 3956 and the stream arrives on UDP as well.
pub fn firewall_check() -> Check {
    if cfg!(windows) {
        return Check::new(Level::Info, "firewall: allow the vendor tools and UDP 3956 in Windows Defender Firewall");
    }
    let ufw_enabled = std::fs::read_to_string("/etc/ufw/ufw.conf")
        .map(|s| s.lines().any(|l| l.trim().eq_ignore_ascii_case("ENABLED=yes")))
        .unwrap_or(false);
    let firewalld = process_running("firewalld");
    let mut active = Vec::new();
    if ufw_enabled {
        active.push("ufw");
    }
    if firewalld {
        active.push("firewalld");
    }
    if active.is_empty() {
        return Check::new(Level::Info, "no ufw or firewalld detected")
            .with("If an nftables/iptables policy drops inbound UDP, allow UDP 3956 and the stream from the camera subnet.");
    }
    Check::new(Level::Warn, format!("firewall active: {}", active.join(", ")))
        .with("GigE discovery uses UDP 3956 and images arrive as UDP; allow the camera subnet, for example:")
        .with("sudo ufw allow from 192.168.1.0/24    (adjust to your camera subnet)")
        .with("or put the camera NIC in a trusted zone (firewalld).")
}

/// Lists IPv4 host interfaces so the user can compare them with camera IPs.
pub fn nic_check(ifaces: &[Interface]) -> Check {
    let usable: Vec<&Interface> = ifaces.iter().filter(|i| !i.loopback).collect();
    if usable.is_empty() {
        if cfg!(windows) {
            return Check::new(Level::Info, "NIC listing not implemented on Windows; use `ipconfig`");
        }
        return Check::new(Level::Warn, "no non-loopback IPv4 interface found")
            .with("Give the camera NIC a static address in the camera subnet.");
    }
    let mut c = Check::new(Level::Info, format!("{} IPv4 interface(s)", usable.len()));
    for i in usable {
        let net = Ipv4Addr::from(u32::from(i.ip) & prefix_to_mask(i.prefix_len));
        c = c.with(format!(
            "{:<12} {}/{}  subnet {}/{}  {}{}",
            i.name,
            i.ip,
            i.prefix_len,
            net,
            i.prefix_len,
            if i.up { "up" } else { "down" },
            i.mtu.map(|m| format!("  mtu {m}")).unwrap_or_default()
        ));
    }
    c.with("Cameras must be in the same subnet as the NIC they are cabled to; use `setip` to move one.")
}

/// All host checks that do not need the SDK.
pub fn system_checks() -> Vec<Check> {
    let ifaces = interfaces();
    let mut out = Vec::new();
    if cfg!(target_os = "linux") {
        out.push(rp_filter_check(&ifaces));
    }
    out.push(firewall_check());
    out.push(nic_check(&ifaces));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks() {
        assert_eq!(prefix_to_mask(24), 0xffff_ff00);
        assert_eq!(prefix_to_mask(0), 0);
        assert_eq!(prefix_to_mask(32), u32::MAX);
        let i = Interface {
            name: "eth0".into(),
            ip: Ipv4Addr::new(192, 168, 1, 5),
            prefix_len: 24,
            up: true,
            loopback: false,
            mtu: Some(1500),
        };
        assert_eq!(i.mask(), Ipv4Addr::new(255, 255, 255, 0));
        assert!(i.contains(Ipv4Addr::new(192, 168, 1, 200)));
        assert!(!i.contains(Ipv4Addr::new(192, 168, 2, 200)));
    }
}
