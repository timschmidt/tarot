#!/usr/bin/env python3
"""Manual real-libinput regression on the verified spare PinePhone Pro.

Run as root ONLY while phone-drm-smoke.sh owns VT8. No system configuration
or permissions are changed. The temporary uinput touchscreen is destroyed on exit.
"""
import argparse
import fcntl
import json
import os
from pathlib import Path
import socket
import struct
import time


def request(control_path, command, **fields):
    with socket.socket(socket.AF_UNIX) as stream:
        stream.settimeout(3)
        stream.connect(str(control_path))
        stream.sendall((json.dumps(dict(command=command, **fields)) + "\n").encode())
        response = json.loads(stream.makefile().readline())
    assert response["ok"], response
    return response


class Touchscreen:
    def __init__(self, guard):
        self.guard = guard
        self.fd = os.open("/dev/uinput", os.O_WRONLY | os.O_NONBLOCK)
        self.active = {}
        self.serial = 1
        self.created = False
        try:
            # These Linux ABI definitions match linux/uinput.h in the phone SDK.
            iow = lambda number, size: (1 << 30) | (size << 16) | (ord("U") << 8) | number
            for event in (1, 3):
                fcntl.ioctl(self.fd, iow(100, 4), event)  # EV_KEY, EV_ABS
            fcntl.ioctl(self.fd, iow(101, 4), 0x14A)  # BTN_TOUCH
            fcntl.ioctl(self.fd, iow(110, 4), 1)  # INPUT_PROP_DIRECT
            axes = ((0, 719), (1, 1439), (0x2F, 1), (0x35, 719),
                    (0x36, 1439), (0x39, 65535))
            for axis, maximum in axes:
                fcntl.ioctl(self.fd, iow(103, 4), axis)
                # uinput_abs_setup: u16 code, padding, input_absinfo (six i32s).
                info = struct.pack("@H2xiiiiii", axis, 0, 0, maximum, 0, 0,
                                   10 if axis in (0, 1, 0x35, 0x36) else 0)
                fcntl.ioctl(self.fd, iow(4, len(info)), info)
            setup = struct.pack("@HHHH80sI", 0x06, 0x1, 0x1, 1,
                                b"Tarot development gesture replay", 0)
            fcntl.ioctl(self.fd, iow(3, len(setup)), setup)
            fcntl.ioctl(self.fd, 0x5501)  # UI_DEV_CREATE
            self.created = True
            time.sleep(0.8)  # Allow udev and the compositor's libinput monitor to discover it.
        except BaseException:
            self.close()
            raise

    def event(self, kind, code, value):
        os.write(self.fd, struct.pack("@llHHi", 0, 0, kind, code, value))

    def syn(self):
        self.event(0, 0, 0)

    def position(self, slot, x, y):
        self.event(3, 0x2F, slot)
        self.event(3, 0x35, round(x * 2))
        self.event(3, 0x36, round(y * 2))
        if slot == 0:
            self.event(3, 0, round(x * 2))
            self.event(3, 1, round(y * 2))

    def down(self, x, y, slot=0):
        self.guard()
        assert slot not in self.active
        self.event(3, 0x2F, slot)
        self.event(3, 0x39, self.serial)
        self.serial += 1
        self.position(slot, x, y)
        self.active[slot] = (x, y)
        self.event(1, 0x14A, 1)
        self.syn()
        time.sleep(0.035)

    def move(self, x, y, slot=0, delay=0.035):
        self.guard()
        assert slot in self.active
        self.position(slot, x, y)
        self.active[slot] = (x, y)
        self.syn()
        time.sleep(delay)

    def up(self, slot=0):
        self.guard()
        assert slot in self.active
        self.event(3, 0x2F, slot)
        self.event(3, 0x39, -1)
        del self.active[slot]
        self.event(1, 0x14A, int(bool(self.active)))
        self.syn()
        time.sleep(0.04)

    def close(self):
        if self.created:
            fcntl.ioctl(self.fd, 0x5502)  # UI_DEV_DESTROY also terminates active contacts.
            self.created = False
        os.close(self.fd)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--socket", type=Path, default=Path("/run/user/10000/tarot-drm.sock"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    assert os.geteuid() == 0, "Use an authenticated administrator terminal for /dev/uinput"
    assert "PinePhone Pro" in Path("/proc/device-tree/model").read_text()
    initial = request(args.socket, "status")
    pid = initial["pid"]
    assert initial["render_metrics"]["drm_presentations"] > 0, "Need the active direct backend"
    args.output.mkdir(parents=True, exist_ok=True)
    owner = args.socket.stat()
    os.chown(args.output, owner.st_uid, owner.st_gid)
    records = {}

    def guard():
        assert Path("/sys/class/tty/tty0/active").read_text().strip() == "tty8", "Tarot no longer owns VT8"
        assert request(args.socket, "status")["pid"] == pid, "The tested session ended"

    def status(label):
        result = request(args.socket, "status")
        records[label] = result
        return result

    def capture(name):
        request(args.socket, "capture", path=str(args.output / (name + ".png")))
        for _ in range(30):
            result = request(args.socket, "status")
            if not result["capture_pending"]:
                assert not result["capture_error"], result["capture_error"]
                return
            time.sleep(0.02)
        raise AssertionError("capture did not finish")

    def settle():
        time.sleep(0.4)

    def visual(result, window):
        return next(item for item in result["presentation"]["windows"] if item["id"] == window)

    guard()
    first = initial["cards"][0]["id"]
    request(args.socket, "activate", id=first)
    settle()
    screen = Touchscreen(guard)
    try:
        # App -> halfway overview -> commit. The model stays App until release.
        full = visual(status("focused"), first)["rect"]
        screen.down(180, 710)
        screen.move(180, 670)
        screen.move(180, 630)
        half = status("half-overview")
        assert half["view"] == {"App": first}, half
        rect = visual(half, first)["rect"]
        assert rect["height"] < full["height"] and rect["height"] > 450, half
        assert 0.2 < half["presentation"]["overview"] < 0.8, half
        capture("gesture-overview-half")
        screen.up()
        settle()
        assert status("overview-committed")["view"] == "Overview"

        # Below-threshold vertical drag must return without activating a card.
        before = visual(status("before-short-drag"), first)["rect"]
        screen.down(180, 300)
        screen.move(180, 270, delay=0.25)
        screen.up()
        settle()
        returned = status("short-drag-returned")
        assert returned["view"] == "Overview", returned
        assert visual(returned, first)["rect"] == before, returned

        # A second contact in app content cancels a strip-owned gesture.
        request(args.socket, "activate", id=first)
        settle()
        screen.down(180, 710)
        screen.move(180, 630)
        screen.down(100, 500, slot=1)
        screen.up(slot=1)
        screen.up()
        settle()
        canceled = status("multitouch-canceled")
        assert canceled["view"] == {"App": first}, canceled
        assert visual(canceled, first)["rect"] == full, canceled
        capture("gesture-multitouch-return")

        # Launcher content scroll stays in Launcher and cannot launch on release.
        request(args.socket, "launcher")
        settle()
        count = len(status("before-launcher-scroll")["cards"])
        screen.down(180, 500)
        for y in (450, 400, 350, 300, 250):
            screen.move(180, y)
        screen.up()
        settle()
        scrolled = status("launcher-scrolled")
        assert scrolled["view"] == "Launcher", scrolled
        assert len(scrolled["cards"]) == count, scrolled
        capture("gesture-launcher-scrolled")

        # A real terminal gives the overview two independent live clients.
        request(args.socket, "launch", id="foot.desktop")
        for _ in range(100):
            current = request(args.socket, "status")
            if len(current["cards"]) > count:
                break
            time.sleep(0.1)
        else:
            raise AssertionError("foot did not map")
        terminal = current["selected"]
        request(args.socket, "overview")
        settle()
        screen.down(180, 300)
        screen.move(240, 300)
        screen.move(300, 300)
        capture("gesture-browse-half")
        screen.move(350, 300)
        screen.up()
        settle()
        browsed = status("browsed-previous")
        assert browsed["view"] == "Overview" and browsed["selected"] == first, browsed
        capture("gesture-browse-settled")

        # Close goes through xdg_toplevel.close, leaving the unrelated terminal alive.
        screen.down(180, 300)
        for y in (260, 220, 180, 140, 100):
            screen.move(180, y)
        capture("gesture-dismiss-held")
        screen.up()
        for _ in range(50):
            current = request(args.socket, "status")
            if all(card["id"] != first for card in current["cards"]):
                break
            time.sleep(0.05)
        else:
            raise AssertionError("GTK client did not honor close")
        settle()
        dismissed = status("dismissed")
        assert any(card["id"] == terminal for card in dismissed["cards"]), dismissed
        capture("gesture-dismissed")
        time.sleep(0.5)
        status("settled-idle")
    finally:
        screen.close()
        (args.output / "gesture-replay.json").write_text(json.dumps(records, indent=2) + "\n")
    print("PASS: real libinput overview, cancellation, launcher scroll, browsing and cooperative dismissal")


if __name__ == "__main__":
    main()
