#!/usr/bin/env python3
"""External shell/GTK clipboard QA. Only fixed synthetic data is recorded."""
import argparse
import json
import os
from pathlib import Path
import time

GTK_TEXT = 'Tarot GTK café → foot\nsecond line'
FOOT_TEXT = 'Tarot foot café → GTK\nsecond line'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mode', choices=['shell', 'clipboard'], required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assert os.geteuid() == 10000
    output = args.output.resolve()
    assert output.is_dir() and output.stat().st_uid == os.geteuid()
    if args.mode == 'shell':
        # Actual interactive ash owns the PTY. Nothing forwards or fabricates
        # its input: native keyboard/clipboard bytes must reach foot and ash.
        shell = output/'shell'
        shell.mkdir(mode=0o700)
        ready = dict(pid=os.getpid(), foot_pid=os.getppid(), uid=os.geteuid(),
                     cgroup=Path('/proc/self/cgroup').read_text(), shell='/bin/sh')
        (output/'shell-ready.json').write_text(json.dumps(ready)+'\n')
        os.chdir(shell)
        env = dict(os.environ, ENV='/dev/null', PS1='tarot$ ', HISTFILE='/dev/null')
        os.execve('/bin/sh', ['/bin/sh', '-i'], env)
    import gi
    gi.require_version('Gtk', '3.0')
    from gi.repository import Gtk, Gdk, GLib
    log = (output/'clipboard-events.jsonl').open('x', buffering=1)
    def emit(event, **fields):
        log.write(json.dumps(dict(event=event, pid=os.getpid(), uid=os.geteuid(),
                                 monotonic=time.monotonic(), **fields))+'\n')
    css = Gtk.CssProvider()
    css.load_from_data(b'* { font-family: Lato; font-size: 20px; } button { min-height: 44px; }')
    Gtk.StyleContext.add_provider_for_screen(Gdk.Screen.get_default(), css, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
    window = Gtk.Window(title='Tarot clipboard peer')
    window.set_default_size(340, 580)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=18)
    box.set_border_width(12)
    box.set_valign(Gtk.Align.CENTER)
    window.add(box)
    box.pack_start(Gtk.Label(label='GTK clipboard bridge'), False, False, 0)
    clipboard = Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD)
    view = Gtk.Label(label='Waiting')
    view.set_line_wrap(True)
    copy = Gtk.Button(label='Copy')
    paste = Gtk.Button(label='Paste')
    def copied(_):
        clipboard.set_text(GTK_TEXT, -1)
        emit('copy', text=GTK_TEXT)
    def pasted(_):
        def received(_clipboard, text, _data):
            view.set_text(text if text is not None else 'No clipboard text')
            emit('paste', text=text)
        clipboard.request_text(received, None)
    copy.connect('clicked', copied)
    paste.connect('clicked', pasted)
    for widget in [copy, paste, view]:
        box.pack_start(widget, False, False, 0)
    window.connect('delete-event', lambda *_: (emit('close'), Gtk.main_quit(), False)[-1])
    window.show_all()
    GLib.timeout_add_seconds(180, lambda: (emit('timeout'), Gtk.main_quit(), False)[-1])
    emit('ready', version='.'.join(map(str, [Gtk.get_major_version(), Gtk.get_minor_version(), Gtk.get_micro_version()])))
    Gtk.main()
    emit('exit')


if __name__ == '__main__':
    main()
