#!/usr/bin/env python3
"""Real touch search, cancellation and filtered application launch on temporary VT8."""
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

    def guard():
        assert Path("/sys/class/tty/tty0/active").read_text().strip() == "tty8"
        assert request(args.socket, "status")["pid"] == initial["pid"]

    def state(label):
        result = request(args.socket, "status")
        records[label] = result
        return result

    def await_state(label, **fields):
        for _ in range(200):
            result = state(label)
            if all(result.get(key) == value for key, value in fields.items()):
                return result
            time.sleep(.02)
        raise AssertionError((label, fields, result))

    def tap(x, y):
        screen.down(x, y)
        screen.up()
        time.sleep(.1)

    def capture(name):
        request(args.socket, "capture", path=str(args.output / (name + ".png")))
        for _ in range(100):
            result = request(args.socket, "status")
            if not result["capture_pending"]:
                assert not result["capture_error"]
                return
            time.sleep(.02)
        raise AssertionError("Capture did not finish")

    request(args.socket, "launcher")
    time.sleep(.4)
    screen = Touchscreen(guard)
    try:
        tap(40, 16)
        for _ in range(100):
            if request(args.socket,'status')['launcher']['menu_open']:break
            time.sleep(.02)
        else:raise AssertionError('Launcher menu did not open')
        tap(160, 60)
        focused = await_state("search-focused", keyboard_visible=True, shell_text_focus=True, launcher_query="", view="Launcher")
        assert focused["app_area"]["height"] == 424
        capture("launcher-keyboard-open")
        for point, query in [((139, 609), "c"), ((23, 557), "ca"), ((337, 557), "cal"), ((139, 609), "calc")]:
            tap(*point)
            await_state("query-" + query, launcher_query=query, keyboard_visible=True, shell_text_focus=True)
        capture("launcher-filtered-calc")

        # The same shared key view must preserve UTF-8 edits in shell text too.
        tap(30, 663)
        tap(69, 609)
        await_state("query-accent", launcher_query="calcé")
        tap(104, 609)
        await_state("query-emoji", launcher_query="calcé🙂")
        tap(330, 609)
        await_state("emoji-deleted", launcher_query="calcé")
        tap(330, 609)
        await_state("accent-deleted", launcher_query="calc")
        screen.down(69, 609)
        screen.down(104, 609, slot=1)
        screen.up(slot=1)
        screen.up()
        time.sleep(.2)
        await_state("typing-cancelled", launcher_query="calc", keyboard_visible=True, shell_text_focus=True)

        tap(320, 663)
        hidden = await_state("hidden", keyboard_visible=False, shell_text_focus=False, launcher_query="calc")
        assert hidden["app_area"]["height"] == 664
        tap(100, 116)
        await_state("same-field-reopened", keyboard_visible=True, shell_text_focus=True)
        tap(330, 609)
        await_state("last-letter-deleted", launcher_query="cal")
        tap(139, 609)
        await_state("query-restored", launcher_query="calc")
        tap(245, 663)  # Enter dismisses the shell keyboard while retaining results.
        await_state("submitted", keyboard_visible=False, shell_text_focus=False, launcher_query="calc")
        capture("launcher-results-ready")

        applications = request(args.socket, "applications")["applications"]
        results = [app for app in applications if "calc" in (app["name"] + " " + app["description"] + " " + " ".join(app["keywords"])).lower()]
        assert results and results[0]["id"] == "org.gnome.Calculator.desktop", results
        existing = {card["id"] for card in initial["cards"]}
        tap(70, 200)
        for _ in range(500):
            launched = state("launched-from-filter")
            cards = [card for card in launched["cards"] if card["id"] not in existing and card["app_id"] == "org.gnome.Calculator"]
            if cards:
                break
            time.sleep(.02)
        else:
            raise AssertionError(("Filtered tile did not launch Calculator", launched))
        assert launched["view"] == {"App": cards[0]["id"]} and not launched["shell_text_focus"]
        time.sleep(.4)
        capture("calculator-from-touch-search")
        request(args.socket, "close", id=cards[0]["id"])
        for _ in range(100):
            if all(card["id"] != cards[0]["id"] for card in request(args.socket, "status")["cards"]):
                break
            time.sleep(.02)
        print("PASS: shell touch typing, Unicode deletion, cancellation, hide/reopen and filtered desktop-entry launch")
    finally:
        screen.close()
        (args.output / "launcher-keyboard-status.json").write_text(json.dumps(records, indent=2) + "\n")


if __name__ == "__main__":
    main()
