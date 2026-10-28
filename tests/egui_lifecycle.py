#!/usr/bin/env python3
"""Manual ordinary eframe multiwindow regression against a running Tarot session."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time
from phone_gesture_replay import request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--socket", type=Path, required=True)
    parser.add_argument("--probe", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    initial = request(args.socket, "status")
    assert not any(card["app_id"] == "org.tarot.EguiProbe" for card in initial["cards"])
    args.output.mkdir(parents=True, exist_ok=True)
    records = {}

    def wait_for(label, count):
        for _ in range(300):
            state = request(args.socket, "status")
            records[label] = state
            assert state["pid"] == initial["pid"]
            cards = [card for card in state["cards"] if card["app_id"] == "org.tarot.EguiProbe"]
            if len(cards) == count:
                return cards
            time.sleep(.02)
        raise AssertionError((label, state))

    def capture(name):
        request(args.socket, "capture", path=str(args.output.resolve() / (name + ".png")))
        for _ in range(150):
            state = request(args.socket, "status")
            if not state["capture_pending"]:
                assert not state["capture_error"], state
                return
            time.sleep(.02)
        raise AssertionError("Capture timed out")

    environment = dict(os.environ, WAYLAND_DISPLAY=initial["wayland_display"], WINIT_UNIX_BACKEND="wayland")
    environment.pop("WAYLAND_SOCKET", None)
    environment.pop("DISPLAY", None)
    report = args.output.resolve() / "app.jsonl"
    assert not report.exists(), "Use a fresh evidence directory"
    with (args.output / "client.log").open("w") as log:
        client = subprocess.Popen([str(args.probe.resolve()), "--second-window", "--report", str(report)],
                                  env=environment, stdin=subprocess.DEVNULL, stdout=log, stderr=log)
        try:
            cards = wait_for("two-windows", 2)
            assert len({card["id"] for card in cards}) == 2 and client.poll() is None
            primary = next(card["id"] for card in cards if "primary" in card["title"])
            secondary = next(card["id"] for card in cards if "second" in card["title"])
            request(args.socket, "overview")
            time.sleep(.3)
            capture("egui-two-cards")
            request(args.socket, "activate", id=primary)
            time.sleep(.3)
            capture("egui-primary")
            request(args.socket, "close", id=secondary)
            remaining = wait_for("secondary-closed", 1)
            assert remaining[0]["id"] == primary and client.poll() is None
            request(args.socket, "close", id=primary)
            wait_for("primary-closed", 0)
            assert client.wait(timeout=5) == 0
            final = request(args.socket, "status")
            assert {card["id"] for card in final["cards"]} == {card["id"] for card in initial["cards"]}
            print("PASS: ordinary eframe multiwindow mapping, card activation and independent cooperative close")
        finally:
            if client.poll() is None:
                client.terminate()
                try:
                    client.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    client.kill()
                    client.wait()
            (args.output / "status.json").write_text(json.dumps(records, indent=2) + "\n")


if __name__ == "__main__":
    main()
