---
name: camera-setip
description: Move a HuaTeng GigE camera to a static IP address in the host NIC's subnet with gcam-mvsdk-rs setip (selected by MAC), or decide to change the host NIC instead. Use when list or doctor reports a camera outside the host NIC subnet, when a camera has a 169.254.x.x address, or when the user asks to set a camera's IP. Writes the camera's persistent network settings.
---

# camera-setip

## When to use

- `list` prints `WARNING : camera is not in the host NIC subnet`, or `list --json` shows
  `"same_subnet": false`.
- `doctor` reports `camera ... is NOT in the subnet of host NIC ...`.
- A new camera answers from a link-local address (169.254.x.x) or an address from another network.
- The user asks to give a camera a fixed address.

If the camera does not appear in `list` at all, use **camera-discovery** first: discovery needs
`rp_filter` 0 to see a camera on a foreign subnet.

## Prerequisites

- The camera appears in `gcam-mvsdk-rs list` (its MAC is on the `Camera` line).
- GigE: `CAP_NET_RAW` or `sudo -E`.

## Steps

1. Read the current state:
   ```bash
   gcam-mvsdk-rs list       # Camera: IP/Mask/GW/MAC; Host NIC: IP/Mask of the card it is cabled to
   gcam-mvsdk-rs doctor     # IPv4 interfaces with their subnets
   ```
2. Choose a free address in the host NIC's subnet. Example: host NIC `192.168.1.74/24`, so
   pick from `192.168.1.x`, avoiding the NIC's own address, other cameras, and any DHCP pool.
   Check it is unused: `ping -c 2 192.168.1.50` should get no reply.
3. Confirm with the user: the camera MAC, the new IP, mask and gateway.
4. Write the configuration (`0.0.0.0` means no gateway, the usual case on a camera network):
   ```bash
   gcam-mvsdk-rs setip 00-1A-2B-3C-4D-5E 192.168.1.50 255.255.255.0 0.0.0.0
   ```
5. Verify:
   ```bash
   gcam-mvsdk-rs list       # Camera line shows the new IP, no WARNING
   ```
   Some cameras apply the address only after a power cycle.

**Alternative: change the host NIC instead** (when the camera's address must stay, or several
cameras already share a subnet). Ask the user first; this changes the host's network. A temporary
address for testing: `sudo ip addr add 192.168.1.10/24 dev enp2s0` (gone after reboot). For a
permanent change use the distribution's network configuration (NetworkManager `nmcli`, netplan).

## Reading the output

```text
SN 056090423003: current IP 169.254.12.7  Mask 255.255.0.0  GW 0.0.0.0
SN 056090423003: set IP 192.168.1.50  Mask 255.255.255.0  GW 0.0.0.0 (persistent)
Run `gcam-mvsdk-rs list` to confirm; some cameras apply the address after a restart.
```

- `note: 192.168.1.50 is outside the subnet of the host NIC that found the camera` means the
  camera will not be reachable from that card afterwards; stop and pick another address unless
  the user intends to move the camera to another network.
- `note: gateway ... is outside ...` means the gateway does not fit the mask; use `0.0.0.0` if
  there is no router.

**Persistent flag.** The CLI always writes a persistent static address (`bPersistent` TRUE): it
survives power cycles. The library call `Sdk::gige_set_ip(&dev, ip, mask, gw, false)` instead
switches the camera to automatic addressing and ignores the address arguments.

## If it fails

| Output | Cause and fix |
|---|---|
| `invalid argument: "<MAC>" is not a MAC address` | Use `00-1A-2B-3C-4D-5E` or `00:1a:2b:3c:4d:5e` form, copied from `list`. |
| `invalid argument: <mask> is not a valid subnet mask` | The mask must be contiguous ones, for example 255.255.255.0. |
| `invalid argument: <ip> cannot be assigned to a camera` | 0.0.0.0, broadcast, multicast and loopback addresses are refused. |
| `no camera matches "<MAC>"; available: ...` | Wrong MAC, or a USB camera (USB cameras have no IP). |
| SDK status error | Another program holds the camera, or the camera rejected the setting; see camera-discovery. |

## Safety

- Confirm the MAC and the complete address with the user before running `setip`.
- A wrong address makes the camera unreachable for streaming until `setip` is run again by MAC
  (discovery still finds it when `rp_filter` is 0). Write the old address down first.
- Never run `setip` on a camera the user did not identify, and never change the host NIC without
  asking.
