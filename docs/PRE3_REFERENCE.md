# Deferred Pre3 reference

Owner instruction, October 30, 2026: queue the supplied HP Pre3 design files
**after Fx0**. The [M8 extension](DEVELOPMENT_PLAN.md#m8-deferred-extension-hp-pre3)
follows the complete M7 Fx0 work and all preceding development. The packet is
reference material now; current Nocturne/PinePhone Pro behavior is unchanged.

The supplied [34-page packet](Tarot_Pre3_Design/Tarot_Pre3_Design_Packet.pdf),
[overview](Tarot_Pre3_Design/Tarot_Pre3_Visual_Overview.png),
[README](Tarot_Pre3_Design/README.md),
[port/input handoff](Tarot_Pre3_Design/handoff/pre3-port-and-input.md),
[interaction audit](Tarot_Pre3_Design/handoff/interaction-and-fidelity.md) and
[source ledger](Tarot_Pre3_Design/handoff/source-ledger.md) remain unmodified.
The owner selected [MIT OR Apache-2.0](Tarot_Pre3_Design/LICENSE.md) for the
original packet graphics and authoring source before public inclusion.

Edition 0.1 proposes a 360 × 600 logical layout at 4/3 scale for 480 × 800
output, a physical QWERTY slider and hardware gesture area, without a permanent
software keyboard or on-screen gesture strip. It uses a Pre3/webOS 2.2 reference
direction rather than silently inheriting Pre 2 or Fx0 controls. Geometry,
motion, palettes and memory limits are proposed studies, not calibrated runtime
values. Its assumption of a Nura port is not evidence of working drivers.

The original inventory contains 109 files (7,800,244 bytes). Structural checks
confirm 28 paired SVG/PNG studies with 360 × 600 view boxes and 480 × 800 PNG
exports, 15 paired pictorial icons, parseable SVG/JSON, four wallpaper variants
and the supplied motion video. PDF metadata confirms 34 pages. These are asset
checks, not historical fidelity, accessibility or device acceptance. Authoring
scripts were not executed and assets were not regenerated. The additional
license file records the owner's selection separately from the originals.

| Supplied reference | SHA-256 |
| --- | --- |
| Tarot_Pre3_Design_Packet.pdf | `09a1298357cd76a55319e0ea5b8ce3018dd91d732e443ecc3ea08d9b6e5a3c15` |
| tokens/pre3-nocturne.tokens.json | `97c67be64180a8b243a6d6649c2265d362a0563b4be074c82e6a1660a6e3f420` |
| tokens/device-profiles.json | `9b455b3a29917a800629be088f8ae4ae8f4a64c8e296b897099ff45e4846e2d3` |

At M8, verify the actual handset, platform support and recovery route, then use
the shared theme/profile/capability boundary established for Fx0. Accept native
rendering, fractional scaling, physical input, protected authentication, phone
services and resource budgets with separate Pre3 evidence. A supported Fx0 or
an attractive drawing does not establish Pre3 support.
