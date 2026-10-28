#!/usr/bin/env python3
"""Synthetic input-method replay in GTK4's empty entry_undo demo on temporary VT8.

Launch gtk4-demo --run=entry_undo in the bounded TAROT_TEXT_PROBE=1 session first.
The helper report intentionally contains fixture text; use no personal fields.
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
    parser.add_argument("--method-report", type=Path, default=Path("/home/user/tarot-dev/artifacts/text-method.jsonl"))
    parser.add_argument("--method-commands", type=Path, default=Path("/home/user/tarot-dev/artifacts/text-method.commands"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    assert os.geteuid() == 0 and "PinePhone Pro" in Path("/proc/device-tree/model").read_text()
    initial = request(args.socket, "status")
    cards = [card for card in initial["cards"] if card["app_id"] == "gtk4-demo" and card["title"] == "Undo and Redo"]
    assert len(cards) == 1, initial
    window = cards[0]["id"]
    assert initial["view"] == {"App": window}
    owner = args.socket.stat()
    args.output.mkdir(parents=True, exist_ok=True)
    os.chown(args.output, owner.st_uid, owner.st_gid)

    def events():
        return [json.loads(line) for line in args.method_report.read_bytes().split(b"\n")[:-1] if line]

    def await_event(start, kind, **fields):
        for _ in range(200):
            found = [event for event in events()[start:] if event["event"] == kind and all(event.get(k) == v for k, v in fields.items())]
            if found:
                return found[-1]
            time.sleep(0.02)
        raise AssertionError((kind, fields, events()[start:]))

    def send(command, **fields):
        with args.method_commands.open("a") as stream:
            stream.write(json.dumps(dict(command=command, **fields)) + "\n")
        time.sleep(0.12)

    def guard():
        assert Path("/sys/class/tty/tty0/active").read_text().strip() == "tty8"
        assert request(args.socket, "status")["pid"] == initial["pid"]

    def capture(name):
        request(args.socket, "capture", path=str(args.output / (name + ".png")))
        for _ in range(100):
            state = request(args.socket, "status")
            if not state["capture_pending"]:
                assert not state["capture_error"]
                return
            time.sleep(.02)
        raise AssertionError("Capture did not finish")

    assert [event for event in events() if event["event"] == "surrounding"][-1]["text"] == "", "Use a fresh empty fixture"
    screen = Touchscreen(guard)
    try:
        # The inspected demo is centered and fits at native size: entry center.
        screen.down(150, 387)
        screen.up()
        send("preedit", text="café")
        capture("gtk4-preedit")
        start = len(events())
        send("commit", text="café 🙂")
        await_event(start, "surrounding", text="café 🙂", cursor=len("café 🙂".encode()))
        capture("gtk4-committed")
        start = len(events())
        request(args.socket, "overview")
        await_event(start, "deactivate")
        time.sleep(.12)
        send("commit", text="BACKGROUND")
        start = len(events())
        request(args.socket, "activate", id=window)
        await_event(start, "activate")
        await_event(start, "surrounding", text="café 🙂")
        time.sleep(.3)
        capture("gtk4-restored")
        print("PASS: actual GTK4 touch focus, preedit, Unicode commit and navigation isolation")
    finally:
        screen.close()
        (args.output / "gtk4-text-status.json").write_text(json.dumps(request(args.socket, "status"), indent=2) + "\n")


if __name__ == "__main__":
    main()
