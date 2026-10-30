TAROT Fx0 GOLD DESIGN PACKET
Fx0 Gold edition 0.3 | 30 October 2026

Start with Tarot_Fx0_Design_Packet.pdf. The dedicated handset theme is Fx0 Gold,
using the phone interface of Pre 2 era webOS 2.x as the reference.

This is a researched design proposal. It has not been certified against a
particular captured firmware build or tested as a live compositor on the
user's LG Fx0. Proposed measurements, motion seeds and adaptations
are identified in the packet and token file. All sample contacts and app
content are fictional.

CONTENTS

Tarot_Fx0_Design_Packet.pdf      Complete 33-page visual and behavior specification
Tarot_Fx0_Visual_Overview.png   Four-screen visual overview
screens/svg/               26 editable portrait studies
screens/png/               26 matching 720 x 1280 renders
components/                Default, pressed, disabled, busy and toggle states
icons/                     16 original application icon studies in SVG
glyphs/                    18 UI glyphs in SVG
brand/                     Original Tarot mark and wordmark
wallpapers/                Fx0 Gold, Nocturne, Heritage and Vellum in SVG and PNG
tokens/tarot.tokens.json    Geometry, type, palette, theme, motion and gestures
motion/                    7.7-second card motion study and explanatory notes
handoff/                   General integration notes, Fx0 evidence, fidelity audit
fonts/                     Lato and Instrument Serif, with their OFL notices
reference/                 Four attributed historical research fragments
source-manifest.txt        Complete source URLs and attribution
FX0_CHANGES.txt            Earlier device adaptation from the PinePhone Pro packet
GOLD_THEME_NOTES.txt       Gold material, semantic colors and integration notes
tokens/fx0-gold.theme.json  Standalone resolved Gold theme
source/                    Python and JavaScript authoring scripts

USING THE FILES

SVG screen artboards use 360 x 640 logical coordinates and declare a
720 x 1280 output size. Shapes, filters and text remain editable. Install
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

The source scripts use Python 3, Pillow, reportlab and pypdf. PNG rendering uses
Node.js and sharp. Motion export additionally requires ffmpeg with libx264.
No network access is required after those dependencies are installed.

From this folder, run:
  python3 source/build_assets.py
  node source/render_assets.cjs
  python3 source/build_packet.py
  python3 source/make_overview.py
  python3 source/make_motion.py
  python3 source/check_gold_contrast.py
  python3 source/package_sources.py

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

The PDF and all screens are checked by the accompanying validation summary. The motion study
illustrates proposed transforms and timings; it is not hardware footage.

FX0 INPUT AND PERFORMANCE

Fx0 geometry and physical Home behavior remain as specified in the earlier
Fx0 packet. The physical Home key is outside the screen export. The drawn gold frame is a
schematic for context. The 36-unit on-screen strip still owns Back and wave
gestures. Home bindings are proposals pending verification of real input
events. The hardware key must never bypass lock authentication.

The Fx0 has 1.5 GB RAM. Preview-cache limits are initial design policies, not
measured memory or frame-time results. The included motion study is exported
at 60 fps for inspection; the renderer must query the actual device mode.
The historical madai package does not certify current Wayland readiness.
Read handoff/fx0-platform-handoff.txt before using the platform assumptions.

GOLD THEME

The main screen suite uses Fx0 Gold. The additional reference cards show
Nocturne, Heritage and Vellum with their original palettes. The active theme
and resolved UI roles are exported in tokens/; UI gradients are separated
from pictorial icon gradients to preserve familiar application colors.

Gold is applied to shell surfaces and the sample cooperative app chrome.
Ordinary Wayland clients retain the pixels they render. The design does not
promise automatic recoloring of arbitrary applications.
