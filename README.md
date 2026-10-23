# gcam-mvsdk-rs

`gcam-mvsdk-rs` is a command-line tool and a Rust library for HuaTeng Vision (华腾威视) GigE and USB
industrial cameras on Linux. It finds the cameras on your network, renames them, sets their IP
address, reads and changes exposure, and grabs frames.

It does this by loading the vendor's own SDK library (`libMVSDK.so`) at run time. That library is
not in this repository, because HuaTeng does not permit redistribution. The installer downloads it
from HuaTeng's website, checks its SHA-256 against a value recorded here, and stores it in your home
directory. Nothing from the vendor is compiled into the tool.

## Run

The command-line tool, after `bash install.bash`:

```bash
gcam-mvsdk-rs list                                        # every camera; --json for scripts; --no-open while others stream
gcam-mvsdk-rs info sb00                                   # settings and capabilities of one camera
gcam-mvsdk-rs measure sb00                                # delivered fps and lost frames over 5 s, no window, no files
gcam-mvsdk-rs set sb00 --frame-speed 2 --exposure-us 5000 --save A  # change settings; --save A makes them the power-on defaults
gcam-mvsdk-rs view sb00                                   # live window; r records PNG images, c stops, q quits
gcam-mvsdk-rs grab sb00 -n 3 -o frames/ --mtu 1500        # save PNG frames without a window
gcam-mvsdk-rs rename <SN> sb00                            # give a camera a name
gcam-mvsdk-rs setip <MAC> 192.168.1.50 255.255.255.0 0.0.0.0
gcam-mvsdk-rs doctor                                      # SDK, rp_filter, firewall, NIC subnets
```

`view` exists when the command was built with `--features view` (OpenCV and libclang installed;
`install.bash` adds the feature when it finds them, see [Live view](#live-view)). A camera is
held by one process at a time: stop any program that streams from it before `view`, `measure`
or `grab`.

Everything the SDK keeps about the cameras (the parameter groups A to D, of which A is loaded on
every open, the per-camera data files and its log) goes to the SDK data directory:
`$GCAM_MVSDK_DATA_DIR`, else `<SDK home>/data`, or `--data-dir DIR` for one command. Point the
variable at a folder of your checkout (this repository ignores `data/`) to keep all camera
configuration with the project.

## Quick start

```bash
git clone https://github.com/swarmbotix/gcam-mvsdk-rs
cd gcam-mvsdk-rs
bash install.bash
gcam-mvsdk-rs list
```

Takes about three minutes on a normal connection: one to build, one to download the SDK, one for
Rust itself if it is not installed yet. The installer prints each step as it goes and can be run
again at any time; it skips what is already done.

Supported: Linux on x86_64, aarch64, armv7l and i686. Tested on Ubuntu 24.04 x86_64. Other
distributions work when a C compiler and `curl` are available. The installer refuses to run on
macOS and Windows.

## What install.bash does to your computer

Read this section if you want to know exactly what changes before you run the script. Everything
below is also printed by the script while it runs, and `bash install.bash --help` lists the options.

### Steps, in order

1. **Checks the platform.** Reads `/etc/os-release` and `uname -m`. Stops if the CPU architecture
   has no SDK build.
2. **Rust toolchain.** If `cargo` is already installed, nothing happens. If it is missing, the script
   runs the official `rustup` installer with the minimal profile. This is the one step that touches
   your shell configuration: rustup appends a line that adds `~/.cargo/bin` to `PATH` in
   `~/.profile` and `~/.bashrc` (or the file for your shell). The script itself never edits those
   files.
3. **C compiler and pkg-config.** Rust needs a C linker. If `cc` is missing on Debian or Ubuntu, the
   script offers `sudo apt-get install -y build-essential pkg-config`. Nothing else is installed
   with `apt`.
4. **Builds and installs the command.** `cargo install --path . --locked` compiles this repository
   and places one file, `~/.cargo/bin/gcam-mvsdk-rs`. Build dependencies are downloaded from
   crates.io into `~/.cargo/registry`, which is Cargo's normal cache.
5. **Downloads the vendor SDK.** Fetches
   `http://huatengvision.com/attachment/down/13/linuxSDK_V2.1.0.49%28202511141513%29.tar.gz`
   (58 MB, plain HTTP), compares the SHA-256 with the value in `sdk-manifest.toml`, and refuses the
   file if it differs. Extracts it to `~/.gcam-mvsdk/2.1.0.49/`. With `--from FILE` it uses an
   archive you downloaded yourself instead, verified the same way.
6. **USB udev rules (only with `--udev`).** Copies the vendor's two rule files to
   `/etc/udev/rules.d/` and reloads udev. Skip this unless you have USB cameras. GigE cameras need
   no privileges: discovery and streaming work from a normal user process.
7. **Verifies.** Runs `gcam-mvsdk-rs doctor` and `gcam-mvsdk-rs list`, then prints where everything
   went.

All `sudo` commands are collected and shown together before the first prompt. `--no-sudo` prints
them and never runs them. Without a terminal attached (for example in a Dockerfile), the script
answers "no" to every question unless `--yes` is given.

### Files written

| Path | What | Size |
|---|---|---|
| `~/.cargo/bin/gcam-mvsdk-rs` | The command, one executable | about 3 MB |
| `~/.cargo/registry/`, `~/.cargo/git/` | Cargo's dependency cache (standard for any Rust project) | varies |
| `~/.gcam-mvsdk/2.1.0.49/` | The vendor SDK: `lib/<arch>/libMVSDK.so`, `include/`, `tools/GeConfigCmd`, `udev/`, `manifest.json` | about 75 MB for all architectures |
| `~/.gcam-mvsdk/data/` | Files the SDK writes when a camera is opened (per-camera settings) | small, created on first use |
| `~/.rustup/`, `~/.cargo/` | Only if Rust was not installed: the toolchain (minimal profile) | several hundred MB |

`~/.gcam-mvsdk` is shared with the Python package `gcam-mvsdk-py`; installing both downloads the SDK
once.

### System-wide changes

Only these, each one only with your explicit `sudo` confirmation:

| Change | Why | Undo |
|---|---|---|
| `apt-get install build-essential pkg-config` (Debian/Ubuntu, only if no C compiler) | Rust needs a linker | `sudo apt-get remove build-essential pkg-config` |
| Copy `88-mvusb.rules`, `99-mvusb.rules` to `/etc/udev/rules.d/` (only with `--udev`) | USB camera access without root | `sudo rm /etc/udev/rules.d/88-mvusb.rules /etc/udev/rules.d/99-mvusb.rules` |

### What it never does

- Does not write to `/usr`, `/lib`, `/opt`, or `/etc` (except the udev rules above, on request).
- Does not run `ldconfig`, install a kernel module, start a service, or add a cron job.
- Does not change `sysctl` settings or the firewall. `doctor` prints those commands; you run them.
- Does not modify `~/.bashrc` or `~/.profile` itself. Only rustup does, and only when it installs Rust.
- Does not touch the system Python, Docker, or any other project on the machine.
- Does not need the repository after installation. You may delete the clone; the command does not
  reference it.

### Network access

Three destinations: `sh.rustup.rs` and `static.rust-lang.org` (only when installing Rust),
`crates.io` (build dependencies), and `huatengvision.com` (the SDK, plain HTTP, which is why the
checksum check is mandatory). No telemetry, no other hosts. For a machine without internet access
see [Air-gapped install](#air-gapped-install).

## After installation: how it works

**Finding the command.** Typing `gcam-mvsdk-rs` makes your shell search the directories in `PATH`
for a file with that name. It is in `~/.cargo/bin`. Being inside the repository directory does not
matter; the shell never looks there. If you just installed Rust, the `PATH` change is in your shell
profile but not yet in the open terminal: open a new one, or run `source ~/.cargo/env`.

**Finding the SDK.** The command then loads `libMVSDK.so`. It looks, in order, at `$GCAM_MVSDK_DIR`
(if set, nowhere else), `./vendor/<version>/` (developer checkouts), `~/.gcam-mvsdk/<version>/`,
then the system library path. `gcam-mvsdk-rs doctor` shows which one is in use.

**Finding cameras.** `list` sends a GigE Vision discovery broadcast on every network card and
waits for replies. This reaches only cameras on the same network segment as a card in your
machine. No privileges are needed; the command runs as a normal user. USB cameras are found
through the udev rules.

## Commands

| Command | What it does |
|---|---|
| `gcam-mvsdk-rs list` | Every camera: user-set name, model, serial number, port, camera and host NIC IP/MAC. |
| `gcam-mvsdk-rs list --json` | The same as JSON, for scripts that start one process per camera. |
| `gcam-mvsdk-rs rename <SN> <NAME>` | Store a user-set name in the camera with that serial number (survives power cycles). |
| `gcam-mvsdk-rs setip <MAC> <IP> <MASK> <GW>` | Give a GigE camera a static address; works even when it is on the wrong subnet. Use `0.0.0.0` for no gateway. |
| `gcam-mvsdk-rs info <NAME or SN>` | Exposure, gain, resolution, trigger, image settings and capabilities of one camera. |
| `gcam-mvsdk-rs grab <NAME or SN> -n 3 -o frames/` | Save frames as PNG. `--exposure-us 5000` sets exposure; `--mtu 1500` fits the packet size to the NIC. |
| `gcam-mvsdk-rs set <NAME or SN> --frame-rate 30 --exposure-us 5000 --save A` | Change the frame-rate cap (0 removes it), the frame-speed tier (`--frame-speed 2`, the sensor readout speed listed by `info`), exposure, or `--auto-exposure on` / `off`, and print old and new values; `--save` stores them in parameter group A, B, C or D, otherwise they stay in RAM. Group A is the one the camera loads when opened, so save there to make the change the new default; B to D hold alternative setups loaded with `load_parameter`. On models without in-camera storage the groups are files on this host, `<SDK home>/data/Configs/<model>-Group<N>.config`, shared by all cameras of that model. Without options it prints the current values. |
| `gcam-mvsdk-rs measure <NAME or SN> --seconds 5` | Stream without saving or displaying and print the measured frame rate (host arrival and camera time stamps), the raw link data rate and the SDK's total / captured / lost counters. `--frame-speed IDX` and `--exposure-us US` apply to that run only; `--mtu` as for `grab`. |
| `gcam-mvsdk-rs view <NAME or SN>` | Live image in an OpenCV window with an fps overlay; `r` starts a PNG recording, `c` stops it, `q` or Esc quits. Needs the `view` feature; see [Live view](#live-view). |
| `gcam-mvsdk-rs doctor` | Check the SDK, `rp_filter`, firewall, NIC subnets and camera addresses, and print the fix for each. |
| `gcam-mvsdk-rs setup` | Download and verify the SDK (`install.bash` runs this for you). |

`list --no-open` skips reading user-set names, which requires opening each camera briefly; use it
while another program is streaming. `gcam-mvsdk-rs <command> --help` lists every option.

## Name your cameras

A new camera has no name; its serial number is the only unique identifier. Give each camera a
short name once, and from then on select it by that name everywhere (`list`, `info`, `grab`, the
library, your own launch scripts). The name is written into the camera's non-volatile memory and
survives power cycles and moving the camera to another computer.

```bash
gcam-mvsdk-rs list                       # find the serial number of the camera you plugged in
gcam-mvsdk-rs rename 056090423003 sb00   # write the name into that camera
gcam-mvsdk-rs list                       # the "user-set name" column now shows sb00
gcam-mvsdk-rs info sb00                  # select by name from now on
```

Rules enforced by the camera and checked by the tool: not empty, at most 31 bytes, none of
`\ / : * ? " < > |` and no control characters. Beyond that the camera stores the name in GBK, so
other characters are accepted; stick to ASCII letters, digits, `-` and `_` anyway, because the name
becomes part of topic names, file names and launch scripts. Pick names that are unique on your
network; if two cameras carry the same name, selection by name is ambiguous and you must fall back
to the serial number. The camera must not be open in another program while you rename it; stop
any publisher or viewer first (status `-13` or `32774` means it is still held).

The same from Rust:

```rust
let cam = sdk.open(&sdk.find_device("056090423003")?)?;   // by serial, since it has no name yet
println!("current name: {:?}", cam.user_name()?);
cam.set_user_name("sb00")?;                                // persists in the camera
```

Renaming is one of about 80 settings the SDK exposes; see [What the SDK can change](#what-the-sdk-can-change)
for the full list with use cases and examples.

## Install the command without install.bash

If you already have Rust and want only the `gcam-mvsdk-rs` command, any of these puts it in
`~/.cargo/bin`:

```bash
# from a local clone
cargo install --path /path/to/gcam-mvsdk-rs --locked

# straight from GitHub, no clone needed
cargo install --git https://github.com/swarmbotix/gcam-mvsdk-rs --tag v0.1.0 --locked

# later, once published to crates.io
cargo install gcam-mvsdk-rs
```

Then fetch the SDK once:

```bash
gcam-mvsdk-rs setup
gcam-mvsdk-rs list
```

## Use from your own Cargo project

Add the dependency from inside your project directory. Pick one line:

```bash
# local clone (absolute path recommended; Cargo rebuilds it with your project)
cargo add gcam-mvsdk-rs --path /path/to/gcam-mvsdk-rs

# GitHub, pinned to a tag
cargo add gcam-mvsdk-rs --git https://github.com/swarmbotix/gcam-mvsdk-rs --tag v0.1.0

# later, once published to crates.io
cargo add gcam-mvsdk-rs

# library only, without the CLI and downloader dependencies (add to any of the above)
cargo add gcam-mvsdk-rs --path /path/to/gcam-mvsdk-rs --no-default-features
```

Which writes one of these into your `Cargo.toml`:

```toml
[dependencies]
gcam-mvsdk-rs = { path = "/path/to/gcam-mvsdk-rs" }
gcam-mvsdk-rs = { git = "https://github.com/swarmbotix/gcam-mvsdk-rs", tag = "v0.1.0" }
gcam-mvsdk-rs = "0.1"
```

Inside the `vision_workspace` checkout the relative form is
`gcam-mvsdk-rs = { path = "../../drivers/gcam-mvsdk-rs" }` from `camera_publishers/cam_gige_ht/`
and `{ path = "../drivers/gcam-mvsdk-rs" }` from `camera_ips/`.

Then in code (the crate name becomes `gcam_mvsdk_rs`):

```rust
use std::time::Duration;
use gcam_mvsdk_rs::{OutputFormat, Sdk};

fn main() -> gcam_mvsdk_rs::Result<()> {
    let sdk = Sdk::load()?; // finds the SDK at runtime
    for dev in sdk.enumerate()? {
        println!("{} SN {} {}", dev.friendly_name, dev.serial, dev.port_type);
    }
    let cam = sdk.open(&sdk.find_device("sb00")?)?; // user-set name or serial number
    cam.set_output_format(OutputFormat::Mono8)?;
    cam.set_auto_exposure(false)?;
    cam.set_exposure_us(5000.0)?;
    cam.play()?;
    let frame = cam.grab(Duration::from_millis(1000))?; // returned to the SDK when dropped
    println!("{}x{}, {} bytes", frame.width(), frame.height(), frame.as_slice().len());
    Ok(())
}
```

`frame.as_slice()` is the raw sensor buffer without a copy; `frame.process()` runs the vendor's
image pipeline into a new buffer.

One thing your program needs at run time, on every machine it runs on, exactly like the command:
the SDK installed, through `gcam-mvsdk-rs setup`, `bash install.bash`, or `GCAM_MVSDK_DIR` pointing
at an extracted copy. The library finds it with the same search order as the command. GigE
discovery and streaming need no privileges, so the binary runs as a normal user.

## What the SDK can change

[docs/CAPABILITIES.md](docs/CAPABILITIES.md) lists everything the vendor SDK can read and change on a
camera, in ten sections: identity and network, exposure and gain, frame rate and trigger, image
geometry, image processing, digital I/O and strobe, GigE transport, parameter storage, callbacks,
and the vendor GUI functions. Each entry gives a typical use case, the vendor function names, whether
`gcam-mvsdk-rs` has a safe method for it, and a Rust snippet. Every snippet is compiled from
`examples/capabilities.rs`, which can also run them against a camera:

```bash
cargo run --example capabilities -- sb00                    # list the snippets
cargo run --example capabilities -- sb00 gige_ip read_sn    # read-only ones
cargo run --example capabilities -- sb00 --write exposure   # ones that change settings
```

Snippets that would store their demonstration values in the camera (renaming it, changing its IP
address, overwriting a parameter group) are refused unless `--persistent` is added as well.

What a particular camera supports, and in what range, depends on the model:
`gcam-mvsdk-rs info <NAME or SN>` prints its capabilities.

## Live view

`gcam-mvsdk-rs view` shows one camera in an OpenCV window and records PNG images on request. It
is compiled in only with the `view` feature, because the `opencv` crate is a binding to the system
OpenCV library and generates it with libclang at build time; no Cargo-installable OpenCV exists.
`install.bash` adds the feature when it finds OpenCV and libclang. By hand, on Ubuntu:

```bash
sudo apt-get install -y libopencv-dev clang libclang-dev
cargo install --path . --locked --features view          # or: cargo run --release --features view -- view sb00
gcam-mvsdk-rs view sb00
# keys: r start recording, c stop recording, q or Esc quit (closing the window also quits)
```

`sb00` is a user-set name or serial number; omit it when exactly one camera is present. Options:
`--exposure-us 5000` (default; automatic exposure off), `--frame-speed IDX` (sensor readout tier
from `info`; default the fastest the camera lists, so the window shows the camera's full rate
whatever its stored defaults), `--mtu 1500` (packet size, as for `grab`) and `--config FILE`
(recording options). Nothing is persisted in the camera.

Pressing `r` records `rate_hz` images per second (default 5) for `duration_s` seconds (default 30),
or until `c` is pressed; the camera keeps streaming at its own rate in between. The images are
plain Mono8 PNG files, without the overlay, in a new folder per recording:
`<data_dir>/<YYYYMMDD_HHMMSS>_<camera>/0.png, 1.png, ...`, where `<camera>` is the user-set name
(else the serial) and `data_dir` defaults to the SDK data directory (see [Run](#run)). The three
options are read from the YAML file given with `--config`; `examples/record.yaml` is a sample.
The terminal prints the folder and the image count when a recording stops.

It needs a display: a desktop session with `DISPLAY`
set, or, in Docker, the X socket shared into the container and `xhost +local:` run on the host.
The `view` feature is off by default, so building the crate or the command never requires OpenCV.

On other distributions, install OpenCV 4 and libclang as described in the `opencv` crate's own
instructions: https://github.com/twistedfall/opencv-rust#getting-opencv

### Add OpenCV to your own project

```bash
cargo add opencv --features highgui,imgproc
```

Adding `--no-default-features` builds only these two modules, as the example does
(`opencv = { version = "0.101", default-features = false, features = ["highgui", "imgproc"] }`).
A frame then goes into a `Mat` without a copy. With `OutputFormat::Mono8` each pixel is one byte,
so the `Mat` is `CV_8UC1`:

```rust
use std::time::Duration;
use gcam_mvsdk_rs::{Camera, OutputFormat};
use opencv::{core::Mat, highgui};

fn show(cam: &Camera) -> Result<(), Box<dyn std::error::Error>> {
    cam.set_output_format(OutputFormat::Mono8)?;
    cam.play()?;
    let mut pixels = Vec::new(); // reused for every frame
    loop {
        let frame = cam.grab(Duration::from_secs(1))?;
        pixels.resize(frame.processed_len()?, 0);
        let info = frame.process_into(&mut pixels)?; // vendor image pipeline into our buffer
        drop(frame); // return the SDK buffer
        let (h, w) = (info.height as i32, info.width as i32);
        let mat = Mat::new_rows_cols_with_data(h, w, &pixels[..info.len])?; // CV_8UC1 view, no copy
        highgui::imshow("camera", &mat)?;
        if highgui::wait_key(1)? == 27 {
            return Ok(()); // Esc
        }
    }
}
```

## Network settings

GigE discovery and streaming run as a normal user: no root, no `sudo`, no file capability. What
they do depend on is the host's network configuration. Discovery needs three things, and streaming
at the camera's full frame rate needs a fourth. `gcam-mvsdk-rs doctor` checks the first three and
prints the command for your machine; the fourth is not checked by `doctor` and is explained under
[Known issues](#known-issues).

```bash
# 1. Reverse-path filtering off, now and after reboot (replace enp2s0 with the camera network card)
sudo sysctl -w net.ipv4.conf.all.rp_filter=0 net.ipv4.conf.enp2s0.rp_filter=0
echo 'net.ipv4.conf.all.rp_filter=0' | sudo tee /etc/sysctl.d/90-gige-camera.conf

# 2. Firewall: allow the camera subnet (ufw shown; adjust the subnet)
sudo ufw allow from 192.168.1.0/24

# 3. Host NIC and cameras in the same subnet: give the NIC a static address, or move a camera
gcam-mvsdk-rs setip 00-1A-2B-3C-4D-5E 192.168.1.50 255.255.255.0 0.0.0.0

# 4. Larger socket receive buffers, now and after reboot (needed for full frame rate; see Known issues)
sudo sysctl -w net.core.rmem_max=33554432 net.core.rmem_default=33554432
printf 'net.core.rmem_max=33554432\nnet.core.rmem_default=33554432\n' \
  | sudo tee -a /etc/sysctl.d/90-gige-camera.conf

# USB cameras instead: vendor udev rules
sudo cp ~/.gcam-mvsdk/2.1.0.49/udev/*.rules /etc/udev/rules.d/ && sudo udevadm control --reload-rules
```

The `sudo` above is for changing kernel settings and the firewall once per host; the camera tool
itself never needs it. Inside Docker, use `network_mode: host` so the discovery broadcast reaches
the camera network.

## Troubleshooting

| Symptom | Cause and fix |
|---|---|
| `gcam-mvsdk-rs: command not found` | `~/.cargo/bin` is not on `PATH` in this terminal. Run `source ~/.cargo/env`, or `export PATH="$HOME/.cargo/bin:$PATH"` and add it to `~/.bashrc`. Being in the repository directory does not help. |
| `list` finds 0 cameras | Check power and cabling, `rp_filter` (Ubuntu 24.04 default is 2, which drops the camera's reply), the firewall, and that a network card has an address in the camera subnet (`gcam-mvsdk-rs doctor` reports each). Discovery only reaches cameras on the same network segment; it needs no privileges, so `sudo` does not help. Details under [Known issue 1](#1-list-finds-no-cameras-and-the-camera-is-absent-from-arp--a). |
| `could not find the HuaTeng MVSDK library` | Run `bash install.bash` or `gcam-mvsdk-rs setup`, or set `GCAM_MVSDK_DIR`. The message lists every place that was checked. A wrong `GCAM_MVSDK_DIR` is reported, not ignored. |
| Status `-13`, `32774` (0x8006) or `-45` when opening a camera | The camera is busy: another program or another computer holds it (a running publisher, a container, the vendor GUI). Stop that program, wait a few seconds for the camera's heartbeat to expire, or power-cycle the camera. `list --no-open` still works meanwhile. |
| `Permission denied` | USB cameras: install the udev rules. Files under `~/.gcam-mvsdk` owned by root (from an earlier `sudo` run): `sudo chown -R "$USER" ~/.gcam-mvsdk`. |
| `grab` times out | The GigE packet size exceeds the network card's MTU: use `--mtu 1500` (or enable jumbo frames). Also check the firewall and `rp_filter`. |
| Frame rate stuck near 60 fps, no frames lost | `net.core.rmem_max` is at the Ubuntu default (about 200 KB) and the camera throttles itself after resend requests. Apply step 4 in [Network settings](#network-settings); details under [Known issue 2](#2-frame-rate-capped-near-60-fps-although-no-frames-are-lost). |
| Download fails, or the size or checksum does not match | The vendor may have moved or replaced the file. Download the Linux SDK from http://huatengvision.com/download/1/ and install it with `--from` (below). A checksum mismatch is never overridden. |

## Known issues

Two problems appear on nearly every fresh Ubuntu host. Neither is a bug in this package or in the
SDK; both are kernel defaults that are wrong for GigE Vision cameras. `install.bash` never changes
them (see [What it never does](#what-it-never-does)), so apply each fix once per host.

### 1. `list` finds no cameras, and the camera is absent from `arp -a`

**Symptom.** The camera is powered, the switch link LED is on, and the host NIC is up, yet
`gcam-mvsdk-rs list` reports no cameras. The camera's MAC address does not appear in `arp -a` either.
Inside Docker with `network_mode: host` the result is identical, so the container is not the cause.

**Why it happens.** A GigE Vision camera answers the discovery broadcast from whatever address it
currently holds. A fresh camera usually has a link-local address (169.254.x.x) or an address left
over from another network, so its reply arrives from a subnet the host has no route to. Ubuntu
24.04 ships with `net.ipv4.conf.*.rp_filter = 2` (loose reverse-path filtering), which silently
drops such packets inside the kernel, before any user-space program sees them. That is why the
camera is missing from `arp -a` as well. When `ufw` is enabled it additionally blocks the inbound
GigE Vision control port (UDP 3956), which stops discovery even with `rp_filter` at 0.

**Fix.** Steps 1 and 2 in [Network settings](#network-settings): set `rp_filter` to 0
for all interfaces and for the camera NIC, write the setting to `/etc/sysctl.d/90-gige-camera.conf` so it
survives a reboot, and allow inbound traffic from the camera in the firewall (or disable `ufw` on an
isolated lab host). If the camera still has an address outside the host subnet afterwards, move it
with `setip` (step 3).

**Verify.**

```bash
sysctl net.ipv4.conf.all.rp_filter net.ipv4.conf.enp2s0.rp_filter   # both must print 0
sudo ufw status                                                      # inactive, or an allow rule for the camera
gcam-mvsdk-rs list                                                    # the camera is listed
```

### 2. Frame rate capped near 60 fps although no frames are lost

**Symptom.** Streaming works and the lost-frame counter stays at zero, but the rate settles near
60 fps at full resolution (about 85 fps at 1280x720) on a camera rated for 91 fps. `info` shows that
the configured frame rate is not the limit.

| State | Throughput | Frame rate (1280x1024 mono) |
|---|---|---|
| Default `rmem_max` (about 200 KB) | 79.9 MB/s | 61 fps |
| `rmem_max` = 32 MB | 119.9 MB/s | 91.5 fps |

**Why it happens.** Ubuntu's default `net.core.rmem_max` is about 200 KB, while one 1280x1024
mono frame is 1.3 MB and arrives as a burst of packets at wire speed. The kernel socket buffer
overflows in the few milliseconds between the camera sending a frame and the SDK draining it,
packets are dropped, the GigE Vision stream issues resend requests, and the camera firmware reacts
by stepping down to a "safe" rate of roughly 78 MB/s. The camera is choosing to send more slowly,
which is why no frames are reported lost.

**Fix.** Step 4 in [Network settings](#network-settings): raise `net.core.rmem_max`
and `net.core.rmem_default` to 32 MB (33554432 bytes) and append them to
`/etc/sysctl.d/90-gige-camera.conf`. The SDK sizes its sockets when a camera is opened, so restart any program
that was already streaming. Inside Docker with `network_mode: host` the new values apply at once,
without a rebuild.

**Verify.**

```bash
sysctl net.core.rmem_max net.core.rmem_default   # both must print 33554432
gcam-mvsdk-rs grab <NAME or SN> -n 200           # then compare the wall-clock time against the rated frame rate
```

## Air-gapped install

On a connected machine, download
`http://huatengvision.com/attachment/down/13/linuxSDK_V2.1.0.49%28202511141513%29.tar.gz`
(57,978,497 bytes, SHA-256 `9624d193a908d94a9850400346cf1ab8095437a040e381b2fd39029b0183dd8a`),
and run `cargo fetch --locked` in the checkout so that the Rust dependencies are cached. The
target machine needs a Rust toolchain (1.88 or newer). Copy the checkout, `~/.cargo/registry` and
the archive to it, then:

```bash
bash install.bash --from /path/to/linuxSDK_V2.1.0.49\(202511141513\).tar.gz
```

The archive is accepted only if its SHA-256 matches a release listed in `sdk-manifest.toml`.

## Uninstall

```bash
bash install.bash --uninstall
```

Removes `~/.cargo/bin/gcam-mvsdk-rs`, then asks before deleting
`~/.gcam-mvsdk` (kept by default, because `gcam-mvsdk-py` uses the same files). To remove the rest
by hand:

```bash
sudo rm -f /etc/udev/rules.d/88-mvusb.rules /etc/udev/rules.d/99-mvusb.rules   # only if --udev was used
sudo rm -f /etc/sysctl.d/90-gige-camera.conf                                     # only if you added it
rustup self uninstall                                                             # only if install.bash installed Rust and you do not want it
```

After that the machine is as it was.

## Environment variables

| Variable | Effect |
|---|---|
| `GCAM_MVSDK_HOME` | Where `install.bash` and `setup` put the SDK (`<home>/<version>/`) and where the command looks for it. Default `~/.gcam-mvsdk`. Shared with `gcam-mvsdk-py`. |
| `GCAM_MVSDK_DIR` | One extracted SDK directory (the one containing `lib/`). When set, it is the only place searched. |
| `GCAM_MVSDK_ARCH` | Overrides the `lib/<arch>` directory name (`x64`, `x86`, `arm64`, `arm`, `arm_softfp`). |
| `GIGE_MTU` | Default for `grab --mtu`. |
| `GCAM_MVSDK_HW_TEST` | `1` enables the hardware tests (developers). |

To keep the SDK somewhere other than `~/.gcam-mvsdk`:

```bash
export GCAM_MVSDK_HOME=/data/gcam-mvsdk     # add this line to ~/.bashrc as well
bash install.bash
```

To use an SDK that is already extracted somewhere:

```bash
export GCAM_MVSDK_DIR="/opt/linuxSDK_V2.1.0.49(202511141513)"
gcam-mvsdk-rs list
```

## For developers

- Conventions, layout and procedures for working on this repository: [CLAUDE.md](CLAUDE.md)
- Vendor SDK manual (English, SDK 2.1.0.49): [docs/LinuxSDK_Document_V2.1.0.49.pdf](docs/LinuxSDK_Document_V2.1.0.49.pdf),
  taken from the Linux SDK archive on HuaTeng's download page, http://huatengvision.com/download/1/,
  where newer SDK versions and the Chinese readme are also published. Chapter "The camera cannot
  be found when running the Demo" is the source of the network requirements in
  [Known issues](#known-issues).
- `bash install.bash --dev` builds in place and puts the SDK in `./vendor/` instead of your home.
- [.claude/skills/](.claude/skills/) holds task guides for Claude Code, both for using the driver
  (listing, discovering, renaming, re-addressing, configuring and grabbing from cameras, the live
  view and capabilities examples, SDK setup) and for developing it (wrapping SDK functions,
  releases, hardware tests). Each is a `SKILL.md` with steps, exact commands and safety rules.

## License

The Rust source code is licensed under either of the Apache License, Version 2.0
(`LICENSE-APACHE`) or the MIT license (`LICENSE-MIT`), at your option. The HuaTeng Vision MVSDK is
proprietary vendor software; it is not included and not covered by this license.
