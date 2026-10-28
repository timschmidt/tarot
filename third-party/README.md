# Third party source attribution

The compositor event/seat/surface scaffolding and move/resize grabs started from Smithay's `smallvil` example at release v0.7.0 and were adapted for Tarot. Affected files retain attribution comments. The upstream MIT notice is preserved in [Smithay-MIT.txt](Smithay-MIT.txt).

Source: [Smithay smallvil v0.7.0](https://github.com/Smithay/smithay/tree/v0.7.0/smallvil).

The input inverse in `src/focus.rs` follows the pinned renderer utility's Fit/CENTER scaling and alignment rounding so it agrees with the actual native scene. The same upstream MIT notice is retained for this rendering foundation. [Smithay 0.7 render-element utilities](https://docs.rs/crate/smithay/0.7.0/source/src/backend/renderer/element/utils/elements.rs)

The surface-placement traversal in that file follows Smithay's renderer-state traversal and offset conventions. It uses the state supplied by the traversal rather than re-entering the surface lock. [Smithay 0.7 surface utilities](https://docs.rs/crate/smithay/0.7.0/source/src/backend/renderer/utils/wayland.rs)

Tarot's shell model, UI, rendering bridge, and provisioning helpers are new project code. No WebOS/LuneOS code or artwork has been copied into this implementation. Tarot-owned code is licensed under MIT OR Apache-2.0, selected by the owner. Cargo dependencies retain their respective licenses.

## Desktop-entry decoding

`freedesktop-desktop-entry` 0.8.4 is used as a Cargo dependency for desktop-file metadata decoding, with its optional gettext feature disabled. Its upstream license is MPL-2.0; its source has not been copied into Tarot. Tarot's own Exec-to-argv implementation handles field-code expansion separately and never invokes a shell implicitly. [Packaged source and license metadata](https://docs.rs/crate/freedesktop-desktop-entry/0.8.4/source/Cargo.toml)

## Smithay GLES damage backport

`smithay/` is the published Smithay 0.7.0 crate source under its original [MIT license](smithay/LICENSE.txt), with the upstream bounds-clipping backport and local session-protection changes described below. Cargo's local patch uses this source; it is excluded from Tarot's workspace members. Its packaged source checksum was `740cea6927892bc182d5bf70c8f79806c8bc9f68f2fb96e55a30be171b63af98`. The original manifest, source notices, examples and build probes are retained; the upstream lockfile and Cargo's local cache marker are omitted.

The two-line GLES change is by Andy Wright, upstream commit [e76b1eece59f0ccf87a04f7eaa6762e4f6e8dfee](https://github.com/Smithay/smithay/commit/e76b1eece59f0ccf87a04f7eaa6762e4f6e8dfee), “renderer/gles: clamp accumulated shm damage to current buffer size.” Damage accumulated across differently sized commits can exceed a cached texture's current size. The backport intersects each partial SHM upload region with the current buffer. It preserves the pinned release's API and other behavior. [Patch file](smithay-damage-clipping.patch)

The real lifecycle regression now checks hidden buffer growth/shrink, fresh pixels and GL diagnostics. Before the backport it logged a 480-high upload into a 240-high texture; the phone first exposed a 1432-high upload into a 952-high texture during keyboard/launcher transitions. Keep the local backport until a selected upstream release contains the fix, then verify both nested and native regressions before removing it.

## Smithay session-protection changes

[smithay-session-protection.patch](smithay-session-protection.patch) records Tarot's local changes against the published 0.7.0 source. These changes retain Smithay's MIT license and are not described as an exact upstream backport. The patch applies independently of the GLES backport; a private-copy application check verifies all seven resulting source files against the vendored tree.

- `session_lock/lock.rs` returns after posting `InvalidUnlock`, rather than falling through to the compositor's unlock handler. It also removes dead output resource references before checking for duplicate lock surfaces, allowing a replacement trusted connection to recover an orphaned session.
- `session_lock/surface.rs` requires both the Wayland surface and its lock-surface protocol object to remain alive; destroying only the role must select the opaque fallback.
- `selection/mod.rs`, `selection/offer.rs` and the three `selection/data_device` request/grab files add and consult `SelectionHandler::allow_selection_access`. Its default is `true`; Tarot returns `false` while protected, covering receive requests through previously issued clipboard/drag offers and new selection/drag requests. Data already transferred cannot be revoked.

The invalid-unlock regression was run with just the early return removed: a rejected second lock object reached the unlock handler and exposed the ordinary scene/status. The guarded build passes the same test on desktop and phone. Current upstream uses a different lock-owner/state design; compare [the inspected immutable source](https://github.com/Smithay/smithay/blob/6b37edc019889148608a73e3e27423979cd642c8/src/wayland/session_lock/lock.rs). Re-evaluate these local changes when selecting a newer Smithay release, with the real protocol, clipboard and native presentation replays retained.

## Linux-PAM runtime boundary

The trusted lock client's own Rust FFI declarations follow Linux-PAM's public application/conversation ABI; no PAM implementation or helper source was copied into Tarot. The disposable unprivileged worker dynamically loads the system `libpam.so.0` and uses distribution modules/policy. `libloading` and `libc` remain Cargo dependencies with their upstream licenses; the system's PAM libraries/helpers retain their distribution license/notice files. [Public API](https://github.com/linux-pam/linux-pam/blob/master/libpam/include/security/pam_appl.h)

## Native card shader variants

Tarot's [card mask](../src/shaders/card-mask.frag) adapts the sampler, `NO_ALPHA` and `DEBUG_FLAGS` variants from pinned Smithay's `src/backend/renderer/gles/shaders/implicit/texture.frag`. That portion retains the [Smithay MIT notice](Smithay-MIT.txt), also installed with the package. The rounded mask, frame-coordinate mapping and analytic card-shadow material are Tarot additions. Native client buffers remain in the normal surface renderer; the mask does not copy application content into wallpaper or screenshot textures.
