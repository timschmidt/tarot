#!/usr/bin/env python3
"""Manual native-client coordinate regression on the verified spare phone/VT8.

Uses the real uinput/libinput route and an unprivileged Wayland client. It checks
client-received events, rather than trusting a compositor-local coordinate log.
"""
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
    parser.add_argument("--moving-child", action="store_true")
    args = parser.parse_args()
    assert os.geteuid() == 0
    assert "PinePhone Pro" in Path("/proc/device-tree/model").read_text()
    initial = request(args.socket, "status")
    assert initial["render_metrics"]["drm_presentations"] > 0
    pid = initial["pid"]
    owner = args.socket.stat()
    account = pwd.getpwuid(owner.st_uid)
    args.output.mkdir(parents=True, exist_ok=True)
    os.chown(args.output, owner.st_uid, owner.st_gid)
    report = args.output / "client-events.jsonl"
    report.unlink(missing_ok=True)
    records = {}

    def guard():
        assert Path("/sys/class/tty/tty0/active").read_text().strip() == "tty8"
        assert request(args.socket, "status")["pid"] == pid, "Test compositor ended"

    def status(label):
        result = request(args.socket, "status")
        records[label] = result
        return result

    def events():
        if not report.exists():
            return []
        return [json.loads(line) for line in report.read_bytes().split(b"\n")[:-1] if line]

    def received(start, kind, count=1):
        for _ in range(100):
            result = [event for event in events()[start:] if event["event"] == kind]
            if len(result) >= count:
                return result
            assert client.poll() is None, "Input probe exited"
            time.sleep(0.01)
        raise AssertionError(f"Missing {kind}: {events()[start:]}")

    def rect(result):
        return next(item["rect"] for item in result["presentation"]["windows"] if item["id"] == window)

    def expected(point, rectangle, child=False, child_origin=(100, 150)):
        # Independent client fixture: geometry (20,30,600,1000), child at (100,150).
        # Integer physical alignment can differ from ideal centering by < 1 client unit.
        factor = min(rectangle["width"] / 600, rectangle["height"] / 1000)
        x = (point[0] - rectangle["x"] - (rectangle["width"] - 600 * factor) / 2) / factor + 20
        y = (point[1] - rectangle["y"] - (rectangle["height"] - 1000 * factor) / 2) / factor + 30
        return x - (child_origin[0] if child else 0), y - (child_origin[1] if child else 0)

    def check(event, point, rectangle, child=False, child_origin=(100, 150)):
        x, y = expected(point, rectangle, child, child_origin)
        assert event["target"] == ("child" if child else "root"), event
        assert abs(event["x"] - x) < 1.25 and abs(event["y"] - y) < 1.25, (event, (x, y), rectangle)

    def capture(name):
        request(args.socket, "capture", path=str(args.output / (name + ".png")))
        for _ in range(100):
            result = request(args.socket, "status")
            if not result["capture_pending"]:
                assert not result["capture_error"], result["capture_error"]
                return
            time.sleep(0.01)
        raise AssertionError("Capture did not finish")

    guard()
    environment = dict(os.environ, WAYLAND_DISPLAY=initial["wayland_display"],
                       XDG_RUNTIME_DIR=str(args.socket.parent))
    environment.pop("DISPLAY", None)
    with (args.output / "probe.log").open("w") as log:
        command = [str(args.probe), "--report", str(report)] + (["--move-child"] if args.moving_child else [])
        client = subprocess.Popen(command,
                                  stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                                  env=environment, cwd=account.pw_dir,
                                  user=owner.st_uid, group=owner.st_gid,
                                  extra_groups=os.getgrouplist(account.pw_name, owner.st_gid))
        screen = None
        try:
            for _ in range(100):
                current = request(args.socket, "status")
                matching = [card for card in current["cards"] if card["app_id"] == "org.tarot.InputProbe"]
                if matching:
                    window = matching[0]["id"]
                    break
                assert client.poll() is None, "Input probe did not map"
                time.sleep(0.02)
            else:
                raise AssertionError("Input probe did not map")
            time.sleep(0.4)
            full = rect(status("focused"))
            capture("native-input-fixture")
            screen = Touchscreen(guard)

            # A fixed oversized surface must scale motion as well as contact down.
            start = len(events())
            points = [(310, 550), (275, 510), (230, 470)]
            screen.down(*points[0])
            check(received(start, "touch_down")[0], points[0], full)
            for index, point in enumerate(points[1:], 1):
                screen.move(*point)
                check(received(start, "touch_motion", index)[index - 1], point, full)
            screen.up()
            received(start, "touch_up")
            status("scaled-root-drag")

            # The child keeps ownership when motion leaves its original 200x200 area.
            start = len(events())
            screen.down(78, 162)
            check(received(start, "touch_down")[0], (78, 162), full, child=True)
            if args.moving_child:
                received(start, "child_move_presented")
            for index, point in enumerate([(100, 180), (250, 400)], 1):
                screen.move(*point)
                check(received(start, "touch_motion", index)[index - 1], point, full, child=True,
                      child_origin=(150, 200) if args.moving_child else (100, 150))
            screen.up()
            received(start, "touch_up")
            if args.moving_child:
                received(start, "child_restore_presented")
            status("subsurface-owned-drag")

            # Smithay's touch-down grab keeps the initiating surface for the whole
            # sequence. Each contact still needs its own correctly scaled coordinates.
            for child_first in (False, True):
                start = len(events())
                first_point, second_point = ((78, 162), (310, 550)) if child_first else ((310, 550), (78, 162))
                first_motion, second_motion = ((100, 180), (275, 510)) if child_first else ((275, 510), (100, 180))
                screen.down(*first_point)
                screen.down(*second_point, slot=1)
                downs = received(start, "touch_down", 2)
                assert downs[0]["id"] != downs[1]["id"]
                check(downs[0], first_point, full, child=child_first)
                check(downs[1], second_point, full, child=child_first)
                screen.move(*first_motion)
                screen.move(*second_motion, slot=1)
                moves = received(start, "touch_motion", 2)
                for event in moves:
                    check(event, first_motion if event["id"] == downs[0]["id"] else second_motion,
                          full, child=child_first)
                screen.up(slot=1)
                screen.up()
                received(start, "touch_up", 2)
            status("native-multitouch")

            # A flushed two-contact app sequence is canceled on overview, and held
            # contacts cannot turn into a strip drag or start another app sequence.
            start = len(events())
            screen.down(310, 550)
            screen.down(78, 162, slot=1)
            received(start, "touch_down", 2)
            request(args.socket, "overview")
            received(start, "touch_cancel")
            count = len(events())
            screen.move(180, 710)
            screen.move(100, 500, slot=1)
            screen.up(slot=1)
            screen.up()
            time.sleep(0.3)
            assert status("view-change-canceled")["view"] == "Overview"
            assert not [e for e in events()[count:] if e["event"].startswith("touch_")], events()[count:]

            # Begin during card expansion, then keep almost the same physical point.
            # The received source coordinate must follow the moving visual, not remain frozen.
            start = len(events())
            request(args.socket, "activate", id=window)
            screen.down(280, 450)
            first = received(start, "touch_down")[0]
            moving = status("touch-during-expansion")
            assert rect(moving)["height"] < full["height"] - 1, "Missed the expansion window"
            for index in range(12):
                screen.move(281 if index % 2 == 0 else 280, 450, delay=0.025)
            moves = received(start, "touch_motion", 12)
            check(moves[-1], (280, 450), full)
            assert abs(first["y"] - moves[-1]["y"]) > 8, (first, moves[-1])
            screen.up()
            received(start, "touch_up")
            status("expansion-drag-finished")
            capture("native-input-after-drag")
            # Removing an input device with a held contact must cancel the
            # delivered native sequence and allow a replacement touchscreen.
            removed = len(events())
            screen.down(310, 550)
            received(removed, "touch_down")
            screen.close()
            screen = None
            received(removed, "touch_cancel")
            screen = Touchscreen(guard)
            fresh = len(events())
            screen.down(310, 550)
            check(received(fresh, "touch_down")[0], (310, 550), full)
            screen.up()
            received(fresh, "touch_up")
            status("replacement-touchscreen")
            records["client_event_count"] = len(events())
            print("PASS: native root/subsurface coordinates, multitouch, cancellation, expansion and device replacement")
        finally:
            if screen is not None:
                screen.close()
            client.terminate()
            try:
                client.wait(timeout=3)
            except subprocess.TimeoutExpired:
                client.kill()
                client.wait()
            (args.output / "native-input-status.json").write_text(json.dumps(records, indent=2) + "\n")


if __name__ == "__main__":
    main()
