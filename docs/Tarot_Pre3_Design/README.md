# Tarot / HP Pre 3 design packet

**Edition:** 0.1 — 30 October 2026. Companion to the existing PinePhone Pro Nocturne and LG Fx0 Gold design packets.

## Files

- `Tarot_Pre3_Design_Packet.pdf`: illustrated design, interaction, hardware, port and fidelity handoff.
- `Tarot_Pre3_Visual_Overview.png`: four-screen overview.
- `screens/svg/` and `screens/png/`: 28 editable 360×600 SVG studies and 480×800 PNG exports.
- `icons/`: 15 original pictorial icon studies (editable SVG and PNG).
- `wallpapers/`: native-resolution vector/raster wallpaper sets (Nocturne, Heritage, Vellum, optional Fx0 Gold).
- `brand/`: Tarot wordmark and nonphotographic Pre 3 slider frame.
- `tokens/`: design values and three-device matrix in JSON.
- `handoff/`: port, interaction, legal provenance and historical sources.
- `motion/`: a 480×800 video study of card transition.
- `screens/index.json`: screen manifest.
- `qa/`: generated render verification summary.

## Device-specific decisions

Pre 3 uses **480 × 800 native pixels**, **360 × 600 logical units at 4/3 scale**, a **hardware gesture area** below the LCD, and **physical slide-out QWERTY** as primary text input. Screen-visible software navigation and keyboard are off by default. Keyboard fallback is an explicit 27th study. Quick Launch: five positions total, four editable.

## Assumption

Per the request, **assume a Nura/postmarketOS Pre 3 port exists**. This is not a test report of working GPU, KMS, slider, gesture, modem, suspend or Android drivers. Waydroid on this 512 MB device is deliberately not assumed viable.

## Design status

The PNGs and PDF contain original SVG-based design studies, not historical firmware captures. Layout values, gesture thresholds, timings and memory budgets are *proposed*, and must be compared to a pinned Pre 3 software build and measured on the actual device. Lato is a freely licensed available system font, not included as a redistributed font file; Prelude is a historical comparison target.

## Licensing

Original graphics in this deliverable are newly drawn and suitable for project review; integrating them into a public repository requires the maintainer to choose the intended license. Historical source images were not bundled. See `handoff/source-ledger.md`.

## Regeneration

The PNG screen exports were rendered with CairoSVG. Authoring source for the generated packet is included in the ZIP at `source/build_pre3.py`.
