#!/usr/bin/env python3
"""Ordinary eframe app: native typing, independent cards and process isolation."""
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
    parser.add_argument("--probe", type=Path, default=Path("/home/user/tarot-dev/bin/tarot-egui-probe"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--protocol-log", action="store_true", help="Log Wayland requests for this synthetic fixture only")
    args = parser.parse_args()
    assert os.geteuid() == 0 and "PinePhone Pro" in Path("/proc/device-tree/model").read_text()
    initial = request(args.socket, "status")
    assert initial["render_metrics"]["drm_presentations"] > 0
    owner = args.socket.stat()
    account = pwd.getpwuid(owner.st_uid)
    args.output.mkdir(parents=True, exist_ok=True)
    os.chown(args.output, owner.st_uid, owner.st_gid)
    records = {}
    report = args.output / "app.jsonl"
    report.unlink(missing_ok=True)

    def guard():
        assert Path("/sys/class/tty/tty0/active").read_text().strip() == "tty8"
        assert request(args.socket, "status")["pid"] == initial["pid"]

    def status(label):
        state = request(args.socket, "status")
        records[label] = state
        return state

    def wait_for(label, condition, seconds=6):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            state = status(label)
            if condition(state):
                return state
            time.sleep(.02)
        raise AssertionError((label, state))

    def app_state():
        if report.exists():
            lines = report.read_bytes().split(b"\n")[:-1]
            if lines:
                return json.loads(lines[-1])
        return {}

    def wait_app(**fields):
        for _ in range(250):
            value = app_state()
            if all(value.get(k) == v for k, v in fields.items()):
                return value
            time.sleep(.02)
        raise AssertionError(("Application state", fields, value))

    def cards(state):
        return [card for card in state["cards"] if card["app_id"] == "org.tarot.EguiProbe"]

    def tap(x, y):
        screen.down(x, y)
        screen.up()
        time.sleep(.12)

    def tap_widget(window, viewport, name):
        time.sleep(.3)
        state = request(args.socket, "status")
        assert state["view"] == {"App": window}
        target = next(item["rect"] for item in state["presentation"]["windows"] if item["id"] == window)
        native = app_state()[viewport]
        x, y, width, height = native["widgets"][name]
        source_w, source_h = native["size"]
        scale = min(target["width"] / source_w, target["height"] / source_h)
        origin_x = target["x"] + (target["width"] - source_w * scale) / 2
        origin_y = target["y"] + (target["height"] - source_h * scale) / 2
        point = (origin_x + (x + width / 2) * scale, origin_y + (y + height / 2) * scale)
        print("Touch widget", viewport, name, point, flush=True)
        tap(*point)

    def capture(name):
        request(args.socket, "capture", path=str(args.output / (name + ".png")))
        wait_for("capture-" + name, lambda s: not s["capture_pending"] and not s["capture_error"])

    environment = dict(os.environ, WAYLAND_DISPLAY=initial["wayland_display"],
                       XDG_RUNTIME_DIR=str(args.socket.parent), WINIT_UNIX_BACKEND="wayland", HOME=account.pw_dir)
    environment.pop("XDG_CACHE_HOME", None)
    if args.protocol_log:
        environment["WAYLAND_DEBUG"] = "client"
    environment.pop("WAYLAND_SOCKET", None)
    environment.pop("DISPLAY", None)
    screen = None
    client = None
    with (args.output / "client.log").open("w") as log:
        try:
            client = subprocess.Popen([str(args.probe), "--report", str(report)],
                                      stdin=subprocess.DEVNULL, stdout=log, stderr=log, env=environment,
                                      cwd=account.pw_dir, user=owner.st_uid, group=owner.st_gid,
                                      extra_groups=os.getgrouplist(account.pw_name, owner.st_gid))
            mapped = wait_for("primary-mapped", lambda s: len(cards(s)) == 1)
            first = cards(mapped)[0]["id"]
            assert mapped["view"] == {"App": first}
            wait_app(primary_text="", second_open=False)
            screen = Touchscreen(guard)
            tap_widget(first, "primary", "text")
            wait_for("primary-keyboard", lambda s: s["keyboard_visible"])
            time.sleep(.4)
            tap(21, 504)
            wait_app(primary_text="q")
            tap(30, 663)
            tap(69, 609)
            wait_app(primary_text="qé")
            tap(104, 609)
            wait_app(primary_text="qé🙂")
            capture("egui-unicode")
            tap(330, 609)  # Winit has no surrounding-text API: virtual Backspace fallback.
            wait_app(primary_text="qé")
            tap_widget(first, "primary", "password")
            time.sleep(.3)
            tap(21, 504)
            wait_app(password_characters=1, primary_text="qé")
            capture("egui-private")
            tap(330, 609)
            wait_app(password_characters=0)
            tap(320, 663)
            wait_for("primary-hidden", lambda s: not s["keyboard_visible"])
            tap_widget(first, "primary", "open-second")
            mapped = wait_for("two-windows", lambda s: len(cards(s)) == 2)
            second = next(card["id"] for card in cards(mapped) if card["id"] != first)
            assert mapped["view"] == {"App": second} and client.poll() is None
            wait_app(second_open=True)
            tap_widget(second, "secondary", "text")
            wait_for("secondary-keyboard", lambda s: s["keyboard_visible"])
            time.sleep(.4)
            tap(21, 504)
            wait_app(secondary_text="q", primary_text="qé")
            tap(56, 504)
            wait_app(secondary_text="qw", primary_text="qé")
            capture("egui-second-window")
            request(args.socket, "overview")
            wait_for("overview", lambda s: s["view"] == "Overview" and not s["keyboard_visible"])
            time.sleep(.3)
            capture("egui-two-cards")
            request(args.socket, "activate", id=first)
            wait_for("primary-returned", lambda s: s["view"] == {"App": first})
            wait_app(primary_text="qé", secondary_text="qw")
            request(args.socket, "close", id=second)
            wait_for("secondary-closed", lambda s: [card["id"] for card in cards(s)] == [first])
            wait_app(second_open=False)
            assert client.poll() is None
            request(args.socket, "close", id=first)
            wait_for("all-closed", lambda s: not cards(s))
            assert client.wait(timeout=5) == 0
            assert {card["id"] for card in request(args.socket, "status")["cards"]} == {card["id"] for card in initial["cards"]}

            # Abrupt app exit must preserve unrelated cards and shell navigation.
            client = subprocess.Popen([str(args.probe)], stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                                      env=environment, cwd=account.pw_dir, user=owner.st_uid, group=owner.st_gid,
                                      extra_groups=os.getgrouplist(account.pw_name, owner.st_gid))
            wait_for("crash-client-mapped", lambda s: len(cards(s)) == 1)
            client.kill()
            client.wait(timeout=5)
            wait_for("crash-client-removed", lambda s: not cards(s))
            request(args.socket, "launcher")
            survived = wait_for("survived-app-crash", lambda s: s["view"] == "Launcher")
            assert survived["pid"] == initial["pid"]
            assert {card["id"] for card in survived["cards"]} == {card["id"] for card in initial["cards"]}
            print("PASS: ordinary egui Unicode/private entry, two independent cards, cooperative close and crash isolation")
        except BaseException:
            if client is not None and client.poll() is None:
                capture("failure")
            raise
        finally:
            if screen is not None:
                screen.close()
            if client is not None and client.poll() is None:
                client.terminate()
                try:
                    client.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    client.kill()
                    client.wait()
            (args.output / "status.json").write_text(json.dumps(records, indent=2) + "\n")


if __name__ == "__main__":
    main()
