# Original Tarot runtime artwork

These assets derive from the owner's [design packet](../../../docs/Tarot_Design_Packet/README.txt), committed in `5ee7671`. Original packet files remain unchanged. `scripts/design-assets.py` copies the Nocturne wallpaper and licensed fonts, rasterizes the original icon vectors and extracts the resting tray material. The runtime uses Rust's PNG decoder and egui; CairoSVG/fontconfig are development tools only.

The included OFL notices apply to Lato and Instrument Serif. Lato is the packet's provisional UI substitute; Instrument Serif is reserved for identity. Research fragments and screen mockups are excluded from runtime assets. Calendar date artwork is still illustrative, and CairoSVG filter/shadow matching needs comparison before visual conformance is claimed. `provenance.json` records the original icon hashes and generated outputs.
