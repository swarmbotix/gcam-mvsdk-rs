---
name: hardware-test
description: Run the gcam-mvsdk-rs hardware integration tests (GCAM_MVSDK_HW_TEST=1) safely on the bench machine with real HuaTeng cameras, and interpret each test's expectations and failures. Use when validating FFI signatures, a release, or camera behaviour on real hardware.
---

# hardware-test

The tests in `tests/hw/main.rs` return immediately unless `GCAM_MVSDK_HW_TEST=1`. They need the
SDK (`./vendor/`, the SDK home `$GCAM_MVSDK_HOME` or `~/.gcam-mvsdk/`, or `GCAM_MVSDK_DIR`) and
powered cameras on the LAN. They run as a normal user; GigE discovery needs no privileges.

## 1. Preconditions

```bash
gcam-mvsdk-rs doctor            # SDK found, rp_filter 0, camera subnet OK
gcam-mvsdk-rs list              # the cameras you expect are listed
```

Stop any publisher using the cameras (`pgrep -a gige_ht`, `docker ps`): an open camera makes
the tests fail with status -45, -13 or 32774.

## 2. Run

```bash
bash .claude/skills/hardware-test/run-hw.sh                     # read-only tests
GCAM_MVSDK_HW_EXPECT=sb00,sb01,sb02,sb03 bash .claude/skills/hardware-test/run-hw.sh
```

`run-hw.sh` builds the test binary and runs it as the current user with `--test-threads=1` and
`--nocapture`. Nothing needs `sudo`; do not run `sudo cargo test` (it would leave `target/`
root-owned).

Optional variables:

| Variable | Effect |
|---|---|
| `GCAM_MVSDK_HW_EXPECT=sb00,sb01` | user-set names that must be present |
| `GCAM_MVSDK_HW_CAMERA=sb00` | camera for the grab test (default: first enumerated) |
| `GCAM_MVSDK_HW_RENAME_SN=<serial>` | enables the rename round trip on that camera (writes NVM) |
| `GIGE_MTU=1500` | packet-size limit for the grab test |

Only set `GCAM_MVSDK_HW_RENAME_SN` when the user agrees: it writes a temporary name
(`hwtestNNNN`) to the camera and then restores the original.

## 3. What each test expects

| Test | Expects | Typical failure |
|---|---|---|
| `sdk_loads_and_reports_version` | SDK loads, version starts with `2.` | SDK not found: sdk-setup skill |
| `enumerate_finds_expected_cameras` | at least one camera, every serial non-empty, `EXPECT` names present | 0 cameras: network (camera-discovery skill) |
| `gige_ip_and_mac_are_readable` | each GigE camera has an IP, a parsable MAC, and is in its host NIC subnet | wrong subnet: `setip` |
| `rename_round_trip` | writes and reads back a temporary name, then restores the original | skipped without `GCAM_MVSDK_HW_RENAME_SN` |
| `grab_produces_mono8_frame` | a Mono8 frame within 5 x 3 s, size equal to the current resolution, `width*height` bytes | time-outs: MTU (`GIGE_MTU=1500`), firewall, rp_filter |

A failure in these tests after an FFI change usually means a wrong signature or struct layout:
compare with the header and re-measure with `.claude/skills/add-sdk-function/sizeof.sh`.

## 4. Report

Quote the test output (pass/fail per test, camera serials, frame size). If cameras were not
reachable, say so; do not mark hardware behaviour as verified.
