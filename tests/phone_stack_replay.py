#!/usr/bin/env python3
"""Real phone hold/drag stacks, member selection, extraction and cooperative close."""
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
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    assert os.geteuid() == 0 and "PinePhone Pro" in Path("/proc/device-tree/model").read_text()
    initial = request(args.socket, "status")
    assert len(initial["cards"]) == 1 and initial["cards"][0]["app_id"] == "gtk3-demo", initial
    gtk = initial["cards"][0]["id"]
    owner = args.socket.stat()
    account = pwd.getpwuid(owner.st_uid)
    args.output.mkdir(parents=True, exist_ok=True)
    os.chown(args.output, owner.st_uid, owner.st_gid)
    records = {}

    def state(label):
        value = request(args.socket, "status")
        records[label] = value
        assert value["pid"] == initial["pid"]
        return value

    def await_state(label, condition):
        for _ in range(300):
            value = state(label)
            if condition(value):
                return value
            time.sleep(.02)
        raise AssertionError((label, value))

    def fingerprint(value):
        return [(card["id"], card["stack"]) for card in value["cards"]]

    def stack(value, window):
        return next(card["stack"] for card in value["cards"] if card["id"] == window)

    def rect(value, window):
        return next(w["rect"] for w in value["presentation"]["windows"] if w["id"] == window)

    def point(window):
        value = request(args.socket, "status")
        xs = []
        for x in range(8, 353, 4):
            hit = next((w["id"] for w in reversed(value["presentation"]["windows"])
                        if w["opacity"] > .01 and w["rect"]["x"] <= x < w["rect"]["x"] + w["rect"]["width"]
                        and w["rect"]["y"] <= 300 < w["rect"]["y"] + w["rect"]["height"]), None)
            if hit == window:
                xs.append(x)
        assert xs, ("Card has no exposed touch point", window, value)
        return (xs[0] + xs[-1]) / 2, 300

    def guard():
        assert Path("/sys/class/tty/tty0/active").read_text().strip() == "tty8"
        assert request(args.socket, "status")["pid"] == initial["pid"]

    def tap(x, y):
        screen.down(x, y)
        screen.up()
        time.sleep(.12)

    def capture(name):
        request(args.socket, "capture", path=str(args.output / (name + ".png")))
        await_state("capture-" + name, lambda s: not s["capture_pending"] and not s["capture_error"])

    def hold(window):
        before = fingerprint(request(args.socket, "status"))
        screen.down(*point(window))
        value = await_state("held-" + str(window), lambda s: (s["presentation"].get("arrange") or {}).get("id") == window)
        assert fingerprint(value) == before
        return value

    def drag_to(anchor, separate=False, before=False):
        for _ in range(150):
            value = request(args.socket, "status")
            target = rect(value, anchor)
            center = target["x"] + target["width"] / 2
            if 40 <= center <= 320:
                break
            screen.move(10 if center < 40 else 350, 300, delay=.02)
            time.sleep(.04)
        else:
            raise AssertionError(("Edge paging did not expose target", anchor, value))
        screen.move(center + (-20 if before else 20), 590 if separate else 300)
        expected = {"Separate" if separate else "Into": {"anchor": anchor, "before": before}}
        await_state("drop-target", lambda s: (s["presentation"].get("arrange") or {}).get("target") == expected)

    environment = dict(os.environ, WAYLAND_DISPLAY=initial["wayland_display"], XDG_RUNTIME_DIR=str(args.socket.parent),
                       WINIT_UNIX_BACKEND="wayland", HOME=account.pw_dir)
    for name in ("WAYLAND_SOCKET", "DISPLAY", "XDG_CACHE_HOME"):
        environment.pop(name, None)
    screen = None
    with (args.output / "client.log").open("w") as log:
        client = subprocess.Popen(["/home/user/tarot-dev/bin/tarot-egui-probe", "--second-window"],
                                  stdin=subprocess.DEVNULL, stdout=log, stderr=log, env=environment,
                                  cwd=account.pw_dir, user=owner.st_uid, group=owner.st_gid,
                                  extra_groups=os.getgrouplist(account.pw_name, owner.st_gid))
        try:
            mapped = await_state("mapped", lambda s: len(s["cards"]) == 3)
            primary = next(c["id"] for c in mapped["cards"] if "primary" in c["title"])
            secondary = next(c["id"] for c in mapped["cards"] if "second" in c["title"])
            ids = {c["id"] for c in mapped["cards"]}
            request(args.socket, "activate", id=primary)
            request(args.socket, "overview")
            time.sleep(.4)
            screen = Touchscreen(guard)
            before = fingerprint(state("before-cancel"))
            hold(primary)
            capture("card-lifted")
            screen.down(100, 500, slot=1)
            screen.up(slot=1)
            screen.up()
            await_state("cancelled", lambda s: s["presentation"]["arrange"] is None)
            time.sleep(.3)
            assert fingerprint(state("after-cancel")) == before

            hold(primary)
            drag_to(gtk)
            capture("stack-drop-target")
            screen.up()
            grouped = await_state("grouped", lambda s: stack(s, gtk) == stack(s, primary) and s["presentation"]["arrange"] is None)
            assert {c["id"] for c in grouped["cards"]} == ids and grouped["view"] == "Overview"
            time.sleep(.3)
            capture("two-card-stack")

            # The exposed member activates its own native window without relaunching.
            tap(*point(gtk))
            await_state("gtk-selected", lambda s: s["view"] == {"App": gtk})
            tap(180, 710)
            await_state("gtk-returned", lambda s: s["view"] == "Overview")
            time.sleep(.3)
            tap(*point(primary))
            await_state("primary-selected", lambda s: s["view"] == {"App": primary})
            tap(180, 710)
            await_state("primary-returned", lambda s: s["view"] == "Overview")
            time.sleep(.3)

            # Explicit menu alternative: Move left within the current stack.
            current = state("before-menu")
            r = rect(current, primary)
            tap(r["x"] + 16, r["y"] - 26)
            await_state("menu-open", lambda s: s["card_menu_open"])
            time.sleep(.2)
            capture("card-menu")
            before_menu_hold = fingerprint(state("before-menu-hold"))
            screen.down(r["x"] + 70, r["y"] + 28)
            time.sleep(.55)
            held_menu = state("menu-item-held")
            assert held_menu["presentation"]["arrange"] is None and fingerprint(held_menu) == before_menu_hold
            screen.up()
            menu_order = await_state("menu-moved-left", lambda s: [c["id"] for c in s["cards"]].index(primary) < [c["id"] for c in s["cards"]].index(gtk))
            assert stack(menu_order, gtk) == stack(menu_order, primary)
            time.sleep(.3)

            hold(gtk)
            drag_to(primary, before=True)
            screen.up()
            reordered = await_state("member-reordered", lambda s: s["selected"] == gtk and s["presentation"]["arrange"] is None)
            order = [c["id"] for c in reordered["cards"]]
            assert order.index(gtk) < order.index(primary) and stack(reordered, gtk) == stack(reordered, primary)
            time.sleep(.3)
            hold(gtk)
            drag_to(primary, separate=True)
            capture("separate-drop-target")
            screen.up()
            separated = await_state("separated", lambda s: stack(s, gtk) != stack(s, primary) and s["presentation"]["arrange"] is None)
            assert len({c["stack"] for c in separated["cards"]}) == 3
            time.sleep(.3)
            capture("unstacked")

            hold(gtk)
            drag_to(secondary, separate=True)
            screen.up()
            await_state("deck-reordered", lambda s: s["cards"][-1]["id"] == gtk and s["presentation"]["arrange"] is None)
            time.sleep(.3)
            capture("deck-reordered")

            hold(gtk)
            drag_to(primary)
            screen.up()
            regrouped = await_state("regrouped", lambda s: stack(s, gtk) == stack(s, primary) and s["presentation"]["arrange"] is None)
            group = stack(regrouped, primary)
            time.sleep(.3)
            screen.down(180, 400)
            for y in (340, 240, 140):
                screen.move(180, y)
            screen.up()
            closed = await_state("member-closed", lambda s: all(c["id"] != gtk for c in s["cards"]))
            assert {c["id"] for c in closed["cards"]} == {primary, secondary} and stack(closed, primary) == group
            assert client.poll() is None
            time.sleep(.3)
            capture("remaining-stack-member")
            request(args.socket, "close", id=secondary)
            await_state("secondary-closed", lambda s: len(s["cards"]) == 1)
            assert client.poll() is None
            request(args.socket, "close", id=primary)
            await_state("empty", lambda s: not s["cards"])
            assert client.wait(timeout=5) == 0
            print("PASS: timed hold, cancellation, edge paging, cross-app stacking, member selection/menu/reorder, unstacking and independent close")
        except BaseException:
            try:
                capture("failure")
            except Exception as error:
                print("Failure capture unavailable:", error)
            raise
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
            (args.output / "status.json").write_text(json.dumps(records, indent=2) + "\n")


if __name__ == "__main__":
    main()
