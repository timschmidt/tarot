# Real terminal and clipboard acceptance

Revision-22 acceptance, October 28, 2026. Revision 23 repeats this real shell/clipboard flow alongside its [virtual-key protection](VIRTUAL_KEYBOARD_PROTECTION.md) checks. This advances the M2 application matrix; it does not complete M2. [Actual captures and exact identities](recordings/terminal-clipboard-r22/README.md).

## Real shell input

The catalog launches installed foot 1.27.0, built with IME and grapheme support, with an ordinary interactive ash shell. The fixture only selects its private working directory and prompt before executing `/bin/sh -i`. It never forwards or fabricates entered commands. All shell commands are typed through the real Rust keyboard using native touch, including quotes, redirects and punctuation.

The replay requires exact files written by ash: `terminal` after deleting an extra character with the keyboard's virtual Backspace, UTF-8 `café` from a native text commit, and `after-pam` after restoring the same shell through real PAM. The keyboard's virtual Enter submits each command. Hide/reopen retains the terminal's focus and reservation. Shell/foot UID, executable identity and login cgroup are independently checked. The application exits through touch-typed `exit`.

## Ordinary clipboard exchange

An external GTK 3.24.52 peer supplies real Copy/Paste widgets using its standard clipboard API. The compositor does not gain a clipboard manager or expose privileged data-control protocols for the test.

1. A native touch activates GTK Copy, supplying `Tarot GTK café → foot` followed by a newline and `second line`.
2. The same foot card is activated. The native keyboard types a quoted `printf` prefix. A temporary physical-keyboard fixture sends Ctrl+Shift+V, then native touches finish and submit the command. Its output file must contain the exact Unicode/newline bytes.
3. Native touches type a standard OSC52 shell command carrying a distinct fixed synthetic string, `Tarot foot café → GTK` followed by the same second line. Foot owns the resulting clipboard source.
4. A native touch activates GTK Paste. Its actual asynchronous clipboard response must equal that string, with a second fresh Paste after real PAM lock/unlock. No direct test callback substitutes a response.

The physical shortcut is explicit acceptance scope. Tarot's touch keyboard does not yet expose the modifier/navigation controls needed for a complete touch-only terminal workflow. OSC52 here is foot's existing behavior and configuration, not a new Tarot clipboard API or a general clipboard-permission policy.

## Session and source evidence

`phone_installed_session_replay.py --terminal-clipboard --cycles 0` adds the new acceptance before its normal Calculator checks. Stage `terminal_clipboard_fixture.py` and `phone_terminal_clipboard_replay.py`, plus the existing touch/physical-keyboard/OCR/PAM helpers. Prepare a UID-10000 `<replay-output>/terminal-clipboard` directory and temporary catalog entries:

- `test.tarot.Terminal.desktop`: `foot --app-id=test.tarot.Terminal --title=Tarot-terminal --font=monospace:size=14 -- python3 /home/user/tarot-dev/terminal_clipboard_fixture.py --mode shell --output <directory>`.
- `test.tarot.Clipboard.desktop`: `python3 /home/user/tarot-dev/terminal_clipboard_fixture.py --mode clipboard --output <directory>`.

The root controller preserves existing settings and login policies, ends only the verified normal Tarot instance, authenticates through normal greetd, runs bounded observations and fresh actual PAM, checks saved settings in a new login and restores the normal default entry with diagnostics disabled. The compositor and applications run as UID 10000 on VT7; root supplies only the temporary guarded input and QA observation. Independent final checks verify exact loaded binaries, preserved preferences/policies, removed entries/processes/input/RTC alarm and a Panfrost descriptor. Both sessions request zero kernel-suspend cycles. No physical-finger acceptance is claimed.

No compiled Rust or package changes are needed for this case. All 330 packaged Rust files match the frozen revision-22 manifest; the final executed QA is separately hashed and retained in the native archive. Earlier revision-22 compiler/protocol/fullscreen checks retain their original scope rather than being presented as newly executed tests.

## Remaining work

Revision 25 adds [touch-only terminal modifiers/navigation/Paste](TERMINAL_CONTROLS.md), including helper recovery and fullscreen/PAM visual acceptance on the same native terminal. The revision-22/23 physical-shortcut evidence above retains its historical scope. Selection gestures, primary selection, other clipboard MIME types and applications, source death/retention policy, broader composition/layouts, browser/accessibility interoperability, current-binary endurance and all remaining development-plan gates remain open. This case verifies clipboard restoration after protection; the earlier dedicated lock fixture supplies cached-offer denial coverage. Do not infer a complete clipboard-security or process-sandbox boundary from these application tests.
