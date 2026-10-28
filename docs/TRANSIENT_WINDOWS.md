# Native transient windows

October 28, 2026: implemented and tested on the nested GLES backend. The phone still runs validated revision 17; this change has not yet been packaged or accepted on Panfrost/KMS. This is progress within M2, not completion of M2 or the development plan.

## Card ownership and lifetime

Every xdg toplevel keeps its own stable native `WindowId`, surface tree, buffers, configure sequence and input coordinates. A mapped family root supplies the card's identity and metadata. Parented toplevels belong to that card. Identical application IDs do not combine unrelated roots.

`xdg_toplevel.set_parent` takes effect immediately. Only a mapped parent is accepted; an unmapped parent acts as null. Explicitly clearing a parent creates an independent card if the child is mapped. Unmapping or destroying a parent reparents its direct children to its former parent, or detaches them if there is none. Remapping does not resurrect discarded relationships. Surviving native windows retain their identities and are never killed merely because a parent disappeared.

The pinned Smithay handler validates parent cycles but does not implement all of this mapped-parent policy. Tarot maintains its effective graph and updates Smithay's role-parent attributes when relationships change, so later protocol validation observes the same graph. See [family state](../src/families.rs), [Smithay parent handling](../third-party/smithay/src/wayland/shell/xdg/handlers/surface/toplevel.rs) and [commit hooks](../third-party/smithay/src/wayland/shell/xdg/mod.rs). No new vendored patch is required.

## Presentation and input

The family shares one card pose, rounded clipping boundary and outer shadow. A transient retains its natural window geometry, centered within the card, uniformly fitted down when it exceeds the available app area. It receives app-area configure bounds and a zero suggested size so the client chooses its natural size. Independent roots retain the existing full-app configure policy. This initial mobile placement policy also centers auxiliary/tool windows; manual dialog positioning is not implemented.

Each native member has its own fit transform and cached subsurface/popup origins. Its rotation uses the exact physical center of the parent card's clip, including output rounding. Children remain above ancestors; raising a nonmodal member preserves that ordering. The same family appears in overview, transitions and the existing stack renderer without extra dialog cards. Callback and idle-inhibitor visibility use the family card, while actual render visibility and live buffers remain required for inhibition.

The active card restores its preferred native member. Mapping a dialog in another card does not switch cards. Mapping while a shell panel or session protection owns focus cannot take keyboard focus. An active window that gains a parent retains native focus while its card identity moves to the family root. Removing a focused child restores a surviving family member.

`xdg_wm_dialog_v1` supplies the separate modal hint. A parent relationship alone is nonmodal: clicking an exposed parent focuses it and typing follows that focus. A modal branch stays above nonmodal siblings. Tarot's mobile policy blocks input to the rest of that card family until the modal branch is addressed; independent cards and shell navigation remain available. Popups retain Smithay's hierarchy/grab validation, use the card as ownership identity and restore the preferred native family member after dismissal.

Closing a card sends `xdg_toplevel.close` to its family root. The client can refuse or present confirmation; Tarot retains the card and all live family surfaces. It does not synthesize a dialog response or terminate the client's processes.

## Evidence and limits

The initial actual Wayland regression fails against revision 17 because a mapped child creates two cards. The corrected lifecycle regression passes:

- First-buffer mapping and deferred focus; nested modal dialogs; a later nonmodal sibling; overview/return and cooperative root close refusal.
- Child unmapping and focus restoration; hidden-family mapping; shell-panel focus isolation.
- Late attach/detach of an already mapped window, preserved identity and focus, inherited orphan parenting and permanent reset of an unmapped parent.
- Existing hidden SHM grow/shrink/new-pixel, capture-error recovery, callback pacing and disconnect cases.

The dedicated fixture paints a real green parent and red dialog with nonzero window-geometry origin. Actual XTEST clicks and keys target only Tarot's window on an owned private Xvfb. At scales 1 and 2, in all four orientations, it checks native app/overview pixels, client-received coordinates, modal parent blocking, nonmodal parent/dialog focus, actual keyboard delivery and restoration from overview. A separate trusted protocol-lock probe verifies protected mapping, diagnostic redaction and family restoration after unlock. These are Wayland/GLES/protocol-lock results; they do not establish native touchscreen, Panfrost/KMS, physical buttons or PAM acceptance.

The current and minimum Rust compilers pass all 141 ordinary workspace tests. Strict host/AArch64 DRM Clippy and the Rust 1.89 AArch64 all-target check pass. Existing stack, rounded-card, Quick Launch/keyboard, idle, lock-policy and orientation replays pass independently. Original failed attempts remain in ignored `artifacts/m2/transients/`, including the late-parent focus failure and the initial pixel sample outside the fixture's letterboxed parent. The latter required a test-sample correction; fitting behavior did not change.

Before claiming this feature on the device: build/sign/audit the next APK; run the lifecycle and transient input/pixel cases in the real unprivileged login session; verify native keyboard and child popup ownership; exercise separate GTK and Qt toplevel dialogs; repeat protected/PAM/session cleanup and audit normal default Tarot. The existing GNOME Text Editor acceptance uses an in-window Adw dialog and does not prove separate-toplevel compatibility. Broader clipboard, accessibility, activation-token and toolkit cases remain part of M2 and later gates.

Reproduction commands are in [building instructions](BUILDING.md#native-transient-regressions).
