---
name: camera-discovery
description: Troubleshoot HuaTeng cameras that gcam-mvsdk-rs does not find, explaining why `list` shows 0 cameras or misses one (privileges, rp_filter, firewall, subnet, cabling), with an unprivileged discovery probe. Use when cameras are missing or unreachable. For everyday listing use camera-list; to move or rename a camera use camera-setip or camera-rename.
---

# camera-discovery

Related skills: **camera-list** (everyday listing and reading the output), **camera-setip**
(moving a camera into the host NIC subnet), **camera-rename** (naming by serial number),
**sdk-setup** (SDK not found).

GigE Vision discovery is a UDP broadcast (port 3956) on each host NIC, answered by every camera
on the same L2 segment. Use the installed `gcam-mvsdk-rs`, or `cargo run -q --` in a checkout.

## 1. Look

```bash
gcam-mvsdk-rs doctor            # SDK, privileges, rp_filter, firewall, NICs, camera subnets
gcam-mvsdk-rs list              # names, models, serials, camera and host NIC IP/MAC
gcam-mvsdk-rs list --json       # for scripts; --no-open if another program is streaming
```

## 2. Interpret "No cameras found"

Work through `doctor` findings in this order:

1. **Privileges** (`neither root nor CAP_NET_RAW`): the SDK needs a raw socket. Either
   `sudo setcap cap_net_raw,cap_net_admin+ep "$(command -v gcam-mvsdk-rs)"` (redo after each
   reinstall; ignored on `nosuid` mounts) or run `sudo -E gcam-mvsdk-rs list`.
   For a binary under `target/`, prefer `sudo -E`; rebuilding drops capabilities.
2. **Is anything answering at all?** `python3 -I .claude/skills/camera-discovery/gvcp_probe.py`
   sends one standard discovery broadcast without privileges and prints every camera that
   replies (MAC, IP, model, serial, user name). Replies here but none in `list` means the SDK
   lacks privileges or is filtered; no replies means power, cabling, or a different segment.
3. **rp_filter** non-zero on the camera NIC: `sudo sysctl -w net.ipv4.conf.all.rp_filter=0 net.ipv4.conf.<nic>.rp_filter=0`.
4. **Firewall** (ufw/firewalld active): allow UDP from the camera subnet.
5. **Subnet**: `doctor` and `list` flag cameras whose IP is outside the host NIC subnet. The NIC
   needs a static address in the camera subnet (for example `192.168.1.x/24`).
6. Cameras behind a router are not discovered by broadcast; use `Sdk::enumerate_gige_at(&[ip])`.

Report what was observed (commands, counts, warnings). Do not claim cameras exist without a
`list` or probe result.

## 3. Move a camera that is on the wrong subnet

```bash
gcam-mvsdk-rs list                                   # note the camera MAC and the host NIC subnet
gcam-mvsdk-rs setip <MAC> <IP> <MASK> <GW>           # e.g. setip 00-1A-2B-3C-4D-5E 192.168.1.50 255.255.255.0 0.0.0.0
gcam-mvsdk-rs list                                   # confirm; some cameras need a power cycle
```

`setip` writes a persistent static address (GigE Vision FORCEIP) and works across subnets. Pick
an unused address in the subnet of the NIC the camera is cabled to. Ask the user before changing
the address of a production camera. The full procedure, including the alternative of changing the
host NIC, is in **camera-setip**.

## 4. Rename by serial number

```bash
gcam-mvsdk-rs rename <SN> <NAME>     # e.g. rename 056082422008 sb03
```

The name (DeviceUserID) is written to camera NVM, read back, and printed. At most 31 bytes, none
of `\ / : * ? " < > |`. Enumeration may show it only after the camera restarts. Publishers select
cameras by this name (`grab sb03`, `Sdk::find_device("sb03")`). Ask the user before renaming.
The full procedure with name rules is in **camera-rename**.

## 5. Status codes when opening a camera

`-45` (access denied), `32774` (0x8006, GigE Vision access denied) or `-13` (I/O error): another
process or host holds the camera. Find it (`pgrep -a gige_ht`, `docker ps`), stop it, wait a few
seconds for the heartbeat to expire, or power-cycle the camera.
