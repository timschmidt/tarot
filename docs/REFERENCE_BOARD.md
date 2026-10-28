# Tarot phone interaction reference board

Updated October 28, 2026. The owner’s [original Nocturne/Pre 2 design packet](Tarot_Design_Packet/README.txt) is now authoritative for Tarot styling and proposed interaction values. The historical research below is supporting context; the earlier 2009 Pre and Pre 3 references must not silently override the chosen Pre 2/webOS 2.x baseline. The precise firmware build remains uncaptured. See the [application and deviation register](DESIGN_APPLICATION.md).

Previously reviewed October 27, 2026. Use the classic Palm phone composition as the baseline, adding later WebOS/LuneOS stacks. This is an observation and adaptation record, not an asset bundle or a claim of finished visual fidelity.

October 28 follow-up: the local revision 10 source now implements the five-position dock, upward-hold selection wave and persistent favorites. Its native-client/keyboard desktop captures were inspected; phone acceptance and final historical styling remain open. [Exact checkpoint](IMPLEMENTATION_STATUS.md#quick-launch-development-checkpoint)

The earlier research reference was Palm's 2009 *Pre User Guide*, hosted by its carrier [Bell](https://support.bell.ca/_web/guides/User-Guides/Mobile/PalmOne/Palm-EN/palm_pre_userguide_en%28en%29.pdf). Page numbers below are the printed page numbers. Selected pages were rendered locally and inspected; the original copyrighted illustrations remain external references. Tarot does not ship them.

| State | Reference | Composition or behavior to preserve | Tarot adaptation / remaining evidence |
| --- | --- | --- | --- |
| Home / no cards | Palm guide pp.17, 23 | Wallpaper, compact status, five-position Quick Launch | Promotional text is removed in local revision 10; four favorite positions and fixed launcher are implemented. Original wallpaper and status/icon treatment still need visual acceptance |
| Focused app | Palm guide pp.30–32 | Status remains above the usable application area | Reserve a software gesture strip; Linux clients retain their own content and menus |
| Overview | Palm guide p.25 | Centered live card, partial neighbors, horizontal browsing | Preserve window identity, animate the native surface tree and keep its input transform aligned |
| Card dismissal | Palm guide p.26 | Upward throw from a card | Request ordinary Linux close; keep a pending client and any confirmation reachable |
| Stacks | [HP Pre3 guide p.43](https://hp.manymanuals.com/pc-workstation-barebones/Pre3/users-guide-1137/43), [LuneOS group view](https://github.com/webOS-ports/luna-next-cardshell/blob/101cb7dbe2e22eb2c9508758165dd634b01b41a2/qml/CardView/CardGroupListView.qml) | Hold and drag to group; members remain individual cards | Manual grouping, member selection/order, extraction and independent close now have phone replay evidence. Initial fan uses wider touch strips and a separate-card rail; historical angle/lift styling and representative reference-image/recording comparison remain acceptance gaps |
| Launcher / search | Palm guide pp.11, 23, 26, 32 | Three-column icons, labels, wallpaper relationship and search | Search and a local persistent favorite editor work. Favorite drag organization, complete app-icon resolution, panel treatment and paging remain |
| Notifications | Palm guide p.32 | A narrow bottom banner leaves the app visible | A functional dashboard and native notification server have phone evidence; historical styling, grouping, icons and durable history remain |
| System menu / lock | Palm guide pp.31 / 16 | Corner menu access; upward lock affordance | Controls and authenticated locking now have native evidence. System-menu illustration includes an app-menu example; expanded controls and complete lock presentation still need visual comparison and broader policy acceptance |

The LuneOS [gesture area](https://github.com/webOS-ports/luna-next-cardshell/blob/101cb7dbe2e22eb2c9508758165dd634b01b41a2/qml/WindowManager/LunaGestureArea.qml) and [swipeable card](https://github.com/webOS-ports/luna-next-cardshell/blob/101cb7dbe2e22eb2c9508758165dd634b01b41a2/qml/CardView/SwipeableCard.qml) provide additional behavioral references. No QML or reference artwork was copied into Tarot. The inspected archive is pinned to the linked commit; runtime code is an independent Rust implementation.

## Superseded scaffold tokens and deliberate departures

These October 27 scaffold values are retained as history, **not current implementation targets**. The owner’s packet supplies the replacement values; neither set represents measured historical firmware:

| Token | Initial value | Reason / review |
| --- | --- | --- |
| Logical phone extent | 360 × 720 at scale 2 | PinePhone Pro's portrait panel |
| Status / software gesture strip | 28 / 28 logical units | Replace the original dedicated gesture hardware without intercepting app scrolling |
| Dock reservation | 76 logical units | Preliminary; favorite artwork and reach testing will refine it |
| Card / neighbor gap | 76% width / 18 logical units | Maintain clear spatial continuity and visible neighbors |
| Touch slop | 10 logical units | Avoid accidental navigation while retaining slow deliberate movement |
| Settle | 220 ms, cubic ease-out | Evaluate from monotonic elapsed time; stop scheduling frames after settling |
| Reduced motion | Immediate settle | Preserve direct finger tracking and explicit navigation controls |
| Minimum new close target | 48 × 48 logical units | Large enough to use without relying on the flick |

The original Home/Cards/Apps scaffold is replaced locally by the five-position dock and wave. Original line artwork covers the classic favorite roles; other application icons still use a provisional fallback. The gradient wallpaper, status text and client decorations remain preliminary. Manual stacks, dashboard, system controls and authenticated locking have functional/native evidence; their visual fidelity remains unaccepted. Final wave styling, favorite drag organization, full typography/icon tokens, accessibility and representative state comparisons remain outstanding. See [interaction design](INTERACTION_DESIGN.md) and [implementation status](IMPLEMENTATION_STATUS.md).
