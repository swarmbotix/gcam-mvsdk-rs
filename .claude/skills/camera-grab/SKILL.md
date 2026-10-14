---
name: camera-grab
description: Save frames from one HuaTeng camera as PNG with gcam-mvsdk-rs grab, check exposure, and fix time-outs with --mtu. Use when the user wants test images, a quick visual check of focus or exposure, or a smoke test after installation or after changing network settings.
---

# camera-grab

## When to use

- Smoke test after `install.bash`, `setip`, a new cable or a new switch.
- Check focus, framing or exposure with a few still images.
- Collect sample images for a detector.

## Prerequisites

- camera-list shows the camera and no WARNING for its subnet.
- No other program streams from the camera (stop publishers first).
- GigE, for full frame rate: the `rmem_max` fix from the README.

## Steps

```bash
gcam-mvsdk-rs grab sb00                                  # one frame into the current directory
gcam-mvsdk-rs grab sb00 -n 5 -o /tmp/frames              # five frames into /tmp/frames
gcam-mvsdk-rs grab sb00 -n 3 -o /tmp/frames --exposure-us 5000   # fixed exposure, auto exposure off
gcam-mvsdk-rs grab sb00 -n 3 -o /tmp/frames --mtu 1500   # packet size for a 1500-byte MTU NIC
gcam-mvsdk-rs grab sb00 --timeout-ms 5000                # wait longer per frame (default 2000)
```

Options (from `gcam-mvsdk-rs grab --help`): `-n/--count N` (default 1), `-o/--out DIR` (default
`.`), `--exposure-us US`, `--timeout-ms MS`, `--mtu BYTES` (default `$GIGE_MTU`, otherwise the
camera's current packet size is kept).

Smoke test after setup:

```bash
gcam-mvsdk-rs grab sb00 -n 1 -o /tmp/smoke && ls -l /tmp/smoke
```

## Reading the output

```text
Camera "sb00" SN 056090423003: mono sensor, max 1280x1024, exposure 5000.0 us
saved /tmp/frames/sb00_0000.png  1280x1024  raw 1310720 bytes  ts 1234.5 ms  exposure 5000 us  mean 87.3
```

- Files are named `<user-set name>_NNNN.png` (the serial number when the camera has no name;
  characters other than letters, digits, `-`, `_` and `.` become `_`).
- Mono sensors produce 8-bit grayscale PNG (Mono8); color sensors produce 8-bit RGB PNG.
- `mean` is the average pixel value (0 to 255): near 255 the image is saturated (lower the
  exposure), near 0 it is too dark (raise exposure or gain). Open the files with any image viewer,
  for example `xdg-open /tmp/frames/sb00_0000.png`.
- `ts` is the camera's own time stamp (0.1 ms resolution); differences between frames show the
  actual frame interval.
- `GigE packet size: 1500 bytes (limit 1500)` appears when `--mtu` or `$GIGE_MTU` is used.

## If it fails

| Output | Cause and fix |
|---|---|
| `timeout waiting for a frame (1/3)` ... `No frames received.` | Usually the packet size exceeds the NIC MTU: add `--mtu 1500` (or enable jumbo frames on the NIC and use `--mtu 9000`). Then check the firewall and `rp_filter` (`gcam-mvsdk-rs doctor`). |
| `no camera matches "sb00"` | Wrong name; use the serial number from `list`, see camera-list. |
| Status `-13`, `32774` or `-45` | Another program holds the camera; stop it and retry. |
| `invalid argument: --count must be at least 1` | Use `-n 1` or more. |
| Image all black or all white | Exposure; retry with `--exposure-us` (see `gcam-mvsdk-rs info sb00` for the range). |

## Safety

`grab` changes the camera's settings for the session: trigger mode continuous, ISP output format
(Mono8 or RGB8), and, when given, exposure (automatic exposure off) and packet size. It does not
save them to a parameter group. Do not run it on a camera a production publisher depends on
without stopping that publisher first; the publisher sets its own values when it restarts.
