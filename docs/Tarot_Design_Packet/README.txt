TAROT DESIGN PACKET
Version 0.1 | 28 October 2026

Start with Tarot_Design_Packet.pdf. The recommended direction is Nocturne,
using the phone interface of Pre 2 era webOS 2.x as the reference.

This is a researched design proposal. It has not been certified against a
particular captured firmware build or tested as a live compositor on the
user's PinePhone Pro. Proposed measurements, motion seeds and adaptations
are identified in the packet and token file. All sample contacts and app
content are fictional.

CONTENTS

Tarot_Design_Packet.pdf      Complete 31-page visual and behavior specification
Tarot_Visual_Overview.png   Four-screen visual overview
screens/svg/               25 editable portrait studies
screens/png/               25 matching 720 x 1440 renders
components/                Default, pressed, disabled, busy and toggle states
icons/                     16 original application icon studies in SVG
glyphs/                    18 UI glyphs in SVG
brand/                     Original Tarot mark and wordmark
wallpapers/                Nocturne, Heritage and Vellum in SVG and PNG
tokens/tarot.tokens.json    Geometry, type, palette, theme, motion and gestures
motion/                    7.7-second card motion study and explanatory notes
handoff/                   Engineering boundary map and reference-capture audit
fonts/                     Lato and Instrument Serif, with their OFL notices
reference/                 Four attributed historical research fragments
source-manifest.txt        Complete source URLs and attribution
source/                    Python and JavaScript authoring scripts

USING THE FILES

SVG screen artboards use 360 x 720 logical coordinates and declare a
720 x 1440 output size. Shapes, filters and text remain editable. Install
the included fonts or select them in your vector editor before editing.
SVGs use live text rather than outlined text so they remain useful for
localization and type matching. PNGs preserve the intended packet rendering.

Lato is an openly licensed provisional substitute. Prelude is the historical
font target. Final production typography needs a metric and visual comparison.
Instrument Serif is reserved for Tarot identity, not ordinary shell controls.
Calendar date art is illustrative; the date should be rendered dynamically
where that behavior is implemented. Generic third-party app icons can retain
their own native artwork rather than being forced into a shared tile mask.

Tokens are proposals, not a claim about the user's existing Rust code. Keep
logical layout separate from physical buffers. Semantic action colors,
contrast and gesture ownership are invariant across themes. Small text should
not blindly inherit a wallpaper or decorative accent color.

The Email, Calendar, Web and Messaging interiors demonstrate cooperative
application styling. Ordinary Wayland clients render their own contents;
matching their interiors or exposing Back/app menus requires cooperation.

REBUILDING THE DESIGN ASSETS

The source scripts use Python 3, Pillow and reportlab. PNG rendering uses
Node.js and sharp. Motion export additionally requires ffmpeg with libx264.
No network access is required after those dependencies are installed.

From this folder, run:
  python3 source/build_assets.py
  node source/render_assets.cjs
  python3 source/build_packet.py
  python3 source/make_overview.py
  python3 source/make_motion.py

Install Python requirements from source/requirements.txt. Install the Node
dependency in source/ using its package.json. The rasterizer creates a local
fontconfig file in tmp/ to use the supplied typefaces on Linux. On another
platform, install these fonts in the normal system font location if needed.
The motion script currently expects ffmpeg at /usr/bin/ffmpeg; adjust that
local path if the executable is installed elsewhere.

PROVENANCE

The Tarot screen artwork, wallpapers, mark, icons and glyphs were newly drawn
for this design packet. Historical screenshot fragments are attributed in
reference/ and the PDF and are not intended as runtime assets. No extracted
Palm font files or runtime asset binaries are included. Third-party font
notices are preserved in fonts/. The source manifest supplies exact links.

The PDF and all screens were rendered and visually reviewed. The motion study
illustrates proposed transforms and timings; it is not hardware footage.
