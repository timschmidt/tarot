#!/usr/bin/env python3
"""Real GTK4 application-menu smoke test inside the temporary phone session."""
import argparse
import json
import os
from pathlib import Path
import time
from phone_gesture_replay import Touchscreen, request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--socket", type=Path, default=Path("/run/user/10000/tarot-drm.sock"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    assert os.geteuid() == 0
    assert "PinePhone Pro" in Path("/proc/device-tree/model").read_text()
    initial = request(args.socket, "status")
    assert initial["render_metrics"]["drm_presentations"] > 0
    owner = args.socket.stat()
    args.output.mkdir(parents=True, exist_ok=True)
    os.chown(args.output, owner.st_uid, owner.st_gid)
    records = {}

    def guard():
        assert Path("/sys/class/tty/tty0/active").read_text().strip() == "tty8"
        assert request(args.socket, "status")["pid"] == initial["pid"]

    def await_grab(label, active):
        for _ in range(150):
            result = request(args.socket, "status")
            if result["popup_grab_active"] == active:
                records[label] = result
                return result
            time.sleep(0.02)
        capture(label + "-timeout")
        raise AssertionError((label, result))

    def capture(name):
        request(args.socket, "capture", path=str(args.output / (name + ".png")))
        for _ in range(100):
            result = request(args.socket, "status")
            if not result["capture_pending"]:
                assert not result["capture_error"]
                return
            time.sleep(0.01)
        raise AssertionError("Capture did not finish")

    request(args.socket, "launch", id="org.gnome.Calculator.desktop")
    for _ in range(200):
        result = request(args.socket, "status")
        cards = [card for card in result["cards"] if card["app_id"] == "org.gnome.Calculator"]
        if cards:
            calculator = cards[0]["id"]
            break
        time.sleep(0.1)
    else:
        raise AssertionError("Calculator did not map")
    request(args.socket, "activate", id=calculator)
    time.sleep(0.5)
    screen = Touchscreen(guard)
    try:
        def tap(x, y):
            screen.down(x, y)
            screen.up()
            time.sleep(0.15)

        capture("gtk4-before-menu")
        tap(337, 51)  # Current Calculator 50.0 touch header, scale-2 phone capture.
        opened = await_grab("gtk4-menu-open", True)
        assert opened["popup_owner"] == calculator, opened
        # A grab can precede the first popup buffer. Let GTK complete its
        # configure/render cycle before capturing the visible menu.
        time.sleep(0.3)
        capture("gtk4-menu-open")
        tap(30, 400)
        closed = await_grab("gtk4-menu-closed", False)
        assert closed["view"] == {"App": calculator}, closed
        capture("gtk4-menu-closed")
        tap(337, 51)
        await_grab("gtk4-menu-reopened", True)
        request(args.socket, "overview")
        overview = await_grab("gtk4-navigation-dismissed", False)
        assert overview["view"] == "Overview", overview
        time.sleep(0.35)
        capture("gtk4-menu-overview")
        print("PASS: real GTK4 menu grab, outside dismissal, reopening and overview dismissal")
    finally:
        screen.close()
        (args.output / "gtk4-popup-status.json").write_text(json.dumps(records, indent=2) + "\n")


if __name__ == "__main__":
    main()
