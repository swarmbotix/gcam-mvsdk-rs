//! `gcam-mvsdk-rs`: command-line tool for HuaTeng Vision cameras.

use std::fs;
use std::io::BufWriter;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::json;

use gcam_mvsdk_rs::doctor::{self, Check, Level};
use gcam_mvsdk_rs::setup::{self, Manifest, SetupOptions, SetupOutcome};
use gcam_mvsdk_rs::{
    host_os, normalize_mac, resolve_library, Candidate, DeviceInfo, Error, OutputFormat, ParameterTeam, Result,
    SearchConfig, Sdk, TargetOs, TriggerMode,
};

#[derive(Parser)]
#[command(
    name = "gcam-mvsdk-rs",
    version,
    about = "List, configure and test HuaTeng Vision GigE / USB cameras through the vendor MVSDK",
    after_help = "The vendor SDK is loaded at runtime from $GCAM_MVSDK_DIR, ./vendor/<version>/, \
                  $GCAM_MVSDK_HOME/<version>/ (default ~/.gcam-mvsdk), or the system library path. \
                  Run `gcam-mvsdk-rs setup` once to install it."
)]
struct Cli {
    /// Directory for the per-camera files the SDK writes when a camera is
    /// opened [default: <SDK home>/data, the SDK home being $GCAM_MVSDK_HOME or ~/.gcam-mvsdk]
    #[arg(long, global = true, value_name = "DIR")]
    data_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Download, verify and extract the vendor SDK into $GCAM_MVSDK_HOME/<version>/ (default ~/.gcam-mvsdk)
    Setup(SetupArgs),
    /// List every camera the SDK can see
    List {
        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
        /// Do not open cameras to read their user-set names
        #[arg(long)]
        no_open: bool,
    },
    /// Write a user-set name (DeviceUserID) to the camera with serial number SN
    Rename {
        /// Serial number of the camera
        #[arg(value_name = "SN")]
        serial: String,
        /// New name (at most 31 bytes; not \ / : * ? " < > |)
        name: String,
    },
    /// Assign a static IP configuration to the GigE camera with this MAC address
    Setip {
        /// Camera MAC address (any of 00-1A-2B-3C-4D-5E, 00:1a:2b:3c:4d:5e)
        mac: String,
        /// New camera IP address
        ip: Ipv4Addr,
        /// Subnet mask, for example 255.255.255.0
        mask: Ipv4Addr,
        /// Gateway (0.0.0.0 when there is none)
        #[arg(value_name = "GW")]
        gateway: Ipv4Addr,
    },
    /// Show settings and capabilities of one camera
    Info {
        /// User-set name or serial number
        #[arg(value_name = "NAME_OR_SN")]
        camera: String,
    },
    /// Save frames from one camera as PNG files
    Grab(GrabArgs),
    /// Change frame rate and exposure of one camera; without options, print them
    Set(SetArgs),
    /// Check the SDK installation, privileges and network configuration
    Doctor,
}

#[derive(Args)]
struct SetupArgs {
    /// Install from a local archive instead of downloading
    #[arg(long, value_name = "FILE")]
    from: Option<PathBuf>,
    /// Release version from sdk-manifest.toml [default: newest for this OS]
    #[arg(long = "version", value_name = "VERSION")]
    sdk_version: Option<String>,
    /// Root directory; the SDK is placed in <DEST>/<version>/ [default: $GCAM_MVSDK_HOME, else ~/.gcam-mvsdk]
    #[arg(long, value_name = "DIR")]
    dest: Option<PathBuf>,
    /// Replace an existing installation of the same version
    #[arg(long)]
    force: bool,
    /// Keep the downloaded archive in <DEST>/downloads/
    #[arg(long)]
    keep_archive: bool,
    /// Install a release that has no SHA-256 in sdk-manifest.toml
    #[arg(long)]
    accept_unverified: bool,
    /// Do not show download progress
    #[arg(long, short)]
    quiet: bool,
}

#[derive(Args)]
struct GrabArgs {
    /// User-set name or serial number
    #[arg(value_name = "NAME_OR_SN")]
    camera: String,
    /// Number of frames to save
    #[arg(short = 'n', long = "count", default_value_t = 1, value_name = "N")]
    count: u32,
    /// Output directory
    #[arg(short = 'o', long = "out", default_value = ".", value_name = "DIR")]
    out: PathBuf,
    /// Exposure time in microseconds (turns automatic exposure off)
    #[arg(long, value_name = "US")]
    exposure_us: Option<f64>,
    /// Time to wait for each frame, in milliseconds
    #[arg(long, default_value_t = 2000, value_name = "MS")]
    timeout_ms: u64,
    /// Largest GigE packet size to select, normally the NIC MTU
    /// [default: $GIGE_MTU if set, otherwise keep the camera setting]
    #[arg(long, value_name = "BYTES")]
    mtu: Option<u32>,
}

#[derive(Args)]
struct SetArgs {
    /// User-set name or serial number
    #[arg(value_name = "NAME_OR_SN")]
    camera: String,
    /// Frame-rate cap in Hz; 0 removes the cap (only some GigE models support it)
    #[arg(long, value_name = "HZ")]
    frame_rate: Option<i32>,
    /// Exposure time in microseconds (turns automatic exposure off)
    #[arg(long, value_name = "US")]
    exposure_us: Option<f64>,
    /// Automatic exposure on or off
    #[arg(long, value_name = "on|off")]
    auto_exposure: Option<OnOff>,
    /// Store the settings in this parameter group, loaded again when the camera is next opened
    #[arg(long, value_name = "A|B|C|D", ignore_case = true)]
    save: Option<Group>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
enum OnOff {
    On,
    Off,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
#[value(rename_all = "UPPER")]
enum Group {
    A,
    B,
    C,
    D,
}

impl Group {
    fn team(self) -> ParameterTeam {
        match self {
            Group::A => ParameterTeam::A,
            Group::B => ParameterTeam::B,
            Group::C => ParameterTeam::C,
            Group::D => ParameterTeam::D,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    let data_dir = cli.data_dir.clone();
    match cli.command {
        Command::Setup(args) => cmd_setup(args),
        Command::List { json, no_open } => cmd_list(&load_sdk(data_dir.as_deref(), !no_open)?, json, no_open),
        Command::Rename { serial, name } => cmd_rename(&load_sdk(data_dir.as_deref(), true)?, &serial, &name),
        Command::Setip { mac, ip, mask, gateway } => {
            cmd_setip(&load_sdk(data_dir.as_deref(), false)?, &mac, ip, mask, gateway)
        }
        Command::Info { camera } => cmd_info(&load_sdk(data_dir.as_deref(), true)?, &camera),
        Command::Grab(args) => cmd_grab(&load_sdk(data_dir.as_deref(), true)?, args),
        Command::Set(args) => cmd_set(&load_sdk(data_dir.as_deref(), true)?, args),
        Command::Doctor => cmd_doctor(),
    }
}

/// Loads the SDK. When the command opens cameras, the SDK's per-camera data
/// files are directed to `--data-dir` (default `<SDK home>/data`) instead of
/// the working directory.
fn load_sdk(data_dir: Option<&Path>, opens_cameras: bool) -> Result<Sdk> {
    let sdk = Sdk::load()?;
    if !opens_cameras {
        return Ok(sdk);
    }
    let dir = match data_dir {
        Some(d) => Some(d.to_path_buf()),
        None => gcam_mvsdk_rs::sdk_home_root().map(|h| h.join("data")),
    };
    if let Some(dir) = dir {
        match fs::create_dir_all(&dir) {
            Ok(()) => {
                if let Err(e) = sdk.set_data_directory(&dir) {
                    if !matches!(e, Error::Unsupported(_)) {
                        eprintln!("warning: could not set the SDK data directory to {}: {e}", dir.display());
                    }
                }
            }
            Err(e) => eprintln!("warning: could not create {}: {e}", dir.display()),
        }
    }
    Ok(sdk)
}

// ---------------------------------------------------------------------------
// setup
// ---------------------------------------------------------------------------

fn cmd_setup(args: SetupArgs) -> Result<ExitCode> {
    let manifest = Manifest::embedded();
    let dest_root = match args.dest {
        Some(d) => d,
        None => setup::default_dest_root()?,
    };
    let opts = SetupOptions {
        dest_root,
        force: args.force,
        keep_archive: args.keep_archive,
        accept_unverified: args.accept_unverified,
        progress: !args.quiet,
    };
    let os = host_os();
    let outcome = match &args.from {
        Some(file) => setup::install_from_file(&manifest, file, args.sdk_version.as_deref(), os.manifest_name(), &opts)?,
        None => {
            let release = match &args.sdk_version {
                Some(v) => manifest.find(v, os.manifest_name()).ok_or_else(|| {
                    Error::Setup(format!("sdk-manifest.toml lists no release {v} for {os}"))
                })?,
                None => manifest.latest(os.manifest_name()).ok_or_else(|| {
                    Error::Setup(format!(
                        "sdk-manifest.toml lists no vendor release for {os}. Download the SDK by hand and \
                         set GCAM_MVSDK_DIR to the extracted directory."
                    ))
                })?,
            };
            setup::install_from_network(release, &opts)?
        }
    };
    match outcome {
        SetupOutcome::Installed { version_dir, library, summary } => {
            println!(
                "Installed the SDK into {} ({} files, {:.1} MiB)",
                version_dir.display(),
                summary.files,
                summary.bytes as f64 / (1024.0 * 1024.0)
            );
            report_library(&version_dir, library.as_deref());
        }
        SetupOutcome::AlreadyInstalled { version_dir, library } => {
            println!("The SDK is already installed in {} (use --force to reinstall)", version_dir.display());
            report_library(&version_dir, library.as_deref());
        }
        SetupOutcome::WindowsInstallerReady { archive, installer, installed_library, instructions } => {
            println!("Verified installer archive: {}", archive.display());
            if let Some(exe) = installer {
                println!("Installer: {}", exe.display());
            }
            println!("{instructions}");
            if installed_library.is_none() {
                return Ok(ExitCode::from(2));
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn report_library(version_dir: &Path, library: Option<&Path>) {
    match library {
        Some(lib) => println!("Library for this host: {}", lib.display()),
        None => println!(
            "warning: {} holds no library for this host architecture ({})",
            version_dir.join("lib").display(),
            gcam_mvsdk_rs::host_arch_dir().unwrap_or("unknown")
        ),
    }
    let udev = version_dir.join("udev");
    if host_os() == TargetOs::Linux && udev.is_dir() {
        println!(
            "USB cameras only: install the udev rules once with\n  sudo cp {}/*.rules /etc/udev/rules.d/ && sudo udevadm control --reload-rules",
            udev.display()
        );
    }
    println!("GigE discovery needs root or CAP_NET_RAW; run `gcam-mvsdk-rs doctor` to check the host.");
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

fn read_user_name(sdk: &Sdk, dev: &DeviceInfo) -> std::result::Result<String, String> {
    sdk.open(dev).and_then(|c| c.user_name()).map_err(|e| e.to_string())
}

fn cmd_list(sdk: &Sdk, as_json: bool, no_open: bool) -> Result<ExitCode> {
    let devs = sdk.enumerate()?;
    if as_json {
        let items: Vec<serde_json::Value> = devs
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let (user_name, user_name_error) = if no_open {
                    (None, None)
                } else {
                    match read_user_name(sdk, d) {
                        Ok(n) => (Some(n), None),
                        Err(e) => (None, Some(e)),
                    }
                };
                json!({
                    "index": i,
                    "user_name": user_name,
                    "user_name_error": user_name_error,
                    "friendly_name": d.friendly_name,
                    "product_name": d.product_name,
                    "product_series": d.product_series,
                    "serial": d.serial,
                    "port_type": d.port_type,
                    "sensor_type": d.sensor_type,
                    "driver_version": d.driver_version,
                    "instance": d.instance,
                    "gige": gige_json(d),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&items).expect("json serializes"));
        return Ok(ExitCode::SUCCESS);
    }

    if devs.is_empty() {
        println!("No cameras found. Check cable, power, and that the host NIC is on the same subnet.");
        if doctor::privilege_check().level >= Level::Warn {
            println!("This process has neither root nor CAP_NET_RAW, which GigE discovery needs; see `gcam-mvsdk-rs doctor`.");
        }
        return Ok(ExitCode::SUCCESS);
    }
    println!("\nFound {} camera(s)\n", devs.len());
    let line = "\u{2500}".repeat(70);
    for (i, d) in devs.iter().enumerate() {
        println!("{line}");
        println!("[{i}]");
        if !no_open {
            let user = match read_user_name(sdk, d) {
                Ok(n) if n.is_empty() => "<empty>".to_string(),
                Ok(n) => n,
                Err(e) => format!("<could not open: {e}>"),
            };
            println!("    User-set name : {user}");
        }
        println!("    Product       : {}", d.product_name);
        println!("    Serial number : {}", d.serial);
        println!("    Port type     : {}", d.port_type);
        if d.is_gige() {
            match (d.gige_ip(), d.gige_mac()) {
                (Ok(ip), Ok(mac)) => {
                    println!("    Camera        : {}  MAC {}", ip.camera, mac.camera);
                    println!("    Host NIC      : {}  MAC {}", ip.host, mac.host);
                    if !ip.same_subnet() {
                        println!("    WARNING       : camera is not in the host NIC subnet; use `setip`");
                    }
                }
                (Err(e), _) | (_, Err(e)) => println!("    (GigE info unavailable: {e})"),
            }
        }
    }
    println!("{line}");
    Ok(ExitCode::SUCCESS)
}

fn gige_json(d: &DeviceInfo) -> serde_json::Value {
    if !d.is_gige() {
        return serde_json::Value::Null;
    }
    match (d.gige_ip(), d.gige_mac()) {
        (Ok(ip), Ok(mac)) => json!({
            "camera_ip": ip.camera.ip.to_string(),
            "camera_mask": ip.camera.mask.to_string(),
            "camera_gateway": ip.camera.gateway.to_string(),
            "camera_mac": mac.camera,
            "host_ip": ip.host.ip.to_string(),
            "host_mask": ip.host.mask.to_string(),
            "host_gateway": ip.host.gateway.to_string(),
            "host_mac": mac.host,
            "same_subnet": ip.same_subnet(),
        }),
        (Err(e), _) | (_, Err(e)) => json!({ "error": e.to_string() }),
    }
}

// ---------------------------------------------------------------------------
// rename
// ---------------------------------------------------------------------------

fn cmd_rename(sdk: &Sdk, serial: &str, name: &str) -> Result<ExitCode> {
    gcam_mvsdk_rs::validate_user_name(name)?;
    let devs = sdk.enumerate()?;
    let dev = devs.iter().find(|d| d.serial == serial).ok_or_else(|| Error::DeviceNotFound {
        query: serial.to_string(),
        available: devs.iter().map(|d| format!("SN {}", d.serial)).collect(),
    })?;
    let cam = sdk.open(dev)?;
    let old = cam.user_name().unwrap_or_default();
    cam.set_user_name(name)?;
    let now = cam.user_name()?;
    println!("SN {serial}: user-set name {old:?} -> {now:?}");
    if now != name {
        println!("warning: the camera reports {now:?} after writing {name:?}");
    }
    println!("The name is stored in the camera's non-volatile memory and survives power cycles.");
    Ok(ExitCode::SUCCESS)
}

// ---------------------------------------------------------------------------
// setip
// ---------------------------------------------------------------------------

fn is_contiguous_mask(mask: Ipv4Addr) -> bool {
    let m = u32::from(mask);
    m.leading_ones() + m.trailing_zeros() == 32
}

fn cmd_setip(sdk: &Sdk, mac: &str, ip: Ipv4Addr, mask: Ipv4Addr, gateway: Ipv4Addr) -> Result<ExitCode> {
    let target = normalize_mac(mac).ok_or_else(|| Error::InvalidArgument(format!("{mac:?} is not a MAC address")))?;
    if !is_contiguous_mask(mask) || mask.is_unspecified() {
        return Err(Error::InvalidArgument(format!("{mask} is not a valid subnet mask")));
    }
    if ip.is_unspecified() || ip.is_broadcast() || ip.is_multicast() || ip.is_loopback() {
        return Err(Error::InvalidArgument(format!("{ip} cannot be assigned to a camera")));
    }
    let devs = sdk.enumerate()?;
    let mut available = Vec::new();
    let mut found = None;
    for d in devs.iter().filter(|d| d.is_gige()) {
        match d.gige_mac() {
            Ok(m) => {
                if normalize_mac(&m.camera).as_deref() == Some(target.as_str()) {
                    found = Some(d);
                    break;
                }
                available.push(format!("{} (SN {})", m.camera, d.serial));
            }
            Err(e) => available.push(format!("<MAC unavailable: {e}> (SN {})", d.serial)),
        }
    }
    let dev = found.ok_or_else(|| Error::DeviceNotFound { query: mac.to_string(), available })?;
    if let Ok(cur) = dev.gige_ip() {
        println!("SN {}: current {}", dev.serial, cur.camera);
        if !cur.host.ip.is_unspecified() {
            let host_net = gcam_mvsdk_rs::IpConfig { ip: cur.host.ip, mask: cur.host.mask, gateway: cur.host.gateway };
            if !host_net.contains(ip) {
                println!(
                    "note: {ip} is outside the subnet of the host NIC that found the camera ({}/{}); \
                     the camera will not be reachable from that NIC afterwards",
                    host_net.network(),
                    host_net.prefix_len()
                );
            }
        }
    }
    let net = gcam_mvsdk_rs::IpConfig { ip, mask, gateway };
    if !gateway.is_unspecified() && !net.contains(gateway) {
        println!("note: gateway {gateway} is outside {}/{}", net.network(), net.prefix_len());
    }
    sdk.gige_set_ip(dev, ip, mask, gateway, true)?;
    println!("SN {}: set {} (persistent)", dev.serial, net);
    println!("Run `gcam-mvsdk-rs list` to confirm; some cameras apply the address after a restart.");
    Ok(ExitCode::SUCCESS)
}

// ---------------------------------------------------------------------------
// info
// ---------------------------------------------------------------------------

fn show<T: std::fmt::Display>(r: Result<T>) -> String {
    match r {
        Ok(v) => v.to_string(),
        Err(e) => format!("(error: {e})"),
    }
}

fn cmd_info(sdk: &Sdk, query: &str) -> Result<ExitCode> {
    let dev = sdk.find_device(query)?;
    let cam = sdk.open(&dev)?;
    let user = cam.user_name().unwrap_or_default();
    println!("Name: {user}  SN: {}  Type: {}", dev.serial, dev.port_type);
    println!("Model: {}  Product: {}  Sensor: {}", dev.friendly_name, dev.product_name, dev.sensor_type);
    println!("Firmware: {}  Driver: {}", show(cam.firmware_version()), dev.driver_version);
    if dev.is_gige() {
        if let (Ok(ip), Ok(mac)) = (dev.gige_ip(), dev.gige_mac()) {
            println!("Camera: {}  MAC {}", ip.camera, mac.camera);
            println!("Host NIC: {}  MAC {}", ip.host, mac.host);
        }
    }
    let cap = cam.capability()?;

    println!("\n-- Exposure / Shutter --");
    match cam.exposure_us() {
        Ok(us) if us > 0.0 => {
            println!(
                "  Exposure time : {us:.1} us  ({:.3} ms)  ->  1/{:.0} s",
                us / 1000.0,
                1_000_000.0 / us
            );
            println!("  Max FPS from shutter alone: {:.1}", 1_000_000.0 / us);
        }
        r => println!("  Exposure time : {}", show(r)),
    }
    println!(
        "  Auto-exposure : {}",
        match cam.auto_exposure() {
            Ok(true) => "ON".to_string(),
            Ok(false) => "OFF".to_string(),
            Err(e) => format!("(error: {e})"),
        }
    );
    if let Ok((lo, hi, step)) = cam.exposure_range_us() {
        println!("  Exposure range: {lo:.1} - {hi:.1} us  (step {step:.1} us)");
    }
    if let Ok(t) = cam.exposure_line_time_us() {
        println!("  Line time     : {t:.3} us");
    }

    println!("\n-- Gain --");
    println!("  Analog gain   : {}", show(cam.analog_gain()));
    if let Ok((r, g, b)) = cam.rgb_gain() {
        println!("  Digital gain  : R={r}  G={g}  B={b}");
    }

    println!("\n-- Resolution / Frame rate --");
    match cam.image_resolution() {
        Ok(r) => println!("  Current       : {} x {}  ({})", r.width, r.height, r.description),
        Err(e) => println!("  Current       : (error: {e})"),
    }
    println!("  Sensor max    : {} x {}", cap.max_width(), cap.max_height());
    println!("  Frame-rate cap: {}", show(cam.frame_rate().map(|r| if r <= 0 { "none".to_string() } else { format!("{r} Hz") })));
    match cam.frame_speed() {
        Ok(i) => println!(
            "  Frame speed   : index {i}{}",
            cap.frame_speeds.iter().find(|s| s.index == i).map(|s| format!(" ({})", s.description)).unwrap_or_default()
        ),
        Err(e) => println!("  Frame speed   : (error: {e})"),
    }

    println!("\n-- Trigger --");
    println!("  Mode          : {}", show(cam.trigger_mode()));
    println!("  Delay         : {} us", show(cam.trigger_delay_us()));
    println!("  Count         : {}", show(cam.trigger_count()));

    println!("\n-- Image Quality --");
    println!("  Gamma         : {}", show(cam.gamma()));
    println!("  Contrast      : {}", show(cam.contrast()));
    println!("  Saturation    : {}", show(cam.saturation()));
    println!("  Sharpness     : {}", show(cam.sharpness()));

    println!("\n-- Sensor --");
    println!("  Type          : {}", if cap.is_mono() { "Monochrome" } else { "Color" });
    println!(
        "  ISP output    : {:#010x} ({:?})",
        cam.output_format().media_type(),
        cam.output_format()
    );

    println!("\n-- Capabilities --");
    if !cap.image_sizes.is_empty() {
        println!("  Preset resolutions:");
        for r in &cap.image_sizes {
            println!("    [{}] {}x{}  '{}'", r.index, r.width, r.height, r.description);
        }
    }
    if !cap.frame_speeds.is_empty() {
        println!("  Frame-speed tiers:");
        for s in &cap.frame_speeds {
            println!("    [{}] '{}'", s.index, s.description);
        }
    }
    if !cap.pack_lengths.is_empty() {
        println!("  GigE packet sizes:");
        for p in &cap.pack_lengths {
            println!("    [{}] {} bytes  '{}'", p.index, p.size, p.description);
        }
    }
    if !cap.media_types.is_empty() {
        println!("  Sensor output formats:");
        for m in &cap.media_types {
            println!("    [{}] {:#010x}  '{}'", m.index, m.media_type, m.description);
        }
    }
    if !cap.triggers.is_empty() {
        println!("  Trigger modes:");
        for t in &cap.triggers {
            println!("    [{}] '{}'", t.index, t.description);
        }
    }
    println!("  I/O: {} input(s), {} output(s)", cap.input_io_count, cap.output_io_count);
    println!();
    Ok(ExitCode::SUCCESS)
}

// ---------------------------------------------------------------------------
// grab
// ---------------------------------------------------------------------------

fn file_stem_for(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '_' })
        .collect();
    if s.is_empty() {
        "camera".to_string()
    } else {
        s
    }
}

fn write_png(path: &Path, info: &gcam_mvsdk_rs::ImageInfo, data: &[u8]) -> Result<()> {
    let (color, channels) = match OutputFormat::from_media_type(info.media_type) {
        OutputFormat::Mono8 => (png::ColorType::Grayscale, 1usize),
        OutputFormat::Rgb8 => (png::ColorType::Rgb, 3),
        OutputFormat::Rgba8 => (png::ColorType::Rgba, 4),
        other => return Err(Error::InvalidArgument(format!("cannot write {other:?} as PNG"))),
    };
    let need = info.width as usize * info.height as usize * channels;
    if data.len() < need {
        return Err(Error::InvalidResponse(format!("image has {} bytes, expected {need}", data.len())));
    }
    let file = fs::File::create(path).map_err(|e| Error::io(format!("creating {}", path.display()), e))?;
    let mut enc = png::Encoder::new(BufWriter::new(file), info.width, info.height);
    enc.set_color(color);
    enc.set_depth(png::BitDepth::Eight);
    let png_err = |e: png::EncodingError| Error::io(format!("writing {}", path.display()), std::io::Error::other(e));
    let mut writer = enc.write_header().map_err(png_err)?;
    writer.write_image_data(&data[..need]).map_err(png_err)?;
    writer.finish().map_err(png_err)?;
    Ok(())
}

fn cmd_grab(sdk: &Sdk, args: GrabArgs) -> Result<ExitCode> {
    if args.count == 0 {
        return Err(Error::InvalidArgument("--count must be at least 1".into()));
    }
    let dev = sdk.find_device(&args.camera)?;
    let cam = sdk.open(&dev)?;
    let cap = cam.capability()?;
    let user = cam.user_name().unwrap_or_default();
    let stem = file_stem_for(if user.is_empty() { &dev.serial } else { &user });

    let format = if cap.is_mono() { OutputFormat::Mono8 } else { OutputFormat::Rgb8 };
    cam.set_output_format(format)?;
    cam.set_trigger_mode(TriggerMode::Continuous)?;
    if let Some(us) = args.exposure_us {
        cam.set_auto_exposure(false)?;
        cam.set_exposure_us(us)?;
    }
    let mtu = args.mtu.or_else(|| std::env::var("GIGE_MTU").ok().and_then(|v| v.trim().parse().ok()));
    if let (Some(mtu), true) = (mtu, dev.is_gige()) {
        let size = cam.set_gige_packet_size(mtu)?;
        println!("GigE packet size: {size} bytes (limit {mtu})");
    }
    fs::create_dir_all(&args.out).map_err(|e| Error::io(format!("creating {}", args.out.display()), e))?;
    println!(
        "Camera {:?} SN {}: {} sensor, max {}x{}, exposure {} us",
        user,
        dev.serial,
        if cap.is_mono() { "mono" } else { "color" },
        cap.max_width(),
        cap.max_height(),
        show(cam.exposure_us().map(|v| format!("{v:.1}")))
    );

    cam.play()?;
    let timeout = Duration::from_millis(args.timeout_ms);
    let mut saved = 0u32;
    let mut timeouts = 0u32;
    while saved < args.count {
        let frame = match cam.grab(timeout) {
            Ok(f) => f,
            Err(e) if e.is_timeout() => {
                timeouts += 1;
                eprintln!("timeout waiting for a frame ({timeouts}/3)");
                if timeouts >= 3 {
                    let _ = cam.pause();
                    eprintln!(
                        "No frames received. For GigE cameras check the packet size against the NIC MTU \
                         (try --mtu 1500), the firewall, and rp_filter (`gcam-mvsdk-rs doctor`)."
                    );
                    return Err(e);
                }
                continue;
            }
            Err(e) => {
                let _ = cam.pause();
                return Err(e);
            }
        };
        let raw_len = frame.len();
        let ts_ms = frame.timestamp().as_secs_f64() * 1000.0;
        let exp = frame.exposure_us();
        let image = frame.process()?;
        drop(frame);
        let path = args.out.join(format!("{stem}_{saved:04}.png"));
        write_png(&path, &image.info, &image.data)?;
        let mean = if image.data.is_empty() {
            0.0
        } else {
            image.data.iter().map(|&b| u64::from(b)).sum::<u64>() as f64 / image.data.len() as f64
        };
        println!(
            "saved {}  {}x{}  raw {} bytes  ts {:.1} ms  exposure {} us  mean {:.1}",
            path.display(),
            image.info.width,
            image.info.height,
            raw_len,
            ts_ms,
            exp,
            mean
        );
        saved += 1;
    }
    cam.pause()?;
    Ok(ExitCode::SUCCESS)
}

// ---------------------------------------------------------------------------
// set
// ---------------------------------------------------------------------------

fn cmd_set(sdk: &Sdk, args: SetArgs) -> Result<ExitCode> {
    if args.exposure_us.is_some() && args.auto_exposure == Some(OnOff::On) {
        return Err(Error::InvalidArgument("--exposure-us turns automatic exposure off; drop --auto-exposure on".into()));
    }
    let dev = sdk.find_device(&args.camera)?;
    let cam = sdk.open(&dev)?;
    let on_off = |r: Result<bool>| show(r.map(|on| if on { "on" } else { "off" }));
    let (rate, exposure, auto) = (show(cam.frame_rate()), show(cam.exposure_us()), on_off(cam.auto_exposure()));
    if args.frame_rate.is_none() && args.exposure_us.is_none() && args.auto_exposure.is_none() && args.save.is_none() {
        println!("frame rate: {rate} Hz (0 = no cap)");
        println!("exposure: {exposure} us");
        println!("auto exposure: {auto}");
        return Ok(ExitCode::SUCCESS);
    }
    if let Some(hz) = args.frame_rate {
        cam.set_frame_rate(hz)?;
        println!("frame rate: {} Hz (was {rate})", show(cam.frame_rate()));
    }
    if let Some(mode) = args.auto_exposure {
        cam.set_auto_exposure(mode == OnOff::On)?;
        println!("auto exposure: {} (was {auto})", on_off(cam.auto_exposure()));
    }
    if let Some(us) = args.exposure_us {
        cam.set_auto_exposure(false)?;
        cam.set_exposure_us(us)?;
        println!("exposure: {} us (was {exposure}), auto exposure: {}", show(cam.exposure_us()), on_off(cam.auto_exposure()));
    }
    match args.save {
        Some(group) => {
            cam.save_parameter(group.team())?;
            println!("saved to parameter group {group:?}: persisted, loaded again when the camera is next opened");
        }
        None => println!("not saved: the change is in RAM until the camera is reconfigured; add --save A|B|C|D to keep it"),
    }
    Ok(ExitCode::SUCCESS)
}

// ---------------------------------------------------------------------------
// doctor
// ---------------------------------------------------------------------------

fn sdk_checks() -> (Vec<Check>, Option<Sdk>) {
    let mut checks = Vec::new();
    let cfg = match SearchConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            checks.push(Check::new(Level::Fail, "unsupported platform").with(e.to_string()));
            return (checks, None);
        }
    };
    match gcam_mvsdk_rs::sdk_home() {
        Some((home, source)) => {
            checks.push(Check::new(Level::Info, format!("SDK home: {} ({source})", home.display())))
        }
        None => checks.push(Check::new(
            Level::Info,
            "SDK home: unknown (no home directory; set GCAM_MVSDK_HOME or GCAM_MVSDK_DIR)",
        )),
    }
    if let Some(dir) = &cfg.sdk_dir {
        checks.push(
            Check::new(Level::Info, format!("GCAM_MVSDK_DIR={}", dir.display()))
                .with("only this directory is searched while GCAM_MVSDK_DIR is set"),
        );
    }
    if host_os() == TargetOs::Windows && cfg.recorded_library.is_none() {
        if let Some(dll) = setup::locate_installed_windows_library() {
            if let Some(root) = gcam_mvsdk_rs::sdk_home_root() {
                if setup::record_library(&root, &dll).is_ok() {
                    checks.push(Check::new(Level::Info, format!("recorded the installed DLL {}", dll.display())));
                }
            }
        }
    }
    match resolve_library(&cfg) {
        Ok(r) => match &r.candidate {
            Candidate::Path(p) => checks.push(Check::new(Level::Ok, format!("SDK library found: {}", p.display()))),
            Candidate::SystemName(n) => {
                let mut c = Check::new(Level::Info, format!("no SDK in the documented locations; trying {n} via the system loader"));
                for s in &r.searched {
                    c = c.with(format!("looked in {s}"));
                }
                checks.push(c);
            }
        },
        Err(e) => {
            checks.push(Check::new(Level::Fail, "SDK library not found").with(e.to_string()));
            return (checks, None);
        }
    }
    let sdk = match load_sdk(None, false) {
        Ok(s) => s,
        Err(e) => {
            checks.push(Check::new(Level::Fail, "SDK library could not be loaded").with(e.to_string()));
            return (checks, None);
        }
    };
    let runtime_version = sdk.version();
    checks.push(Check::new(
        Level::Ok,
        format!(
            "SDK loaded and initialised: version {} from {}",
            runtime_version.as_deref().unwrap_or("unknown"),
            sdk.library_path().display()
        ),
    ));
    // <version_dir>/lib/<arch>/<library>
    let version_dir = sdk.library_path().parent().and_then(Path::parent).and_then(Path::parent);
    match version_dir.map(setup::read_install_manifest) {
        Some(Ok(m)) => {
            let mut c = Check::new(Level::Ok, format!("installed by setup: version {} on {}", m.version, m.installed_at))
                .with(format!("sha256 {}", m.sha256))
                .with(format!("source {}", m.source_url));
            if let Some(v) = &runtime_version {
                if v != &m.version {
                    c.level = Level::Warn;
                    c = c.with(format!("the library reports version {v}, manifest.json says {}", m.version));
                }
            }
            checks.push(c);
        }
        _ => checks.push(Check::new(
            Level::Info,
            "no manifest.json next to the library (not installed by `setup`; fine for GCAM_MVSDK_DIR or a system install)",
        )),
    }
    let missing = sdk.api().missing_optional_symbols();
    if !missing.is_empty() {
        checks.push(Check::new(Level::Info, format!("optional functions not exported: {}", missing.join(", "))));
    }
    (checks, Some(sdk))
}

fn camera_checks(sdk: &Sdk) -> Vec<Check> {
    let mut out = Vec::new();
    let devs = match sdk.enumerate() {
        Ok(d) => d,
        Err(e) => return vec![Check::new(Level::Fail, "enumeration failed").with(e.to_string())],
    };
    if devs.is_empty() {
        out.push(
            Check::new(Level::Warn, "no cameras found")
                .with("Check power and cabling, the privilege, rp_filter and firewall findings above,")
                .with("and that a host NIC has an address in the camera subnet."),
        );
        return out;
    }
    let mut c = Check::new(Level::Ok, format!("{} camera(s) found", devs.len()));
    let mut level = Level::Ok;
    for d in &devs {
        if !d.is_gige() {
            c = c.with(format!("SN {}  {}  ({})", d.serial, d.friendly_name, d.port_type));
            continue;
        }
        match d.gige_ip() {
            Ok(ip) if ip.same_subnet() => {
                c = c.with(format!("SN {}  {}  camera {} via host {}", d.serial, d.friendly_name, ip.camera.ip, ip.host.ip));
            }
            Ok(ip) => {
                level = Level::Warn;
                c = c.with(format!(
                    "SN {}  camera {} is NOT in the subnet of host NIC {}/{}; fix with `gcam-mvsdk-rs setip`",
                    d.serial,
                    ip.camera.ip,
                    ip.host.ip,
                    ip.host.prefix_len()
                ));
            }
            Err(e) => c = c.with(format!("SN {}  GigE info unavailable: {e}", d.serial)),
        }
    }
    c.level = level;
    out.push(c);
    out
}

fn cmd_doctor() -> Result<ExitCode> {
    let (mut checks, sdk) = sdk_checks();
    checks.extend(doctor::system_checks());
    if let Some(sdk) = &sdk {
        checks.extend(camera_checks(sdk));
    }
    for c in &checks {
        println!("{c}");
    }
    let worst = checks.iter().map(|c| c.level).max().unwrap_or(Level::Ok);
    let fails = checks.iter().filter(|c| c.level == Level::Fail).count();
    let warns = checks.iter().filter(|c| c.level == Level::Warn).count();
    println!("\n{fails} failure(s), {warns} warning(s)");
    Ok(if worst == Level::Fail { ExitCode::FAILURE } else { ExitCode::SUCCESS })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks() {
        assert!(is_contiguous_mask("255.255.255.0".parse().unwrap()));
        assert!(!is_contiguous_mask("255.0.255.0".parse().unwrap()));
    }

    #[test]
    fn file_stems() {
        assert_eq!(file_stem_for("sb00"), "sb00");
        assert_eq!(file_stem_for("a b/c"), "a_b_c");
        assert_eq!(file_stem_for(""), "camera");
    }

    #[test]
    fn set_args() {
        let cli = Cli::try_parse_from([
            "gcam-mvsdk-rs", "set", "sb00", "--frame-rate", "30", "--exposure-us", "5000", "--auto-exposure", "off",
            "--save", "B",
        ])
        .unwrap();
        let Command::Set(a) = cli.command else { panic!("not the set command") };
        assert_eq!((a.camera.as_str(), a.frame_rate, a.exposure_us), ("sb00", Some(30), Some(5000.0)));
        assert_eq!((a.auto_exposure, a.save), (Some(OnOff::Off), Some(Group::B)));
        assert_eq!(a.save.map(Group::team), Some(ParameterTeam::B));
        let cli = Cli::try_parse_from(["gcam-mvsdk-rs", "set", "sb00", "--auto-exposure", "on", "--save", "d"]).unwrap();
        let Command::Set(a) = cli.command else { panic!("not the set command") };
        assert_eq!((a.auto_exposure, a.save), (Some(OnOff::On), Some(Group::D)));
        let cli = Cli::try_parse_from(["gcam-mvsdk-rs", "set", "sb00"]).unwrap();
        let Command::Set(a) = cli.command else { panic!("not the set command") };
        assert!(a.frame_rate.is_none() && a.exposure_us.is_none() && a.auto_exposure.is_none() && a.save.is_none());
        assert!(Cli::try_parse_from(["gcam-mvsdk-rs", "set", "sb00", "--auto-exposure", "maybe"]).is_err());
        assert!(Cli::try_parse_from(["gcam-mvsdk-rs", "set", "sb00", "--save", "E"]).is_err());
        assert!(Cli::try_parse_from(["gcam-mvsdk-rs", "set"]).is_err());
    }

    #[test]
    fn cli_parses() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
        let cli = Cli::try_parse_from(["gcam-mvsdk-rs", "grab", "sb00", "-n", "3", "-o", "/tmp/x"]).unwrap();
        assert!(matches!(cli.command, Command::Grab(GrabArgs { count: 3, .. })));
        let cli = Cli::try_parse_from(["gcam-mvsdk-rs", "setup", "--from", "a.tar.gz", "--dest", "vendor/"]).unwrap();
        assert!(matches!(cli.command, Command::Setup(SetupArgs { from: Some(_), .. })));
        assert!(Cli::try_parse_from(["gcam-mvsdk-rs", "setip", "00-11-22-33-44-55", "1.2.3", "255.255.255.0", "0.0.0.0"]).is_err());
    }
}
