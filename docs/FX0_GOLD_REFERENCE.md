# Deferred Fx0 Gold reference

Owner decision, October 30, 2026: retain the supplied packet as reference
material. After all existing development steps, plan Gold as an additional theme
and LG Fx0 as another device, using reusable device profiles and capabilities.
[Deferred M7 plan](DEVELOPMENT_PLAN.md#m7-deferred-extension-gold-theme-and-additional-devices).
Nocturne and PinePhone Pro remain the current implementation baseline.

The supplied [33-page packet](tarot-fx0-gold-design-sources/Tarot_Fx0_Design_Packet.pdf),
[overview](tarot-fx0-gold-design-sources/Tarot_Fx0_Visual_Overview.png),
[README](tarot-fx0-gold-design-sources/README.txt),
[Gold material notes](tarot-fx0-gold-design-sources/GOLD_THEME_NOTES.txt) and
[platform handoff](tarot-fx0-gold-design-sources/handoff/fx0-platform-handoff.txt)
are preserved as supplied. The packet is Gold edition 0.3, dated October 30.
Its tokens, 26 screen studies, motion and hardware layout are proposed targets;
its reported contrast checks and historical platform references do not certify
Tarot runtime behavior or a present-day Fx0 port.

| Reference input | SHA-256 |
| --- | --- |
| Tarot_Fx0_Design_Packet.pdf | `c589b3a10061253c68019cd88be657b59d398d89fdcb8581dfdc9abb4724a26c` |
| tokens/tarot.tokens.json | `1c9740bb50f76f2a36a8dd389cdbfa4705e6d701e3a18b5fbece2ef4f6008251` |
| tokens/fx0-gold.theme.json | `49e84951a3800ee9c42208d0b86afac8ffc18e537d97d77b6ad32745c359ea0d` |

The reference inventory has 136 files. Structural checks confirm 26
paired SVG/PNG screens, 720 × 1280 PNG dimensions, parseable SVG/JSON,
16 icons and 18 glyphs. The supplied authoring scripts are retained; no asset regeneration was run. Font OFL notices and
[historical fragment attribution](tarot-fx0-gold-design-sources/reference/README.txt)
remain with the packet. Historical fragments stay research material.

When M7 starts, recheck platform support against current primary sources and
the actual handset before selecting a graphics, input, wake or telephony path.
The M7 plan separates theme roles from geometry and hardware support so that
Gold can work on other profiles and Fx0 can use other supported themes.
