---
name: camera-rename
description: Give a HuaTeng camera a user-set name (DeviceUserID) with gcam-mvsdk-rs rename, selected by serial number, and verify it. Use when the user asks to name, rename or label a camera, or to make cameras selectable by name (sb00, sb01, ...). Writes to the camera's non-volatile memory, so only on the serial number the user named.
---

# camera-rename

## When to use

- The user wants to name a new camera, change a camera's name, or label cameras by position.
- Programs should select cameras by a stable name instead of the serial number.

## Prerequisites

- camera-list works and shows the camera (with its serial number).
- No other program has the camera open (publisher, viewer, container, vendor GUI).
- GigE: `CAP_NET_RAW` or `sudo -E`, as for `list`.

## Steps

1. Find the serial number and the current name:
   ```bash
   gcam-mvsdk-rs list
   ```
2. Confirm with the user: the exact serial number and the new name. Check that no other camera
   already carries that name (the `User-set name` lines of `list`).
3. Write the name:
   ```bash
   gcam-mvsdk-rs rename 056090423003 sb00
   ```
4. Verify:
   ```bash
   gcam-mvsdk-rs list               # the User-set name line shows sb00
   gcam-mvsdk-rs info sb00          # selection by name works
   ```

Library equivalent:

```rust
let cam = sdk.open(&sdk.find_device("056090423003")?)?;
println!("current name: {:?}", cam.user_name()?);
cam.set_user_name("sb00")?; // validated, then written to the camera's non-volatile memory
```

## Name rules

- At most 31 bytes. Use ASCII letters, digits, `-` and `_`. The tool encodes other characters as
  GBK and counts bytes, but other tools may not display them.
- None of `\ / : * ? " < > |` and no control characters (the tool rejects them before writing).
- Unique on the network: the tool does not enforce it. With duplicates, selection by name returns
  the first match, so use the serial number until the duplicate is fixed.

## Reading the output

```text
SN 056090423003: user-set name "" -> "sb00"
The name is stored in the camera's non-volatile memory and survives power cycles.
```

A line `warning: the camera reports ... after writing ...` means the read-back differs; run
`list` again and, if it persists, power-cycle the camera and check once more.

## If it fails

| Output | Cause and fix |
|---|---|
| `invalid argument: name ...` | Breaks a name rule above; choose another name. Nothing was written. |
| `no camera matches "<SN>"; available: ...` | Wrong serial number; copy it from `list`. `rename` accepts only serial numbers. |
| Status `-13`, `32774` (0x8006) or `-45` | The camera is held by another program or computer. Stop the publisher or viewer (for example `docker ps`, `pgrep -a gige_ht`), wait a few seconds for the heartbeat to expire, then retry. |
| `No cameras found` | camera-discovery. |

## Safety

- Never rename a camera the user did not name explicitly by serial number. Do not infer the
  serial from a position in the list.
- Confirm the serial number and the new name with the user before running `rename`.
- Renaming changes what running programs and launch scripts select; tell the user which
  configurations refer to the old name.
