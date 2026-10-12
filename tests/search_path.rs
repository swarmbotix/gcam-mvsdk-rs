//! Library search order, exercised against a fake directory tree.

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use gcam_mvsdk_rs::{
    arch_dir_for, library_file_name, resolve_library, search_entries, version_dirs, Candidate, LoadError,
    SearchConfig, TargetOs, ENV_ARCH, ENV_HOME, ENV_SDK_DIR,
};

const LIB: &str = "libMVSDK.so";

fn touch(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b"not a real library").unwrap();
}

struct Tree {
    _tmp: tempfile::TempDir,
    root: PathBuf,
}

impl Tree {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        Tree { _tmp: tmp, root }
    }

    fn cfg(&self) -> SearchConfig {
        SearchConfig {
            sdk_dir: None,
            arch_dir: "x64".into(),
            library_name: LIB.into(),
            vendor_root: Some(self.root.join("checkout").join("vendor")),
            home_root: Some(self.root.join("home").join(".gcam-mvsdk")),
            recorded_library: None,
            use_system_path: true,
        }
    }

    fn vendor_lib(&self, version: &str) -> PathBuf {
        self.root.join("checkout/vendor").join(version).join("lib/x64").join(LIB)
    }

    fn home_lib(&self, version: &str) -> PathBuf {
        self.root.join("home/.gcam-mvsdk").join(version).join("lib/x64").join(LIB)
    }
}

fn resolved_path(cfg: &SearchConfig) -> PathBuf {
    match resolve_library(cfg).unwrap().candidate {
        Candidate::Path(p) => p,
        other => panic!("expected a path, got {other:?}"),
    }
}

#[test]
fn nothing_installed_falls_back_to_system_loader() {
    let t = Tree::new();
    let r = resolve_library(&t.cfg()).unwrap();
    assert_eq!(r.candidate, Candidate::SystemName(LIB.into()));
    assert_eq!(r.searched.len(), 2, "{:?}", r.searched);
    assert!(r.searched.iter().all(|s| s.contains("no SDK version installed")));
}

#[test]
fn nothing_installed_without_system_path_is_not_found() {
    let t = Tree::new();
    let mut cfg = t.cfg();
    cfg.use_system_path = false;
    match resolve_library(&cfg) {
        Err(LoadError::NotFound { library, searched, .. }) => {
            assert_eq!(library, LIB);
            assert_eq!(searched.len(), 2);
        }
        other => panic!("unexpected {other:?}"),
    }
    let msg = resolve_library(&cfg).unwrap_err().to_string();
    assert!(msg.contains("gcam-mvsdk-rs setup"), "{msg}");
    assert!(msg.contains("GCAM_MVSDK_DIR"), "{msg}");
}

#[test]
fn home_install_is_found() {
    let t = Tree::new();
    touch(&t.home_lib("2.1.0.49"));
    assert_eq!(resolved_path(&t.cfg()), t.home_lib("2.1.0.49"));
}

#[test]
fn vendor_checkout_wins_over_home() {
    let t = Tree::new();
    touch(&t.home_lib("2.1.0.49"));
    touch(&t.vendor_lib("2.1.0.49"));
    assert_eq!(resolved_path(&t.cfg()), t.vendor_lib("2.1.0.49"));
}

#[test]
fn newest_version_wins_within_a_root() {
    let t = Tree::new();
    touch(&t.home_lib("2.1.0.49"));
    touch(&t.home_lib("2.1.10.195"));
    touch(&t.home_lib("2.1.9.0"));
    // A non-version directory is ignored.
    touch(&t.root.join("home/.gcam-mvsdk/downloads/lib/x64").join(LIB));
    assert_eq!(resolved_path(&t.cfg()), t.home_lib("2.1.10.195"));
    let dirs = version_dirs(&t.root.join("home/.gcam-mvsdk"));
    let names: Vec<_> = dirs.iter().map(|d| d.file_name().unwrap().to_str().unwrap()).collect();
    assert_eq!(names, ["2.1.10.195", "2.1.9.0", "2.1.0.49"]);
}

#[test]
fn version_without_library_for_this_arch_is_skipped() {
    let t = Tree::new();
    touch(&t.root.join("checkout/vendor/2.1.0.49/lib/arm64").join(LIB));
    touch(&t.home_lib("2.1.0.49"));
    assert_eq!(resolved_path(&t.cfg()), t.home_lib("2.1.0.49"));
}

#[test]
fn env_dir_is_exclusive_and_checked_in_order() {
    let t = Tree::new();
    touch(&t.home_lib("2.1.0.49"));
    let sdk = t.root.join("sdk");
    let mut cfg = t.cfg();
    cfg.sdk_dir = Some(sdk.clone());

    // Set but empty: the home install must not be used.
    match resolve_library(&cfg) {
        Err(LoadError::NotFound { searched, .. }) => {
            assert_eq!(
                searched,
                vec![
                    sdk.join("lib/x64").join(LIB).display().to_string(),
                    sdk.join("lib").join(LIB).display().to_string(),
                    sdk.join(LIB).display().to_string(),
                ]
            );
        }
        other => panic!("unexpected {other:?}"),
    }

    touch(&sdk.join(LIB));
    assert_eq!(resolved_path(&cfg), sdk.join(LIB));
    touch(&sdk.join("lib").join(LIB));
    assert_eq!(resolved_path(&cfg), sdk.join("lib").join(LIB));
    touch(&sdk.join("lib/x64").join(LIB));
    assert_eq!(resolved_path(&cfg), sdk.join("lib/x64").join(LIB));
}

#[test]
fn env_dir_may_name_the_library_file() {
    let t = Tree::new();
    let file = t.root.join("somewhere").join(LIB);
    touch(&file);
    let mut cfg = t.cfg();
    cfg.sdk_dir = Some(file.clone());
    assert_eq!(resolved_path(&cfg), file);
}

#[test]
fn arch_override_selects_another_directory() {
    let t = Tree::new();
    touch(&t.root.join("home/.gcam-mvsdk/2.1.0.49/lib/arm64").join(LIB));
    let mut cfg = t.cfg();
    assert!(matches!(resolve_library(&cfg).unwrap().candidate, Candidate::SystemName(_)));
    cfg.arch_dir = "arm64".into();
    assert_eq!(resolved_path(&cfg), t.root.join("home/.gcam-mvsdk/2.1.0.49/lib/arm64").join(LIB));
}

#[test]
fn recorded_library_comes_before_system_loader() {
    let t = Tree::new();
    let dll = t.root.join("Program Files/HuaTengVision/SDK/X64/MVCAMSDK_X64.dll");
    touch(&dll);
    let mut cfg = t.cfg();
    cfg.library_name = "MVCAMSDK_X64.dll".into();
    cfg.recorded_library = Some(dll.clone());
    assert_eq!(resolved_path(&cfg), dll);
    let entries = search_entries(&cfg);
    assert!(matches!(entries.last().unwrap().candidate, Some(Candidate::SystemName(_))));
}

#[test]
fn recorded_library_is_read_from_manifest_json() {
    let t = Tree::new();
    let root = t.root.join("home/.gcam-mvsdk");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("manifest.json"), r#"{"library": "C:\\SDK\\MVCAMSDK_X64.dll"}"#).unwrap();
    assert_eq!(
        gcam_mvsdk_rs::read_recorded_library(&root),
        Some(PathBuf::from("C:\\SDK\\MVCAMSDK_X64.dll"))
    );
    assert_eq!(gcam_mvsdk_rs::read_recorded_library(&t.root), None);
}

#[test]
fn architecture_mapping() {
    assert_eq!(arch_dir_for(TargetOs::Linux, "x86_64", false), Some("x64"));
    assert_eq!(arch_dir_for(TargetOs::Linux, "x86", false), Some("x86"));
    assert_eq!(arch_dir_for(TargetOs::Linux, "aarch64", false), Some("arm64"));
    assert_eq!(arch_dir_for(TargetOs::Linux, "arm", true), Some("arm"));
    assert_eq!(arch_dir_for(TargetOs::Linux, "arm", false), Some("arm_softfp"));
    assert_eq!(arch_dir_for(TargetOs::Linux, "riscv64", false), None);
    assert_eq!(arch_dir_for(TargetOs::Windows, "x86_64", false), Some("x64"));
    assert_eq!(arch_dir_for(TargetOs::Windows, "x86", false), Some("x86"));
    assert_eq!(arch_dir_for(TargetOs::Windows, "aarch64", false), None);

    assert_eq!(library_file_name(TargetOs::Linux, "x64"), Some("libMVSDK.so"));
    assert_eq!(library_file_name(TargetOs::Linux, "arm64"), Some("libMVSDK.so"));
    assert_eq!(library_file_name(TargetOs::Windows, "x64"), Some("MVCAMSDK_X64.dll"));
    assert_eq!(library_file_name(TargetOs::Windows, "x86"), Some("MVCAMSDK.dll"));
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn host_mapping_on_linux_x86_64() {
    assert_eq!(gcam_mvsdk_rs::host_arch_dir(), Some("x64"));
    assert_eq!(gcam_mvsdk_rs::host_os(), TargetOs::Linux);
}

// ---------------------------------------------------------------------------
// GCAM_MVSDK_HOME, through SearchConfig::from_lookup (no process environment is modified)
// ---------------------------------------------------------------------------

fn cfg_from(vars: &[(&str, &Path)], cwd: &Path, user_home: &Path) -> SearchConfig {
    let mut map: HashMap<String, OsString> =
        vars.iter().map(|(k, v)| (k.to_string(), v.as_os_str().to_os_string())).collect();
    // Pin the architecture so the test does not depend on the host.
    map.entry(ENV_ARCH.to_string()).or_insert_with(|| OsString::from("x64"));
    SearchConfig::from_lookup(|name| map.get(name).cloned(), Some(cwd.to_path_buf()), Some(user_home.to_path_buf()))
        .unwrap()
}

fn lib_under(root: &Path, version: &str, cfg: &SearchConfig) -> PathBuf {
    root.join(version).join("lib/x64").join(&cfg.library_name)
}

#[test]
fn home_defaults_to_dot_gcam_mvsdk() {
    let t = Tree::new();
    let user_home = t.root.join("home");
    let cfg = cfg_from(&[], &t.root.join("checkout"), &user_home);
    assert_eq!(cfg.home_root, Some(user_home.join(".gcam-mvsdk")));
    assert_eq!(cfg.vendor_root, Some(t.root.join("checkout/vendor")));
    assert_eq!(cfg.arch_dir, "x64");
    assert_eq!(cfg.sdk_dir, None);
}

#[test]
fn empty_home_variable_means_default() {
    let t = Tree::new();
    let user_home = t.root.join("home");
    let cfg = cfg_from(&[(ENV_HOME, Path::new(""))], &t.root.join("checkout"), &user_home);
    assert_eq!(cfg.home_root, Some(user_home.join(".gcam-mvsdk")));
}

#[test]
fn gcam_mvsdk_home_replaces_the_default_home() {
    let t = Tree::new();
    let user_home = t.root.join("home");
    let sdk_home = t.root.join("elsewhere/sdk-home");
    let cfg = cfg_from(&[(ENV_HOME, sdk_home.as_path())], &t.root.join("checkout"), &user_home);
    assert_eq!(cfg.home_root, Some(sdk_home.clone()));

    // A library in the default home is not used while GCAM_MVSDK_HOME points elsewhere.
    touch(&lib_under(&user_home.join(".gcam-mvsdk"), "2.1.0.49", &cfg));
    assert!(matches!(resolve_library(&cfg).unwrap().candidate, Candidate::SystemName(_)));

    touch(&lib_under(&sdk_home, "2.1.0.49", &cfg));
    assert_eq!(resolved_path(&cfg), lib_under(&sdk_home, "2.1.0.49", &cfg));
}

#[test]
fn vendor_checkout_wins_over_gcam_mvsdk_home() {
    let t = Tree::new();
    let sdk_home = t.root.join("sdk-home");
    let cwd = t.root.join("checkout");
    let cfg = cfg_from(&[(ENV_HOME, sdk_home.as_path())], &cwd, &t.root.join("home"));
    touch(&lib_under(&sdk_home, "2.1.0.49", &cfg));
    touch(&lib_under(&cwd.join("vendor"), "2.1.0.49", &cfg));
    assert_eq!(resolved_path(&cfg), lib_under(&cwd.join("vendor"), "2.1.0.49", &cfg));
}

#[test]
fn gcam_mvsdk_dir_stays_exclusive_with_home_set() {
    let t = Tree::new();
    let sdk_home = t.root.join("sdk-home");
    let sdk_dir = t.root.join("explicit");
    let cfg = cfg_from(
        &[(ENV_HOME, sdk_home.as_path()), (ENV_SDK_DIR, sdk_dir.as_path())],
        &t.root.join("checkout"),
        &t.root.join("home"),
    );
    touch(&lib_under(&sdk_home, "2.1.0.49", &cfg));
    assert!(matches!(resolve_library(&cfg), Err(LoadError::NotFound { .. })));
    touch(&sdk_dir.join("lib/x64").join(&cfg.library_name));
    assert_eq!(resolved_path(&cfg), sdk_dir.join("lib/x64").join(&cfg.library_name));
}

#[test]
fn recorded_library_follows_gcam_mvsdk_home() {
    let t = Tree::new();
    let sdk_home = t.root.join("sdk-home");
    fs::create_dir_all(&sdk_home).unwrap();
    fs::write(sdk_home.join("manifest.json"), r#"{"library": "/opt/recorded/libMVSDK.so"}"#).unwrap();
    let cfg = cfg_from(&[(ENV_HOME, sdk_home.as_path())], &t.root.join("checkout"), &t.root.join("home"));
    assert_eq!(cfg.recorded_library, Some(PathBuf::from("/opt/recorded/libMVSDK.so")));
}
