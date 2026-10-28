#!/usr/bin/env python3
"""Native Rust/egui keyboard replay on the bounded verified VT8 phone session."""
import argparse
import json
import os
from pathlib import Path
import pwd
import signal
import subprocess
import time
from phone_gesture_replay import Touchscreen, request
from phone_touch_keyboard import Keyboard


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--socket", type=Path, default=Path("/run/user/10000/tarot-drm.sock"))
    parser.add_argument("--probe", type=Path, default=Path("/home/user/tarot-dev/bin/text-probe"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    assert os.geteuid() == 0 and "PinePhone Pro" in Path("/proc/device-tree/model").read_text()
    initial = request(args.socket, "status")
    owner = args.socket.stat()
    account = pwd.getpwuid(owner.st_uid)
    args.output.mkdir(parents=True, exist_ok=True)
    os.chown(args.output, owner.st_uid, owner.st_gid)
    report, commands = args.output / "app-events.jsonl", args.output / "app-commands.jsonl"
    report.unlink(missing_ok=True)
    commands.write_text("")
    os.chown(commands, owner.st_uid, owner.st_gid)
    records = {}

    def events():
        if not report.exists():
            return []
        return [json.loads(line) for line in report.read_bytes().split(b"\n")[:-1] if line]

    def await_text(start, text):
        for _ in range(150):
            found = [event for event in events()[start:] if event["event"] == "applied" and event["text"] == text]
            if found:
                return found[-1]
            time.sleep(.02)
        raise AssertionError(("Text not received", text, events()[start:]))

    def status(label):
        result = request(args.socket, "status")
        records[label] = result
        return result

    def await_keyboard(label, visible):
        for _ in range(150):
            result = status(label)
            if result["keyboard_visible"] == visible:
                if label == "opened" and not any(card["app_id"] == "org.tarot.TextProbe" and result["view"] == {"App": card["id"]} for card in result["cards"]):
                    time.sleep(.02)
                    continue
                return result
            time.sleep(.02)
        raise AssertionError((label, result))

    def capture(name):
        request(args.socket, "capture", path=str(args.output / (name + ".png")))
        for _ in range(100):
            state = request(args.socket, "status")
            if not state["capture_pending"]:
                assert not state["capture_error"]
                return
            time.sleep(.02)
        raise AssertionError("Capture did not finish")

    def guard():
        assert Path("/sys/class/tty/tty0/active").read_text().strip() == "tty8"
        assert request(args.socket, "status")["pid"] == initial["pid"]

    def tap(x, y):
        screen.down(x, y)
        screen.up()
        time.sleep(.1)

    environment = dict(os.environ, WAYLAND_DISPLAY=initial["wayland_display"], XDG_RUNTIME_DIR=str(args.socket.parent))
    environment.pop("WAYLAND_SOCKET", None)
    environment.pop("DISPLAY", None)
    with (args.output / "client.log").open("w") as log:
        client = subprocess.Popen([str(args.probe), "--role", "app", "--report", str(report), "--commands", str(commands)],
                                  stdin=subprocess.DEVNULL, stdout=log, stderr=log, env=environment, cwd=account.pw_dir,
                                  user=owner.st_uid, group=owner.st_gid, extra_groups=os.getgrouplist(account.pw_name, owner.st_gid))
        screen = None
        try:
            opened = await_keyboard("opened", True)
            assert len(opened["cards"]) == len(initial["cards"]) + 1, opened
            window = next(card["id"] for card in opened["cards"] if card["app_id"] == "org.tarot.TextProbe")
            assert opened["view"] == {"App": window}
            area, keyboard = opened["app_area"], opened["keyboard_rect"]
            assert area["height"] == 424 and keyboard == {"x": 0.0, "y": 452.0, "width": 360.0, "height": 240.0}, opened
            assert area["y"] + area["height"] == keyboard["y"]
            time.sleep(.4)
            capture("keyboard-open")
            screen = Touchscreen(guard)
            start = len(events())
            tap(21, 504)  # Q: first row in the inspected scale-2 keyboard.
            await_text(start, "hello q")
            start = len(events())
            tap(330, 609)  # Backspace.
            await_text(start, "hello ")
            tap(30, 663)  # Symbols.
            start = len(events())
            tap(69, 609)  # é: first symbol key after Shift.
            await_text(start, "hello é")
            start = len(events())
            tap(104, 609)  # Smiley: actual committed character is Unicode.
            await_text(start, "hello é🙂")
            capture("keyboard-unicode")
            start = len(events())
            tap(330, 609)
            await_text(start, "hello é")

            # Two simultaneous contacts cancel the pending key rather than type.
            before = len(events())
            screen.down(69, 609)
            screen.down(104, 609, slot=1)
            screen.up(slot=1)
            screen.up()
            time.sleep(.2)
            assert not [event for event in events()[before:] if event["event"] == "applied"]
            assert status("typed")["view"] == {"App": window}

            tap(180, 710)  # The shell strip remains accessible below the keyboard.
            hidden = await_keyboard("overview", False)
            assert hidden["view"] == "Overview" and hidden["app_area"]["height"] == 664, hidden
            time.sleep(.3)
            capture("keyboard-overview")
            request(args.socket, "activate", id=window)
            await_keyboard("returned", True)
            time.sleep(.4)
            start = len(events())
            tap(21, 504)
            await_text(start, "hello éq")
            start = len(events())
            tap(245, 663)  # Enter travels through the restricted virtual keyboard.
            for _ in range(100):
                keys = [event["state"] for event in events()[start:] if event["event"] == "key" and event["key"] == 28]
                if keys == [1, 0]:
                    break
                time.sleep(.02)
            assert keys == [1, 0], events()[start:]
            capture("keyboard-returned")
            tap(320, 663)  # Hide without moving the application's keyboard focus.
            hidden = await_keyboard("manually-hidden", False)
            assert hidden["view"] == {"App": window} and hidden["app_area"]["height"] == 664

            tap(320, 710)  # Reopen the same native field using the shell control.
            await_keyboard("reopened-same-field", True)
            time.sleep(.3)
            previous = status("before-restart")["input_method"]
            children = Path(f'/proc/{initial["pid"]}/task/{initial["pid"]}/children').read_text().split()
            candidates = [int(pid) for pid in children if Path(f"/proc/{pid}/exe").resolve() == Path("/home/user/tarot-dev/bin/tarot-keyboard")]
            assert len(candidates) == 1, candidates
            assert candidates[0] == previous["pid"]
            start = len(events())
            screen.down(21, 504)  # A held key must not type after its helper dies.
            os.kill(candidates[0], signal.SIGKILL)
            for _ in range(300):
                recovered = status("helper-restarted")
                if recovered["input_method"]["generation"] > previous["generation"] and recovered["keyboard_visible"]:
                    break
                time.sleep(.02)
            else:
                raise AssertionError(("Keyboard did not recover", recovered))
            assert recovered["pid"] == initial["pid"] and recovered["view"] == {"App": window}
            assert recovered["input_method"]["pid"] != previous["pid"] and client.poll() is None
            screen.up()
            time.sleep(.1)
            assert not [event for event in events()[start:] if event["event"] == "applied"]
            start = len(events())
            tap(21, 504)
            await_text(start, "hello éqq")
            capture("keyboard-recovered")
            printable=''.join(chr(code) for code in range(0x20,0x7f))
            start=len(events());Keyboard(tap).type_ascii(printable)
            await_text(start,"hello éqq"+printable)
            status("all-printable-ascii-committed");capture("keyboard-ascii-page")
            print("PASS: native keyboard typing, all printable ASCII, Unicode, cancellation, Enter, same-field reopen and recovery from a held-key crash")
        finally:
            if screen is not None:
                screen.close()
            if client.poll() is None:
                client.terminate()
                try:
                    client.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    client.kill()
                    client.wait()
            (args.output / "keyboard-status.json").write_text(json.dumps(records, indent=2) + "\n")


if __name__ == "__main__":
    main()
