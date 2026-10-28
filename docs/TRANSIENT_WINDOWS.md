# Native transient windows

October 28, 2026: revision 18 is installed and native-tested on the PinePhone Pro. Protocol lifecycle, input/protection and basic GTK/Qt dialog actions pass; visual review identifies Qt initial sizing and popup-overflow work below. This is progress within M2, not completion of M2 or the development plan.

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

Revision 18 is clean-built, signature-checked and payload-audited. Its APK SHA-256 is `53d0623765b4f75e5d5f818694265c1c83074d022dba17ee1f5ad3e0024adb06`; compositor SHA-256 is `db0939338f95b271e34140fb42d44999e7fc713a71f6fc869a1edde1312f1104`. The launcher, locker and keyboard match revision 17 byte-for-byte. An audited ARM integration-test binary runs as a catalog-launched UID-10000 process in the real login cgroup, with actual touch, keyboard and fresh PAM protection. All three native protocol cases pass. A fresh controller reuses that binary-specific evidence without reinstalling or repeating the passing protocol cases, then passes the toolkit and broader regressions.

GTK 3.24.52 and Qt/PySide 6.11.1 separate-dialog fixtures accept exact native-keyboard text, child popup ownership and selection, overview/return, retained modal content on root close, actual Cancel/Apply and final cooperative close. Qt advertises the modal hint; GTK3 relies on toolkit modality. Both filter the root's close before the fixture's close callback while the modal child remains. The original callback expectation is retained as a failed attempt. A second failure captures launch opacity 0.476 with dock opacity 0.524; the final runner waits for complete opacity and dock dismissal. The passing controller, exact executed QA sources, both failures and independent normal-session audit are retained under ignored `artifacts/m2/transients-r18/`. Policies/settings are preserved and default normal Tarot has no diagnostics or test input/alarm. These are zero-kernel-suspend sessions.

Visual review still finds the Qt dialog stretched to the full app area despite its natural-size request. The creation path sends a full-app configure before the client can set its parent; defer that initial configure until the initial commit and add a configure-history regression. The pinned constrain helper also crops each transient tree to its own fitted target before the family mask, so menus extending beyond that rectangle need a failing overflow case and renderer correction. The successful Choice/Other popup cases stay within that target. These gaps remain open rather than being inferred away from functional passes.

[Native captures and identities](recordings/r18/README.md). The fixtures exercise real toolkit widgets; they do not certify arbitrary GTK/Qt applications. GNOME Text Editor's existing Adw close dialog is still a separate in-window result. Broader clipboard, accessibility, activation-token and toolkit cases remain part of M2 and later gates. The API references used for the fixtures are [GTK3 Dialog](https://docs.gtk.org/gtk3/class.Dialog.html), [GTK transient parenting](https://docs.gtk.org/gtk3/method.Window.set_transient_for.html), and [Qt 6.11 QDialog](https://doc.qt.io/qtforpython-6.11/PySide6/QtWidgets/QDialog.html).

Reproduction commands are in [building instructions](BUILDING.md#native-transient-regressions).
