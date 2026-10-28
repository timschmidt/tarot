# Fullscreen text input

Installed and native-validated revision 22, October 28, 2026. This extends the [window-state policy](WINDOW_STATES.md).

## Reproduced obstruction

A real GTK3 bottom field in the installed revision-21 fullscreen client occupies y=624–664 in a 360×720 allocation. Native touch opens Tarot's trusted keyboard at y=444, but the fullscreen viewport remains 720 units tall. The field is covered. The bounded baseline fails its viewport predicate and returns to normal Tarot with original settings and policies preserved. Actual captures, toolkit bounds, loaded hashes, exact executed QA and independent normal-session/Panfrost audits remain under ignored `artifacts/m2/fullscreen-ime-r22/`.

## Mobile viewport policy

The trusted input panel reserves space for its owning fullscreen card family. On the portrait PinePhone Pro, that changes the suggested fullscreen client size from 360×720 to 360×444 while the 240-unit keyboard and 36-unit gesture strip occupy the bottom. The fullscreen state stays set and the status bar stays hidden. Hidden fullscreen clients retain their full-output configuration. Natural children and popup constraints use their owning family's viewport/input area.

When the keyboard hides, the full viewport returns. Overview hides the ordinary keyboard; returning to the same focused field restores its reservation. Lock protection continues to use its separate rendering and input path. New geometry is negotiated through ordinary xdg configure/ack/commit. During viewport transitions the existing native fit keeps old buffers usable until the client supplies its resized content.

Ordinary diagnostics report committed native geometry and keyboard ownership for acceptance checks. Protected status keeps its existing three-field response; these ordinary details are not exposed while locked. The normal installed entry still has diagnostics disabled.

## Device acceptance

The GTK3/Qt6 fixtures expose a real bottom entry only in fullscreen. Input comes from actual native touch and the real Rust keyboard. The passing replay requires:

- The rendered viewport and committed native client geometry both end at the keyboard's top, with the fullscreen flag retained and keyboard owner equal to the current card.
- The toolkit's own field rectangle is fully inside that viewport, and actual key touches deliver exact synthetic text to that field.
- Hiding/reopening the keyboard restores 720/444-unit viewports without losing text or focus.
- Actual gesture-strip overview/card activation and real PAM lock/unlock restore the same field, followed by fresh successful typing.
- Existing dialog/menu/state controls, protocol/input/pixel cases, saved restart and normal-session cleanup still pass on the final package.

Both toolkits pass with exact final synthetic text `tarotxza`. GTK reports its resized field at y=348–388 and Qt at y=337–377, with both parent allocations 360×444. All five native protocol cases and 40 driver requests, existing shell/hardware checks and saved restart pass on the final signed package. Independent audit verifies the ordinary default UID-10000/VT7 session, diagnostics disabled, original settings/policies, cleanup and Panfrost. Both sessions request zero kernel-suspend cycles. [Actual captures and exact package/source/QA identities](recordings/r22/README.md).

Current/minimum-compiler tests, strict host/ARM checks, lifecycle/state cases, scale-2 fullscreen/transient pixels/input in every orientation and existing rounded scale-1/2, card/stack/keyboard/editor/idle/lock/orientation checks pass. All 330 frozen Rust files match the worktree; exact executed QA is reconciled separately because a caret-safe touch adjustment followed package-source freezing. Broader landscape/size constraints, cursor/candidate behavior, locales, terminal/browser/clipboard/accessibility coverage and the remaining development-plan gates stay open.
