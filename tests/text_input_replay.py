#!/usr/bin/env python3
"""Real protocol replay against a development compositor with the trusted text probe.

Start Tarot with --input-method text-probe and method role/report/commands arguments.
Only synthetic fixture text is used. This is not an on-screen keyboard.
"""
import argparse
import json
import os
from pathlib import Path
import pwd
import subprocess
import time
from phone_gesture_replay import request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--socket", type=Path, required=True)
    parser.add_argument("--probe", type=Path, required=True)
    parser.add_argument("--method-report", type=Path, required=True)
    parser.add_argument("--method-commands", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    initial = request(args.socket, "status")
    owner = args.socket.stat()
    args.output.mkdir(parents=True, exist_ok=True)
    if os.geteuid() == 0:
        os.chown(args.output, owner.st_uid, owner.st_gid)
    account = pwd.getpwuid(owner.st_uid)
    environment = dict(os.environ, WAYLAND_DISPLAY=initial["wayland_display"], XDG_RUNTIME_DIR=os.environ.get("XDG_RUNTIME_DIR", str(args.socket.parent)))
    environment.pop("WAYLAND_SOCKET", None)
    environment.pop("DISPLAY", None)
    process_args = dict(env=environment, cwd=account.pw_dir)
    if os.geteuid() == 0:
        process_args.update(user=owner.st_uid, group=owner.st_gid, extra_groups=os.getgrouplist(account.pw_name, owner.st_gid))
    processes = []
    records = {}

    def events(path):
        if not path.exists():
            return []
        return [json.loads(line) for line in path.read_bytes().split(b"\n")[:-1] if line]

    def await_event(path, start, kind, **fields):
        for _ in range(250):
            matching = [event for event in events(path)[start:] if event["event"] == kind
                        and all(event.get(key) == value for key, value in fields.items())]
            if matching:
                return matching[-1]
            time.sleep(0.02)
        raise AssertionError(("Missing event", str(path), kind, fields, events(path)[start:]))

    def send(path, command, **fields):
        with path.open("a") as stream:
            stream.write(json.dumps(dict(command=command, **fields)) + "\n")

    def method(command, **fields):
        start = len(events(args.method_report))
        send(args.method_commands, command, **fields)
        await_event(args.method_report, start, "method_sent", command=command)

    def settle_method(start):
        await_event(args.method_report, start, "method_done")
        time.sleep(0.08)

    def app(name):
        output = args.output / name
        output.mkdir(exist_ok=True)
        if os.geteuid() == 0:
            os.chown(output, owner.st_uid, owner.st_gid)
        report, commands = output / "events.jsonl", output / "commands.jsonl"
        report.unlink(missing_ok=True)
        commands.write_text("")
        if os.geteuid() == 0:
            os.chown(commands, owner.st_uid, owner.st_gid)
        before = {card["id"] for card in request(args.socket, "status")["cards"]}
        with (output / "client.log").open("w") as log:
            process = subprocess.Popen([str(args.probe), "--role", "app", "--report", str(report), "--commands", str(commands)],
                                       stdin=subprocess.DEVNULL, stdout=log, stderr=log, **process_args)
        processes.append(process)
        await_event(report, 0, "enter")
        for _ in range(100):
            current = request(args.socket, "status")
            found = [card["id"] for card in current["cards"] if card["id"] not in before and card["app_id"] == "org.tarot.TextProbe"]
            if found:
                return report, commands, found[0]
            assert process.poll() is None
            time.sleep(0.02)
        raise AssertionError("Text client did not map")

    try:
        await_event(args.method_report, 0, "ready")
        global_name = await_event(args.method_report, 0, "method_global")["name"]
        start_method = len(events(args.method_report))
        first, first_commands, window = app("first")
        assert await_event(first, 0, "registry")["input_method_visible"] is False
        await_event(args.method_report, start_method, "activate")
        await_event(args.method_report, start_method, "surrounding", text="hello ")
        settle_method(start_method)

        start = len(events(first))
        method("preedit", text="にほん")
        await_event(first, start, "applied", text="hello ", preedit="にほん")
        await_event(first, start, "preedit", text="にほん", begin=9, end=9)
        settle_method(len(events(args.method_report)) - 1)

        start, start_method = len(events(first)), len(events(args.method_report))
        method("commit", text="café 🙂")
        await_event(first, start, "applied", text="hello café 🙂", preedit="")
        settle_method(start_method)

        start, start_method = len(events(first)), len(events(args.method_report))
        send(first_commands, "context", text="abécd", cursor=4)
        await_event(args.method_report, start_method, "surrounding", text="abécd", cursor=4)
        settle_method(start_method)
        method("delete", before=2, after=1)
        await_event(first, start, "applied", text="abd", cursor=2)
        time.sleep(0.12)

        start = len(events(first))
        method("stale", text="SHOULD NOT APPLY")
        await_event(first, start, "discarded", text="abd")
        time.sleep(0.08)
        assert not [event for event in events(first)[start:] if event["event"] == "applied"]

        start_method = len(events(args.method_report))
        send(first_commands, "disable")
        await_event(args.method_report, start_method, "deactivate")
        settle_method(start_method)
        start = len(events(first))
        method("commit", text="DISABLED")
        time.sleep(0.15)
        assert not [event for event in events(first)[start:] if event["event"] == "applied"]
        start_method = len(events(args.method_report))
        send(first_commands, "enable")
        await_event(args.method_report, start_method, "activate")
        settle_method(start_method)

        start, start_method = len(events(first)), len(events(args.method_report))
        request(args.socket, "overview")
        await_event(first, start, "leave")
        await_event(args.method_report, start_method, "deactivate")
        settle_method(start_method)
        start = len(events(first))
        method("commit", text="BACKGROUND")
        time.sleep(0.15)
        assert not [event for event in events(first)[start:] if event["event"] == "applied"]
        request(args.socket, "activate", id=window)
        await_event(first, start, "enter")
        time.sleep(0.15)

        first_count = len(events(first))
        second, _, second_window = app("second")
        time.sleep(0.15)
        start = len(events(second))
        method("commit", text="SECOND")
        await_event(second, start, "applied", text="hello SECOND")
        assert not [event for event in events(first)[first_count:] if event["event"] == "applied"]
        records["two_clients"] = request(args.socket, "status")
        assert records["two_clients"]["view"] == {"App": second_window}

        # A hidden privileged global must also reject a guessed explicit bind.
        intruder_report = args.output / "intruder.jsonl"
        intruder_log = args.output / "intruder.log"
        with intruder_log.open("w") as log:
            intruder = subprocess.run([str(args.probe), "--role", "inspect", "--report", str(intruder_report), "--commands", str(first_commands), "--try-bind", str(global_name)],
                                      stdin=subprocess.DEVNULL, stdout=log, stderr=log, timeout=5, **process_args)
        assert intruder.returncode != 0
        assert "Invalid binding of zwp_input_method_manager_v2" in intruder_log.read_text(), intruder_log.read_text()
        records["untrusted_bind_isolated"] = request(args.socket, "status")
        assert records["untrusted_bind_isolated"]["pid"] == initial["pid"]
        assert len(records["untrusted_bind_isolated"]["cards"]) == len(initial["cards"]) + 2
        disconnected = len(events(second))
        send(args.method_commands, "quit")
        await_event(second, disconnected, "leave")
        request(args.socket, "overview")
        request(args.socket, "activate", id=window)
        records["helper_exit_survived"] = request(args.socket, "status")
        assert records["helper_exit_survived"]["pid"] == initial["pid"]
        assert records["helper_exit_survived"]["view"] == {"App": window}
        assert all(process.poll() is None for process in processes)
        print("PASS: Unicode composition, UTF-8 deletion, stale serials, focus isolation, privileged-global denial and helper exit")
    finally:
        for process in processes:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
        (args.output / "text-status.json").write_text(json.dumps(records, indent=2) + "\n")
        if args.method_report.exists():
            (args.output / "method-events.jsonl").write_bytes(args.method_report.read_bytes())


if __name__ == "__main__":
    main()
