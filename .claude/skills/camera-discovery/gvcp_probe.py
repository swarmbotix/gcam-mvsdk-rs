#!/usr/bin/env python3
"""Send one GigE Vision discovery broadcast and print every camera that answers.

Needs no privileges and no SDK. Usage: python3 -I gvcp_probe.py [broadcast-address ...] [--wait SECONDS]
Default broadcast address: 255.255.255.255. Add subnet broadcasts (e.g. 192.168.1.255) when the
host has several NICs.
"""
import socket
import struct
import sys
import time


def main(argv):
    wait = 2.0
    targets = []
    args = list(argv)
    while args:
        a = args.pop(0)
        if a == "--wait" and args:
            wait = float(args.pop(0))
        else:
            targets.append(a)
    targets = targets or ["255.255.255.255"]

    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
    s.bind(("0.0.0.0", 0))
    s.settimeout(0.25)
    # GVCP header: key 0x42, flags 0x11 (ack required, broadcast ack allowed), DISCOVERY_CMD 0x0002.
    packet = struct.pack(">BBHHH", 0x42, 0x11, 0x0002, 0x0000, 0x0001)
    for t in targets:
        s.sendto(packet, (t, 3956))

    seen = {}
    end = time.time() + wait
    while time.time() < end:
        try:
            data, addr = s.recvfrom(2048)
        except socket.timeout:
            continue
        if len(data) < 8 + 0xF8 or data[2:4] != b"\x00\x03":
            continue
        p = data[8:]

        def text(lo, hi):
            return p[lo:hi].split(b"\0")[0].decode("ascii", "replace")

        mac = ":".join(f"{b:02x}" for b in p[0x0A:0x10])
        seen[mac] = {
            "ip": socket.inet_ntoa(p[0x24:0x28]),
            "mask": socket.inet_ntoa(p[0x34:0x38]),
            "model": text(0x68, 0x88),
            "serial": text(0xD8, 0xE8),
            "user_name": text(0xE8, 0xF8),
            "reply_from": addr[0],
        }
    for mac, info in sorted(seen.items()):
        print(mac, " ".join(f"{k}={v}" for k, v in info.items()))
    print(f"{len(seen)} camera(s) answered")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
