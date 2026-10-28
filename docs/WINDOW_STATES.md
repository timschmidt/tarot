# Native window-state policy

Installed and native-validated revision 21, October 28, 2026. This work extends the [transient policy](TRANSIENT_WINDOWS.md); it does not close M2.

## Requests and presentation

Each native toplevel keeps requested maximize/fullscreen state separately from the committed server state. Requests before the initial bufferless commit update pending configuration without sending a premature configure. After that handshake, every maximize/unmaximize/fullscreen/unfullscreen request receives a configure, including repeated requests and rejected changes. Only the client's acknowledged surface commit changes fullscreen presentation. Requests do not activate a hidden card or replace shell/protected focus.

Independent roots accept maximization within the ordinary app work area. Unmaximizing clears that state while retaining the mobile shell's app-area placement; this does not create a freely positioned desktop window. Root fullscreen uses the entire selected output in logical coordinates. The single-output backend chooses its active output; a client's output preference does not move focus. Maximization requested during fullscreen determines the state restored afterward.

Effective parented windows keep natural dialog sizing and reject maximize/fullscreen with an explicit configure after the handshake. Becoming a transient or unmapping clears retained requests. A remapping surface waits for a fresh initial commit and starts without the previous state. Supported capabilities are assigned per window: maximize, fullscreen and minimize for roots, none for transients. The unimplemented compositor window menu is not advertised.

Minimizing the active root enters card overview without closing or unmapping its native surfaces. A hidden root, transient, unmapped surface or protected session cannot use minimization to change the current view.

The [xdg-shell specification](https://raw.githubusercontent.com/wayland-mirror/wayland-protocols/main/stable/xdg-shell/xdg-shell.xml) defines the configure/ack/commit negotiation, permits compositor policy to reject state requests, discards state on unmap, and requires fill behind transparent fullscreen content. Tarot's placement, capability and navigation choices above are mobile shell policy.

## Fullscreen rendering and navigation

Fullscreen hides the ordinary status bar. The visible gesture strip remains a shell overlay with exclusive input ownership; hiding it requires the separate edge-reveal design. The trusted keyboard and notification/system overlays also remain under shell control. The normal app viewport still resizes for the keyboard; fullscreen currently retains its full-output viewport beneath the keyboard. Fullscreen text-field occlusion and broader IME accommodation remain follow-up work.

Native trees and inverse input use the same per-window geometry. Fullscreen cards expand toward the full output as they activate and return to the normal card pose in overview. A black backing prevents transparent client pixels and letterboxing from exposing wallpaper or unrelated clients. It must share the card's rounded/rotated texture mask, opacity and output clipping. A solid-color element cannot use that mask's texture-program override; the first prototype exposes square black corners. A failing pixel assertion reproduces this. The corrected backing uses a cached one-pixel black texture through the same rounded/rotated mask, and the assertion passes.

Protection retains its existing independent rendering path: fullscreen never draws over the real locker. Unlock restores the same native root and its committed state. Popup placement constrains usable input above the keyboard and gesture strip while retaining the owning fullscreen tree's coordinate system.

## Acceptance record

The real state-hint regression fails against revision 20: `set_maximized` sends a configure before the first commit. Revision 21 passes all four pre-commit request types, natural-dialog rejection replies, capability checks, initial fullscreen and hidden-card focus preservation. Initial fullscreen/unfullscreen acknowledgments also leave the displayed geometry unchanged until a surface commit.

Nested scale-1/2 acceptance passes transparent fullscreen pixels, top/bottom client coordinates, actual gesture-strip/card navigation in all orientations, managed protocol-lock restoration, restored maximization, minimization and state reset on unmap. A maximized fixture supplies the configured window geometry. The corner assertion fails on the solid backing and passes on the corrected masked texture, including a post-unlock pixel check. All 142 ordinary tests pass on the current and minimum compilers; strict host/ARM Clippy and minimum-compiler ARM checks pass. Existing upstream warnings remain. Existing rounded-card, stack, Quick Launch/keyboard, real GTK editor, idle, lock-policy and orientation replays pass.

The clean-built, signed revision-21 package passes all five ARM protocol cases on Panfrost/KMS in a real UID-10000 login, with 40 actual driver requests and fresh PAM. GTK 3.24.52 and Qt/PySide 6.11.1 accept native touches on real Maximize, Fullscreen and Restore widgets and report 360×720 fullscreen allocations. Actual frames and geometry checks accompany those events. Existing dialog/menu/text/close cases and natural dialog response sizes remain verified. The same controller passes the card/stack/Quick Launch/editor/hardware checks and saved restart. Both replay sessions request zero kernel-suspend cycles.

Independent final audit verifies exact loaded hashes, ordinary UID 10000/VT7, default Tarot, hardware keys, diagnostics disabled, original settings/policies, no fixture/input/RTC alarms and a live Panfrost descriptor. All 330 packaged Rust files match the frozen source manifest; exact executed QA hashes match the native archive. [Curated native captures and identities](recordings/r21/README.md). Full local evidence and the separate earlier failures remain under ignored `artifacts/m2/window-state-r21/`.

## Remaining cases

- Fullscreen IME accommodation, text-field occlusion, purpose/candidate behavior and additional keyboard layouts.
- Broader min/max-size constraints, interactive move/resize requests, dialogs/popups within fullscreen families, fullscreen stack combinations and output changes.
- Real terminal, clipboard, browser and accessibility coverage, plus independent per-application exceptions.
- Current-binary device endurance and all remaining M0–M6 gates. This checkpoint does not complete M2 or the development plan.
