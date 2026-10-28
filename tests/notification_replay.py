#!/usr/bin/env python3
"""Real D-Bus notification regression against an isolated Tarot session."""
import argparse
import json
import os
from pathlib import Path
import pwd
import subprocess
import time
from phone_gesture_replay import request


def await_value(read, condition, seconds=5):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        value = read()
        if condition(value):
            return value
        time.sleep(.02)
    raise AssertionError(value)


def session_environment(pid):
    fields = Path(f"/proc/{pid}/environ").read_bytes().split(b"\0")
    values = dict(field.split(b"=", 1) for field in fields if b"=" in field)
    environment = dict(os.environ)
    environment["DBUS_SESSION_BUS_ADDRESS"] = values[b"DBUS_SESSION_BUS_ADDRESS"].decode()
    return environment


class Fixture:
    def __init__(self, binary, directory, environment, owner=None, extra_args=()):
        directory.mkdir(parents=True, exist_ok=True)
        self.report = directory / "events.jsonl"
        self.commands = directory / "commands.jsonl"
        assert not self.report.exists(), "Use a fresh evidence directory"
        self.commands.write_text("")
        options = {}
        if owner is not None and os.geteuid() == 0:
            account = pwd.getpwuid(owner.st_uid)
            os.chown(directory, owner.st_uid, owner.st_gid)
            os.chown(self.commands, owner.st_uid, owner.st_gid)
            options = dict(user=owner.st_uid, group=owner.st_gid,
                           extra_groups=os.getgrouplist(account.pw_name, owner.st_gid), cwd=account.pw_dir)
            environment = dict(environment, HOME=account.pw_dir)
        self.log = (directory / "client.log").open("w")
        self.process = subprocess.Popen([str(binary), "--report", str(self.report), "--commands", str(self.commands), *map(str, extra_args)],
                                        env=environment, stdin=subprocess.DEVNULL, stdout=self.log, stderr=self.log, **options)
        self.sequence = 0
        await_value(self.events, lambda events: any(e["event"] == "ready" for e in events))

    def events(self):
        if not self.report.exists():
            return []
        return [json.loads(line) for line in self.report.read_bytes().split(b"\n")[:-1] if line]

    def call(self, command, **fields):
        self.sequence += 1
        with self.commands.open("a") as stream:
            stream.write(json.dumps(dict(command=command, request=self.sequence, **fields)) + "\n")
        found = await_value(self.events, lambda es: any(e["event"] == "reply" and e["request"] == self.sequence for e in es))
        return next(e for e in found if e["event"] == "reply" and e["request"] == self.sequence)

    def notify(self, **fields):
        reply = self.call("notify", **fields)
        assert reply["ok"], reply
        return reply["result"]["id"]

    def signal(self, event, identifier, **fields):
        found = await_value(self.events, lambda es: any(e["event"] == event and e["id"] == identifier and all(e.get(k) == v for k, v in fields.items()) for e in es))
        return next(e for e in found if e["event"] == event and e["id"] == identifier and all(e.get(k) == v for k, v in fields.items()))

    def close(self):
        if self.process.poll() is None:
            with self.commands.open("a") as stream:
                stream.write('{"command":"quit"}\n')
            try:
                assert self.process.wait(timeout=4) == 0
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
                raise
        self.log.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--socket", type=Path, required=True)
    parser.add_argument("--probe", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    owner = args.socket.stat()
    if os.geteuid() == 0:
        os.chown(args.output, owner.st_uid, owner.st_gid)
    initial = await_value(lambda: request(args.socket, "status"), lambda s: s["notifications"]["available"])
    environment = session_environment(initial["pid"])
    records = {}
    def state(label):
        result = request(args.socket, "status")
        records[label] = result
        assert result["pid"] == initial["pid"]
        return result
    def metadata(identifier):
        return next((n for n in request(args.socket, "status")["notifications"]["items"] if n["id"] == identifier), None)
    def ui(action):
        response = request(args.socket, "notification_action", action=action)
        if isinstance(action, dict) and "Settings" in action:
            desired = action["Settings"]
            await_value(lambda: state("settings-applied"), lambda s:
                        all(s["notifications"][key] == value for key, value in desired.items())
                        and s["notifications"]["settings_saved"])
        return response
    def capture(name):
        request(args.socket, "capture", path=str(args.output / (name + ".png")))
        await_value(lambda: request(args.socket, "status"), lambda s: not s["capture_pending"] and not s["capture_error"])

    a = b = None
    try:
        a = Fixture(args.probe.resolve(), args.output / "a", environment, owner)
        b = Fixture(args.probe.resolve(), args.output / "b", environment, owner)
        ready = a.events()[0]
        assert set(ready["capabilities"]) == {"actions", "body", "persistence"}
        identifier = a.notify(summary="Tarot notification fixture", body="Unicode café 🙂; plain text", timeout=300,
                              actions=["default", "Open", "reply", "Reply"], resident=True)
        first = await_value(lambda: metadata(identifier), lambda n: n is not None)
        assert a.notify(replaces_id=identifier, summary="Replaced without another card", timeout=0,
                        actions=["default", "Open", "reply", "Reply"], resident=True) == identifier
        replaced = await_value(lambda: metadata(identifier), lambda n: n["revision"] > first["revision"])
        time.sleep(.35)
        assert metadata(identifier)["live"]
        assert not any(e["event"] == "closed" and e["id"] == identifier for e in a.events())
        assert state("banner")["view"] == initial["view"]
        capture("native-banner")

        ui({"Invoke": {"id": identifier, "revision": first["revision"], "key": "reply"}})
        time.sleep(.1)
        assert not any(e["event"] == "action" for e in a.events())
        ui({"Invoke": {"id": identifier, "revision": replaced["revision"], "key": "reply"}})
        a.signal("action", identifier, key="reply")
        assert metadata(identifier)["live"]
        assert not any(e["event"] == "action" for e in b.events()), "Signals must return only to the sender"
        for command, fields in [("close", {"id": identifier}), ("notify", {"replaces_id": identifier})]:
            denied = b.call(command, **fields)
            assert not denied["ok"] and "AccessDenied" in denied["error"], denied
        malformed = b.call("notify", actions=["unpaired"])
        assert not malformed["ok"] and "InvalidArgs" in malformed["error"]
        oversized = b.call("notify", body="x" * 4097)
        assert not oversized["ok"] and "InvalidArgs" in oversized["error"]

        explicit = a.notify(summary="Client recalled this entry", timeout=0)
        assert a.call("close", id=explicit)["ok"]
        a.signal("closed", explicit, reason=3)
        await_value(lambda: metadata(explicit), lambda n: n is not None and not n["live"])
        expired = a.notify(summary="Transient expiry fixture", timeout=100, transient=True)
        a.signal("closed", expired, reason=1)
        await_value(lambda: metadata(expired), lambda n: n is None)
        invoked = a.notify(summary="Nonresident action", actions=["reply", "Reply"], timeout=0)
        current = await_value(lambda: metadata(invoked), lambda n: n is not None)
        ui({"Invoke": {"id": invoked, "revision": current["revision"], "key": "reply"}})
        a.signal("action", invoked, key="reply")
        a.signal("closed", invoked, reason=2)
        critical = a.notify(summary="Synthetic critical item", urgency=2, timeout=1)
        time.sleep(.15)
        assert metadata(critical)["live"]

        ui({"Settings": {"dnd": True, "hide_previews": True}})
        suppressed = a.notify(summary="Retained during do not disturb", timeout=0)
        await_value(lambda: state("dnd"), lambda s: s["notifications"]["dnd"] and s["notifications"]["banner"] is None)
        await_value(lambda: metadata(suppressed), lambda n: n is not None and n["live"])
        ui({"Settings": {"dnd": False, "hide_previews": True}})
        private = a.notify(summary="This synthetic summary is hidden in the banner", timeout=0)
        await_value(lambda: state("private"), lambda s: s["notifications"]["banner"] is not None and s["notifications"]["banner"][0] == private)
        time.sleep(.2)
        capture("hidden-preview-banner")
        request(args.socket, "notifications", open=True)
        time.sleep(.25)
        capture("notification-dashboard")
        request(args.socket, "notifications", open=False)

        a.close()
        await_value(lambda: metadata(identifier), lambda n: n["orphaned"] and n["action_count"] == 0)
        ui({"Invoke": {"id": identifier, "revision": replaced["revision"], "key": "reply"}})
        ui("Clear")
        await_value(lambda: state("cleared"), lambda s: not s["notifications"]["items"])
        ui({"Settings": {"dnd": True, "hide_previews": False}})
        for index in range(64):
            b.notify(summary=f"Bounded fixture {index}", timeout=0)
        full = b.call("notify", summary="Over limit", timeout=0)
        assert not full["ok"] and "LimitsExceeded" in full["error"], full
        assert len(state("capacity")["notifications"]["items"]) == 64
        ui("Clear")
        await_value(lambda: state("capacity-cleared"), lambda s: not s["notifications"]["items"])
        b.close()
        ui({"Settings": {"dnd": False, "hide_previews": False}})
        final = await_value(lambda: state("final"), lambda s: not s["notifications"]["dnd"])
        assert final["view"] == initial["view"] and final["cards"] == initial["cards"]
        print("PASS: real notification D-Bus lifecycle, replacement, directed actions, expiry, ownership, disconnect, privacy, DND and bounded capacity")
    finally:
        for fixture in (a, b):
            if fixture is not None:
                fixture.close()
        (args.output / "status.json").write_text(json.dumps(records, indent=2) + "\n")


if __name__ == "__main__":
    main()
