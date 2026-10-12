# gcam-mvsdk-rs

`gcam-mvsdk-rs` is a command-line tool and a Rust library for HuaTeng Vision (华腾威视) GigE and USB
industrial cameras on Linux. It finds the cameras on your network, renames them, sets their IP
address, reads and changes exposure, and grabs frames.

It does this by loading the vendor's own SDK library (`libMVSDK.so`) at run time. That library is
not in this repository, because HuaTeng does not permit redistribution. The installer downloads it
from HuaTeng's website, checks its SHA-256 against a value recorded here, and stores it in your home
directory. Nothing from the vendor is compiled into the tool.

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
6. **Privileges for camera discovery (asks first).** GigE cameras answer a broadcast that Linux only
   allows from a process with the `CAP_NET_RAW` capability. The script shows the command
   `sudo setcap cap_net_raw,cap_net_admin+ep ~/.cargo/bin/gcam-mvsdk-rs` and asks before running
   it. Saying no is fine; see [Privileges](#privileges-and-network) for the alternative.
7. **USB udev rules (only with `--udev`).** Copies the vendor's two rule files to
   `/etc/udev/rules.d/` and reloads udev. Skip this unless you have USB cameras.
8. **Verifies.** Runs `gcam-mvsdk-rs doctor` and `gcam-mvsdk-rs list`, then prints where everything
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
| `setcap cap_net_raw,cap_net_admin+ep` on `~/.cargo/bin/gcam-mvsdk-rs` | GigE discovery broadcast | `sudo setcap -r ~/.cargo/bin/gcam-mvsdk-rs`, or just delete the file |
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
machine, and only when the process may open raw sockets (step 6 above, or `sudo -E`). USB cameras
are found through the udev rules.

## Commands

| Command | What it does |
|---|---|
| `gcam-mvsdk-rs list` | Every camera: user-set name, model, serial number, port, camera and host NIC IP/MAC. |
| `gcam-mvsdk-rs list --json` | The same as JSON, for scripts that start one process per camera. |
| `gcam-mvsdk-rs rename <SN> <NAME>` | Store a user-set name in the camera with that serial number (survives power cycles). |
| `gcam-mvsdk-rs setip <MAC> <IP> <MASK> <GW>` | Give a GigE camera a static address; works even when it is on the wrong subnet. Use `0.0.0.0` for no gateway. |
| `gcam-mvsdk-rs info <NAME or SN>` | Exposure, gain, resolution, trigger, image settings and capabilities of one camera. |
| `gcam-mvsdk-rs grab <NAME or SN> -n 3 -o frames/` | Save frames as PNG. `--exposure-us 5000` sets exposure; `--mtu 1500` fits the packet size to the NIC. |
| `gcam-mvsdk-rs doctor` | Check the SDK, privileges, `rp_filter`, firewall, NIC subnets and camera addresses, and print the fix for each. |
| `gcam-mvsdk-rs setup` | Download and verify the SDK (`install.bash` runs this for you). |

`list --no-open` skips reading user-set names, which requires opening each camera briefly; use it
while another program is streaming. `gcam-mvsdk-rs <command> --help` lists every option.

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

Then fetch the SDK once and, if you want discovery without `sudo`, grant the capability:

```bash
gcam-mvsdk-rs setup
sudo setcap cap_net_raw,cap_net_admin+ep "$(command -v gcam-mvsdk-rs)"
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

Two things your program needs at run time, on every machine it runs on, exactly like the command:

1. The SDK installed: `gcam-mvsdk-rs setup`, or `bash install.bash`, or `GCAM_MVSDK_DIR` pointing
   at an extracted copy. The library finds it with the same search order as the command.
2. `CAP_NET_RAW` for GigE discovery: `sudo setcap cap_net_raw,cap_net_admin+ep target/release/your-binary`
   after each build, or run it with `sudo -E`.

## Privileges and network

GigE discovery needs four things. `gcam-mvsdk-rs doctor` checks each one and prints the command
for your machine.

```bash
# 1. Raw-socket capability for the command (install.bash offers this; repeat after reinstalling)
sudo setcap cap_net_raw,cap_net_admin+ep "$(command -v gcam-mvsdk-rs)"

# 2. Reverse-path filtering off, now and after reboot (replace enp2s0 with the camera network card)
sudo sysctl -w net.ipv4.conf.all.rp_filter=0 net.ipv4.conf.enp2s0.rp_filter=0
echo 'net.ipv4.conf.all.rp_filter=0' | sudo tee /etc/sysctl.d/90-gige-camera.conf

# 3. Firewall: allow the camera subnet (ufw shown; adjust the subnet)
sudo ufw allow from 192.168.1.0/24

# 4. Host NIC and cameras in the same subnet: give the NIC a static address, or move a camera
gcam-mvsdk-rs setip 00-1A-2B-3C-4D-5E 192.168.1.50 255.255.255.0 0.0.0.0

# USB cameras instead: vendor udev rules
sudo cp ~/.gcam-mvsdk/2.1.0.49/udev/*.rules /etc/udev/rules.d/ && sudo udevadm control --reload-rules
```

**What step 1 means.** `setcap` marks one file so that it may open raw network sockets without
being root. It applies to that file only, not to Rust, not to other programs, and it disappears
when the file is replaced (for example by `cargo install` again, which is why the installer
re-applies it). The command uses the capability for one purpose: the discovery broadcast and the
camera control packets of the GigE Vision protocol. If you prefer not to grant it, run
`sudo -E gcam-mvsdk-rs list` instead; `-E` keeps `GCAM_MVSDK_HOME` and `HOME` so the SDK is still
found. Inside Docker, use `network_mode: host` and `privileged: true`.

## Troubleshooting

| Symptom | Cause and fix |
|---|---|
| `gcam-mvsdk-rs: command not found` | `~/.cargo/bin` is not on `PATH` in this terminal. Run `source ~/.cargo/env`, or `export PATH="$HOME/.cargo/bin:$PATH"` and add it to `~/.bashrc`. Being in the repository directory does not help. |
| `list` finds 0 cameras | Most often missing privileges: run the `setcap` command above, or `sudo -E gcam-mvsdk-rs list`. Then check power and cabling, `rp_filter`, the firewall, and that a network card has an address in the camera subnet (`gcam-mvsdk-rs doctor` reports each). Discovery only reaches cameras on the same network segment. |
| `could not find the HuaTeng MVSDK library` | Run `bash install.bash` or `gcam-mvsdk-rs setup`, or set `GCAM_MVSDK_DIR`. The message lists every place that was checked. A wrong `GCAM_MVSDK_DIR` is reported, not ignored. |
| Status `-13`, `32774` (0x8006) or `-45` when opening a camera | The camera is busy: another program or another computer holds it (a running publisher, a container, the vendor GUI). Stop that program, wait a few seconds for the camera's heartbeat to expire, or power-cycle the camera. `list --no-open` still works meanwhile. |
| `Permission denied` | USB cameras: install the udev rules. `setcap`: needs `sudo`, and has no effect on file systems mounted `nosuid`. Files under `~/.gcam-mvsdk` owned by root (from an earlier `sudo` run): `sudo chown -R "$USER" ~/.gcam-mvsdk`. |
| `grab` times out | The GigE packet size exceeds the network card's MTU: use `--mtu 1500` (or enable jumbo frames). Also check the firewall and `rp_filter`. |
| Download fails, or the size or checksum does not match | The vendor may have moved or replaced the file. Download the Linux SDK from http://huatengvision.com/download/1/ and install it with `--from` (below). A checksum mismatch is never overridden. |

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

Removes `~/.cargo/bin/gcam-mvsdk-rs` (and with it the capability), then asks before deleting
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

- Design and background: [PLAN_RUST_WRAPPER.md](PLAN_RUST_WRAPPER.md)
- Conventions, layout and procedures for working on this repository: [CLAUDE.md](CLAUDE.md)
- `bash install.bash --dev` builds in place and puts the SDK in `./vendor/` instead of your home.

## License

The Rust source code is licensed under either of the Apache License, Version 2.0
(`LICENSE-APACHE`) or the MIT license (`LICENSE-MIT`), at your option. The HuaTeng Vision MVSDK is
proprietary vendor software; it is not included and not covered by this license.
