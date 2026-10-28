# Revision 23 protected virtual-key dispatch

[Exact package/source/binary/probe/QA identities and results](evidence.json) separate the initial failure, corrected protocol case, ordinary phone acceptance and QA-controller completion.

The revision-22 host baseline accepts an ordinary virtual key, then delivers synthetic Control+B press/release to the synthetic locker. The regression fails. Revision 23 checks each already-bound virtual key/modifier request against protection and clears retained virtual modifiers when denied. The stronger final test leaves Control active before locking, synchronizes the method and locker, requires no protected key or nonzero modifier, then sends a raw ordinary key after unlock without resetting modifiers. It arrives with zero modifiers.

That stronger case passes on the host and against the exact installed revision-23 binary on the phone, with audited ARM method/locker probes. The phone guard runs as a nested Wayland/EGL compositor inside the actual UID-10000 login cgroup. It proves protocol dispatch, separately from the outer session's physical KMS and fresh real PAM checks. It is not described as a synthetic proof of PAM authentication.

Unmodified actual outer PinePhone Pro/Panfrost/KMS captures retain the existing app behavior on revision 23:

- [GTK3 fullscreen after PAM](gtk-after-pam.png) and [Qt6 fullscreen after PAM](qt-after-pam.png): exact `tarotxza` remains above the real keyboard after hide/reopen, actual overview/card activation and fresh real PAM. Both negotiate 360×444 and restore full height afterward.
- [Real terminal after PAM](terminal-after-pam.png): native touches enter shell commands, Backspace/Enter and UTF-8 `café`; real GTK ↔ foot Unicode/newline clipboard transfer and the same shell after unlock pass. Clipboard paste still uses physical Ctrl+Shift+V; touch terminal controls remain open.

All five native protocol cases/40 driver requests, existing toolkit dialog/menu/state controls, Quick Launch, rounded cards, three/five-member stacks, real GTK editor Save/Cancel and hardware/PAM checks pass in the first native replay. Its controller then creates a tuple-valued JSON key and fails before restart/cleanup. That failed controller and immutable first-run archive remain retained. A separate completion controller revalidates exact installed/first-run hashes, performs a fresh PAM saved-settings restart and restores original preferences and normal Tarot. It does not reinstall the package or rewrite the failed controller as successful.

Independent final audit verifies ordinary default Tarot on UID 10000/VT7, hardware keys, diagnostics disabled, exact loaded binaries, original settings/policies, no fixture/input/RTC alarm and a live Panfrost descriptor. Both replay sessions request zero kernel-suspend cycles. No new physical-finger acceptance is claimed. The three helper payloads remain byte-identical to revision 22.

All 142 ordinary tests pass on current and minimum compilers, strict host/AArch64 DRM checks pass, and lifecycle/state, scale-2 fullscreen/transient plus existing rounded scale-1/2, stack, keyboard, editor, idle, lock and orientation replays pass. One initial unit invocation hits the sandbox's socket-bind restriction; the unchanged suite passes with local socket permission. The exact nine-file local protection patch independently applies to published Smithay 0.7.0. All 330 packaged Rust files match the frozen source; final external probes/Python QA are hashed separately because held-modifier coverage follows freezing. Seventeen executed QA/support files match the collected archive and worktree.

[Policy and remaining work](../../VIRTUAL_KEYBOARD_PROTECTION.md) keep touch terminal controls, broader input/protection/application cases, current-binary endurance and the full development-plan gates open. Full local evidence is under ignored `artifacts/m2/virtual-keyboard-lock-r23/`.
