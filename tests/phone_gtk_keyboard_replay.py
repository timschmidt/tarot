#!/usr/bin/env python3
"""Capture actual GTK4 typing with the native keyboard; inspect resulting text.

Launch gtk4-demo --run=entry_undo in a TAROT_KEYBOARD=1 bounded VT8 session.
Machine assertions cover focus/geometry/lifecycle; captures establish visible text.
"""
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
    assert os.geteuid() == 0 and "PinePhone Pro" in Path("/proc/device-tree/model").read_text()
    initial = request(args.socket, "status")
    owner = args.socket.stat()
    args.output.mkdir(parents=True, exist_ok=True)
    os.chown(args.output, owner.st_uid, owner.st_gid)
    records = {}

    def await_keyboard(label, visible):
        for _ in range(400):
            result = request(args.socket, "status")
            matching = [card for card in result["cards"] if card["app_id"] == "gtk4-demo" and card["title"] == "Undo and Redo"]
            if matching and result["keyboard_visible"] == visible:
                records[label] = result
                return result, matching[0]["id"]
            time.sleep(.03)
        raise AssertionError((label, result))

    def guard():
        assert Path("/sys/class/tty/tty0/active").read_text().strip() == "tty8"
        assert request(args.socket, "status")["pid"] == initial["pid"]

    def tap(x, y):
        screen.down(x, y)
        screen.up()
        time.sleep(.15)

    def capture(name):
        request(args.socket, "capture", path=str(args.output / (name + ".png")))
        for _ in range(100):
            state = request(args.socket, "status")
            if not state["capture_pending"]:
                assert not state["capture_error"]
                return
            time.sleep(.02)
        raise AssertionError("Capture did not finish")

    opened, window = await_keyboard("opened", True)
    assert opened["view"] == {"App": window} and opened["app_area"]["height"] == 424
    time.sleep(.4)
    screen = Touchscreen(guard)
    try:
        capture("gtk4-keyboard-empty")
        tap(21, 504)
        tap(30, 663)
        tap(69, 609)
        tap(104, 609)
        assert request(args.socket, "status")["view"] == {"App": window}
        capture("gtk4-keyboard-q-accent-emoji")
        tap(330, 609)
        capture("gtk4-keyboard-after-backspace")
        tap(180, 710)
        overview, _ = await_keyboard("overview", False)
        assert overview["view"] == "Overview" and overview["app_area"]["height"] == 664
        request(args.socket, "activate", id=window)
        returned, _ = await_keyboard("returned", True)
        assert returned["view"] == {"App": window}
        time.sleep(.4)
        tap(56, 504)
        capture("gtk4-keyboard-restored-q-accent-w")
        print("Captured GTK4 native keyboard typing and deletion; inspect qé🙂, qé and qéw in the images")
    finally:
        screen.close()
        (args.output / "gtk4-keyboard-status.json").write_text(json.dumps(records, indent=2) + "\n")


if __name__ == "__main__":
    main()
