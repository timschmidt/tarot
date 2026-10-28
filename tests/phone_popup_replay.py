#!/usr/bin/env python3
"""Real xdg-popup focus/grab regression on the verified phone's temporary VT8 session."""
import argparse
import json
import os
from pathlib import Path
import pwd
import subprocess
import time
from phone_gesture_replay import Touchscreen, request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--socket", type=Path, default=Path("/run/user/10000/tarot-drm.sock"))
    parser.add_argument("--probe", type=Path, default=Path("/home/user/tarot-dev/bin/input-probe"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    assert os.geteuid() == 0
    assert "PinePhone Pro" in Path("/proc/device-tree/model").read_text()
    initial = request(args.socket, "status")
    assert initial["render_metrics"]["drm_presentations"] > 0
    owner = args.socket.stat()
    account = pwd.getpwuid(owner.st_uid)
    args.output.mkdir(parents=True, exist_ok=True)
    os.chown(args.output, owner.st_uid, owner.st_gid)
    report = args.output / "popup-events.jsonl"
    report.unlink(missing_ok=True)
    records = {}

    def guard():
        assert Path("/sys/class/tty/tty0/active").read_text().strip() == "tty8"
        assert request(args.socket, "status")["pid"] == initial["pid"], "Compositor ended"

    def events():
        if not report.exists():
            return []
        return [json.loads(line) for line in report.read_bytes().split(b"\n")[:-1] if line]

    def wait_event(start, kind, **values):
        for _ in range(150):
            matches = [event for event in events()[start:] if event["event"] == kind
                       and all(event.get(key) == value for key, value in values.items())]
            if matches:
                return matches[-1]
            assert client.poll() is None, ("Probe exited", events()[start:])
            time.sleep(0.01)
        raise AssertionError(("Missing event", kind, values, events()[start:]))

    def status(label):
        result = request(args.socket, "status")
        records[label] = result
        return result

    def capture(name):
        request(args.socket, "capture", path=str(args.output / (name + ".png")))
        for _ in range(100):
            result = request(args.socket, "status")
            if not result["capture_pending"]:
                assert not result["capture_error"], result["capture_error"]
                return
            time.sleep(0.01)
        raise AssertionError("Capture did not finish")

    def tap(point):
        screen.down(*point)
        screen.up()
        time.sleep(0.12)

    def popup_rectangle(level, since):
        configs = {}
        for event in events()[since:]:
            if event["event"] == "popup_configure":
                configs[event["level"]] = event
        # Parent window geometry is (20,30,600,1000). Popup configurations are
        # relative to their parent's geometry; their own geometry starts at zero.
        x, y = sum(configs[n]["x"] for n in range(1, level + 1)), sum(configs[n]["y"] for n in range(1, level + 1))
        factor = min(full["width"] / 600, full["height"] / 1000)
        left = full["x"] + (full["width"] - 600 * factor) / 2 + x * factor
        top = full["y"] + (full["height"] - 1000 * factor) / 2 + y * factor
        return left, top, configs[level]["width"] * factor, configs[level]["height"] * factor

    def center(level, since):
        x, y, width, height = popup_rectangle(level, since)
        return round(x + width / 2), round(y + height / 2)

    guard()
    environment = dict(os.environ, WAYLAND_DISPLAY=initial["wayland_display"], XDG_RUNTIME_DIR=str(args.socket.parent))
    environment.pop("DISPLAY", None)
    with (args.output / "probe.log").open("w") as log:
        client = subprocess.Popen([str(args.probe), "--popups", "--report", str(report)],
                                  stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                                  env=environment, cwd=account.pw_dir, user=owner.st_uid,
                                  group=owner.st_gid, extra_groups=os.getgrouplist(account.pw_name, owner.st_gid))
        screen = None
        try:
            for _ in range(100):
                current = request(args.socket, "status")
                matches = [card for card in current["cards"] if card["app_id"] == "org.tarot.InputProbe"]
                if matches:
                    window = matches[0]["id"]
                    break
                assert client.poll() is None
                time.sleep(0.02)
            else:
                raise AssertionError("Probe did not map")
            time.sleep(0.4)
            full = next(v["rect"] for v in current["presentation"]["windows"] if v["id"] == window)
            # Take the settled rectangle, not the initial mapping animation snapshot.
            full = next(v["rect"] for v in status("focused")["presentation"]["windows"] if v["id"] == window)
            screen = Touchscreen(guard)

            start = len(events())
            tap((240, 360))
            wait_event(start, "popup_configure", level=1)
            wait_event(start, "keyboard_enter", target="popup1")
            opened = status("popup-one")
            assert opened["popup_grab_active"] and opened["popup_owner"] == window, opened
            assert len(opened["cards"]) == len(initial["cards"]) + 1, opened
            capture("popup-one")

            # Expire the original action in the serial ledger. An active parent
            # grab may still authorize its nested menu using that parent serial.
            time.sleep(5.3)
            nested = len(events())
            tap(center(1, start))
            wait_event(start, "popup_configure", level=2)
            wait_event(start, "keyboard_enter", target="popup2")
            assert not [event for event in events()[nested:]
                        if event["event"] == "keyboard_enter" and event["target"] == "root"], "Nested grab briefly focused the root"
            status("nested-popup")
            capture("popup-nested")

            closed = len(events())
            tap(center(2, start))  # Client destroys the top popup only.
            wait_event(closed, "popup_destroyed", level=2)
            wait_event(closed, "keyboard_enter", target="popup1")
            assert status("nested-return")["popup_grab_active"]

            reopened = len(events())
            tap(center(1, start))
            wait_event(reopened, "popup_configure", level=2)
            wait_event(reopened, "keyboard_enter", target="popup2")
            dismissed = len(events())
            tap((180, 710))  # Outside the app: dismiss the stack and enter overview.
            wait_event(dismissed, "popup_done", level=1)
            done = [event["level"] for event in events()[dismissed:] if event["event"] == "popup_done"]
            assert done == [2, 1], done
            time.sleep(0.3)
            overview = status("navigation-dismissed")
            assert overview["view"] == "Overview" and not overview["popup_grab_active"], overview

            request(args.socket, "activate", id=window)
            time.sleep(0.4)
            invalid = len(events())
            tap((310, 600))  # Probe deliberately supplies an unrelated serial.
            wait_event(invalid, "popup_done", level=1)
            assert not [event for event in events()[invalid:]
                        if event["event"] == "keyboard_enter" and event["target"] == "popup1"]
            assert not status("invalid-serial-denied")["popup_grab_active"]

            corner = len(events())
            tap((310, 100))
            wait_event(corner, "popup_configure", level=1)
            wait_event(corner, "keyboard_enter", target="popup1")
            x, y, width, height = popup_rectangle(1, corner)
            assert x >= -1 and x + width <= 361 and y >= 27 and y + height <= 693, (x, y, width, height)
            status("constrained-corner")
            capture("popup-corner")
            parent = len(events())
            tap((50, 550))  # Normal owner-events delivery to the parent closes its menu.
            wait_event(parent, "popup_destroyed", level=1)
            wait_event(parent, "keyboard_enter", target="root")
            time.sleep(0.15)
            assert not status("parent-restored")["popup_grab_active"]

            tap((18, 582))  # Valid serial, but deliberately grab after initial commit.
            for _ in range(100):
                if client.poll() is not None:
                    break
                time.sleep(0.02)
            assert client.poll() not in (None, 0), "Invalid mapped grab did not isolate the bad client"
            assert "already is mapped" in (args.output / "probe.log").read_text()
            time.sleep(0.3)
            survived = status("invalid-client-isolated")
            assert all(card["id"] != window for card in survived["cards"]), survived
            assert {card["id"] for card in initial["cards"]}.issubset({card["id"] for card in survived["cards"]}), survived
            assert not survived["popup_grab_active"], survived
            print("PASS: popup focus, nested serial reuse, unwind, navigation dismissal, constraints and invalid-client isolation")
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
            (args.output / "popup-status.json").write_text(json.dumps(records, indent=2) + "\n")


if __name__ == "__main__":
    main()
