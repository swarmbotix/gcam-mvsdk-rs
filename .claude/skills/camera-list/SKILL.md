---
name: camera-list
description: List the HuaTeng cameras gcam-mvsdk-rs can see and explain the output (user-set name, model, serial, port, camera and host NIC IP/MAC), as text or JSON. Use for everyday "which cameras are there", "what is the serial/IP of sb01", or before selecting a camera for info, grab, rename or setip. If the list is empty, switch to camera-discovery.
---

# camera-list

## When to use

- The user asks which cameras are connected, or for a camera's serial number, address or MAC.
- Before `info`, `grab`, `rename` or `setip`, to pick the camera and check its state.
- In scripts that start one process per camera (`list --json`).

Use **camera-discovery** instead when `list` reports no cameras, and **camera-setip** when a
camera is marked as outside the host NIC subnet.

## Prerequisites

- The command is installed (`bash install.bash`; it lands in `~/.cargo/bin`). In a checkout,
  `cargo run -q -- list` works the same way.
- The SDK is found: `gcam-mvsdk-rs doctor` prints `SDK library found`.
- No privileges: run the command as a normal user.

## Steps

```bash
gcam-mvsdk-rs list              # text, one block per camera
gcam-mvsdk-rs list --no-open    # do not open the cameras (use while a publisher is streaming)
gcam-mvsdk-rs list --json       # machine-readable, for scripts
```

Then select a camera by its user-set name or serial number:

```bash
gcam-mvsdk-rs info sb00
gcam-mvsdk-rs grab 056090423003 -n 1 -o /tmp/check
```

## Reading the output

Text output, one block per camera:

| Line | Meaning |
|---|---|
| `User-set name` | Name stored in the camera (set with `rename`). `<empty>` if never named; `<could not open: ...>` if another program holds the camera. Omitted with `--no-open`. |
| `Product` | Internal product name (the same for every unit of a model). |
| `Serial number` | Factory serial number; unique and never changes. |
| `Port type` | Interface, for example a GigE or USB description. |
| `Camera` | GigE only: the camera's IP, mask, gateway and MAC. |
| `Host NIC` | GigE only: the address, mask, gateway and MAC of the network card that found the camera. |
| `WARNING : camera is not in the host NIC subnet; use setip` | The camera answered but cannot be streamed from; see camera-setip. |

`list --json` prints an array with `index`, `user_name`, `user_name_error`, `friendly_name`,
`product_name`, `product_series`, `serial`, `port_type`, `sensor_type`, `driver_version`,
`instance`, and `gige` (`camera_ip`, `camera_mask`, `camera_gateway`, `camera_mac`, `host_ip`,
`host_mask`, `host_gateway`, `host_mac`, `same_subnet`; `null` for USB cameras). With `--no-open`,
`user_name` is `null`.

A camera is on the wrong subnet when `gige.same_subnet` is `false`, or when the first three
octets of `Camera IP` and `Host NIC IP` differ under a 255.255.255.0 mask.

**Selecting a camera afterwards.** Commands that take `NAME_OR_SN` try, in order: the serial
number, the enumerated name (if exactly one camera has it), then each camera's user-set name (this
opens every camera briefly). If two cameras share a user-set name, select by serial number.

## If it fails

| Output | Next step |
|---|---|
| `No cameras found. ...` | camera-discovery (`rp_filter`, firewall, subnet, cabling). |
| `could not find the HuaTeng MVSDK library` | sdk-setup. |
| `<could not open: ...>` with status -13, 32774 or -45 | Another program holds the camera; use `list --no-open`, or stop that program. |
| `WARNING : camera is not in the host NIC subnet` | camera-setip. |

## Safety

`list` only reads. Without `--no-open` it opens each camera for a moment to read its name, which
fails (harmlessly) on a camera another program is streaming from; prefer `--no-open` on production
systems while publishers run.
