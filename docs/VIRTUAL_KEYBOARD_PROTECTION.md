# Virtual keyboard protection boundary

Installed and native-validated revision 23, October 28, 2026. This was found while preparing touch terminal modifiers. Terminal controls remain the next input feature; fixing this dispatch path comes first.

## Reproduced path

The normal keyboard alone can bind the privileged virtual-keyboard protocol through its private inherited connection. Registry filtering controls who binds; it does not by itself constrain later requests from that already-bound connection.

On the revision-22 host compositor, an actual trusted method fixture sends Ctrl+B while the synthetic lock surface owns keyboard focus. The locker receives a nonzero Control modifier, key press and key release. The bounded isolated regression fails. These are fixed synthetic events in a private nested session, with no real authentication data. The existing helper UI normally disappears while protected; that client-side behavior is insufficient to enforce the server input boundary.

## Dispatch policy

Tarot's local Smithay patch adds `SeatHandler::allow_virtual_keyboard_input`, defaulting to `true` for compatibility. Tarot returns `false` throughout protection. Every virtual key/modifier request checks the policy before forwarding to the current focus, including previously created protocol objects. Denied requests also zero the virtual keyboard's retained modifier state so stale shortcuts cannot return afterward. Keymap setup and object destruction retain their normal behavior.

Physical keyboard input continues through the existing protected-input route to the real locker. The ordinary input method is not a protected input source. Its text-input commits use the existing separate focus/serial bridge. No privileged protocol is made visible to ordinary clients.

## Acceptance and scope

The actual protocol regression requires an ordinary modified key to arrive, then full protected presentation and locker-side protocol checkpoints. After the method's Wayland sync, fresh checkpoints must show no virtual keys or nonzero virtual modifiers. Control is deliberately left active before locking. Unlock must restore a raw ordinary key with zero modifiers, without sending a fresh modifier/reset request. The corrected host test passes; the original failing result remains separate under ignored `artifacts/m2/virtual-keyboard-lock-r23/`.

Phone acceptance runs the installed signed binary as a nested Wayland compositor inside its ordinary login, using separately audited ARM method/locker probes. This is protocol dispatch and nested EGL evidence, separate from the outer session's physical KMS and actual PAM checks. The same installed package retains the real GTK/Qt fullscreen fields, foot/ash text and clipboard flow, existing native protocol/card/stack/editor/hardware checks, saved restart and independent normal-session/Panfrost cleanup.

The exact nine-file protection patch applies independently against published Smithay 0.7.0 and matches the vendored tree. [Attribution](../third-party/README.md#smithay-session-protection-changes). This is a local protection policy, not an upstream release claim. Revisit it with the other local changes when updating Smithay.

The first native product replay passes every case, then its controller fails to serialize a tuple-valued result key before restart/cleanup. That original failed controller and complete first-run evidence remain immutable. A separate completion controller revalidates exact installed/first-run hashes before fresh PAM restart and normal-session restoration; independent final session/Panfrost audits also pass. The original controller failure is not rewritten as a successful run. [Curated results and exact identities](recordings/r23/README.md). Touch-only terminal controls, other clipboard/application cases, broader input combinations, current-binary endurance and all remaining M0–M6 gates remain open. This does not complete the phone's lock/security or development-plan gates.
