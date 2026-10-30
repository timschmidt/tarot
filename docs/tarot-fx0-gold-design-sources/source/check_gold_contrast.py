#!/usr/bin/env python3
"""Check selected Fx0 Gold design-token pairs using the Python standard library.

Run from any directory:
    python source/check_gold_contrast.py

The default input is tokens/fx0-gold.theme.json. Results are written to
handoff/gold-contrast-checks.json and handoff/gold-contrast-notes.txt.
This is a palette and gradient-stop check, not whole-app certification.
"""

import argparse
from collections import defaultdict
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import sys
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parents[1]
HEX_COLOR = re.compile(r"^#[0-9A-Fa-f]{6}$")
SOURCES = [
    "https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html",
    "https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html",
    "https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html",
]


def rgb(color):
    """Return unrounded 0..255 sRGB channels for a hex color or RGB triple."""
    if isinstance(color, str):
        if not HEX_COLOR.fullmatch(color):
            raise ValueError(f"Expected a six-digit hex color, got {color!r}")
        return tuple(int(color[i:i + 2], 16) for i in (1, 3, 5))
    if len(color) != 3 or any(not 0 <= channel <= 255 for channel in color):
        raise ValueError(f"Invalid RGB channels: {color!r}")
    return tuple(float(channel) for channel in color)


def luminance(color):
    channels = [channel / 255 for channel in rgb(color)]
    linear = [
        channel / 12.92 if channel <= 0.04045
        else ((channel + 0.055) / 1.055) ** 2.4
        for channel in channels
    ]
    return sum(weight * channel for weight, channel in
               zip((0.2126, 0.7152, 0.0722), linear))


def contrast(foreground, background):
    values = sorted((luminance(foreground), luminance(background)))
    return (values[1] + 0.05) / (values[0] + 0.05)


def composite_srgb(overlay, opacity, background):
    """Source-over composition in sRGB channels; retain fractional channels."""
    if not 0 <= opacity <= 1:
        raise ValueError("Scrim opacity must be between 0 and 1")
    return tuple(a * opacity + b * (1 - opacity)
                 for a, b in zip(rgb(overlay), rgb(background)))


def inspect_launcher_scrim(path, color, expected_opacity):
    """Check the exported study, without executing its asset generator."""
    if not path.exists():
        return {"status": "not_checked", "reason": "Launcher SVG is absent."}
    candidates = []
    for element in ET.parse(path).getroot().iter():
        if element.tag.rsplit("}", 1)[-1] != "rect":
            continue
        if element.get("fill", "").upper() != color.upper():
            continue
        if (element.get("x") == "0" and element.get("y") == "32"
                and element.get("width") == "360"
                and element.get("height") == "572"):
            candidates.append(float(element.get("fill-opacity", "1")))
    matched = any(abs(value - expected_opacity) < 1e-12 for value in candidates)
    return {
        "status": "matched" if matched else "mismatch",
        "artifact": "screens/svg/03-launcher.svg",
        "expected_color": color,
        "expected_opacity": expected_opacity,
        "observed_matching_rect_opacities": candidates,
        "scope": "One exported portrait study; not a runtime compositor test.",
    }


def build_report(theme_path, scrim_opacity=None):
    raw = theme_path.read_bytes()
    theme = json.loads(raw)
    roles = theme["role_tokens"]
    wallpaper = theme["wallpaper"]
    scrim_metadata = theme.get("compositing", {}).get("launcher_scrim", {})
    if scrim_opacity is not None:
        opacity_source = "Explicit --launcher-scrim-opacity override"
    elif "opacity" in scrim_metadata:
        scrim_opacity = float(scrim_metadata["opacity"])
        opacity_source = "compositing.launcher_scrim.opacity in the standalone theme JSON"
    else:
        scrim_opacity = 0.5
        opacity_source = "Fallback design assumption of 0.50; theme JSON does not declare opacity"
    used_roles = {}
    checks = []

    def role(key):
        value = roles[key].upper()
        rgb(value)
        used_roles[key] = value
        return value

    def add(identifier, group, foreground_key, background_key,
            threshold=4.5, required=True):
        foreground = role(foreground_key)
        background = role(background_key)
        record(identifier, group, foreground, background, threshold, required,
               foreground_key, background_key)

    def record(identifier, group, foreground, background, threshold,
               required, foreground_source, background_source):
        value = contrast(foreground, background)
        checks.append({
            "id": identifier,
            "group": group,
            "foreground_source": foreground_source,
            "background_source": background_source,
            "foreground": foreground,
            "background": background,
            "ratio": value,
            "ratio_display": f"{value:.6f}:1",
            "threshold": threshold,
            "required": required,
            "passes": value >= threshold,
        })

    add("body_primary", "Body text", "app_header.title", "email_content.surface")
    add("body_secondary", "Secondary text", "app_header.action", "email_content.surface")
    for stop in ("start", "middle", "end"):
        add(f"header_primary_{stop}", "Header primary text", "app_header.title",
            f"gradient.chrome.{stop}")
        add(f"header_secondary_{stop}", "Header secondary text", "app_header.action",
            f"gradient.chrome.{stop}")
        add(f"focus_header_{stop}", "Focus on light header", "focus.on_light",
            f"gradient.chrome.{stop}", 3.0)

    for stop in ("start", "high", "low", "end"):
        add(f"button_primary_{stop}", "Light primary button text",
            "components.button_label", f"gradient.button.{stop}")
    add("pressed_button", "Pressed button text", "components.pressed_label",
        "components.pressed_surface")

    for stop in ("start", "end"):
        add(f"menu_text_{stop}", "Dark menu text", "full_app.menu_label",
            f"gradient.menu.{stop}")
        add(f"menu_secondary_{stop}", "Dark menu secondary text",
            "full_app.menu_secondary", f"gradient.menu.{stop}")
        add(f"focus_menu_{stop}", "Focus on dark menu", "focus.on_dark",
            f"gradient.menu.{stop}", 3.0)

    for stop in ("start", "middle", "end"):
        add(f"on_label_{stop}", "Gold ON and primary action text", "switch.on_label",
            f"gradient.accent.{stop}")
        add(f"focus_accent_{stop}", "Focus inside gold control", "focus.on_dark",
            f"gradient.accent.{stop}", 3.0)
        add(f"answer_label_{stop}", "Answer action text", "incoming.answer_label",
            f"gradient.success_action.{stop}")
        add(f"decline_label_{stop}", "Decline action text", "incoming.decline_label",
            f"gradient.destructive_action.{stop}")
        add(f"end_call_label_{stop}", "End call action text", "ongoing_call.end_label",
            f"gradient.destructive_action.{stop}")

    add("off_settings", "Neutral OFF label", "switch.off_label",
        "group_box.switch_off_surface")
    add("off_component", "Neutral OFF label", "switch.off_label",
        "components.switch_off_surface")
    add("state_explanation", "Control state explanation", "components.label",
        "components.surface")
    add("pending_indicator", "Pending indicator", "components.pending_indicator",
        "components.surface", 3.0)
    add("focus_ivory", "Focus on ivory", "focus.on_light", "email_content.surface", 3.0)
    add("network_glyph", "Status network symbol", "status.network_glyph",
        "status.background", 3.0)
    add("battery_charge", "Status battery fill", "status.battery_charge",
        "status.background", 3.0)

    # This deliberately treats the glow token as fully opaque. The current
    # wallpaper blends it at lower opacity over darker colors, so this is a
    # conservative color bound, not a screenshot sample or a pixel certificate.
    scrim_color = role("launcher.scrim")
    if "color" in scrim_metadata and scrim_metadata["color"].upper() != scrim_color:
        raise ValueError("compositing.launcher_scrim.color differs from role_tokens launcher.scrim")
    brightest_glow = wallpaper["glow"].upper()
    composite = composite_srgb(scrim_color, scrim_opacity, brightest_glow)
    for identifier, group, key, threshold in (
        ("launcher_scrim_text", "Launcher scrim text", "launcher.app_label", 4.5),
        ("launcher_scrim_focus", "Launcher scrim focus", "focus.on_dark", 3.0),
    ):
        record(identifier, group, role(key), list(composite), threshold, True,
               key, "launcher.scrim over fully opaque wallpaper.glow")

    # Context checks document why the bright wallpaper needs a scrim. These
    # combinations are not approved usages and do not fail the checker exit code.
    for name in ("top", "mid", "bottom", "glow"):
        record(f"raw_wallpaper_{name}", "Unscrimmed wallpaper context",
               role("launcher.app_label"), wallpaper[name].upper(), 4.5, False,
               "launcher.app_label", f"wallpaper.{name}")

    required = [item for item in checks if item["required"]]
    scrim_check = inspect_launcher_scrim(
        theme_path.parent.parent / "screens/svg/03-launcher.svg",
        scrim_color, scrim_opacity,
    )
    failures = [item["id"] for item in required if not item["passes"]]
    return {
        "schema_version": 1,
        "generated_utc": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "theme_name": theme.get("name", "Fx0 Gold"),
        "theme_key": theme.get("theme_key"),
        "input_file": str(theme_path),
        "input_sha256": hashlib.sha256(raw).hexdigest(),
        "scope": "Selected design-token pairs, gradient stops, and one conservative scrim composite; not whole-app certification.",
        "method": {
            "formula": "(lighter relative luminance + 0.05) / (darker relative luminance + 0.05)",
            "srgb_linearization_breakpoint": 0.04045,
            "text_threshold": 4.5,
            "meaningful_graphic_and_focus_threshold": 3.0,
            "comparison_uses_unrounded_ratios": True,
            "gradient_policy": "Check supplied endpoints and intermediate stops; no rendered glyph or gradient-interior sampling.",
            "scrim_composition_space": "sRGB channel source-over; fractional channels retained",
            "launcher_scrim_opacity": scrim_opacity,
            "launcher_scrim_opacity_source": opacity_source,
            "brightest_assumed_wallpaper_color": brightest_glow,
        },
        "summary": {
            "required_count": len(required),
            "required_passed": len(required) - len(failures),
            "required_failed": failures,
            "context_count": len(checks) - len(required),
            "all_required_pairs_pass": not failures,
            "launcher_export_matches_scrim_assumption": scrim_check["status"] == "matched",
        },
        "launcher_scrim_export_check": scrim_check,
        "used_role_tokens": dict(sorted(used_roles.items())),
        "wallpaper_tokens": wallpaper,
        "checks": checks,
        "limitations": [
            "No whole-app, WCAG, runtime, or device certification is claimed.",
            "Only the listed color pairs are checked; ordinary client pixels and pictorial icons are outside this check.",
            "Opacity, antialiasing, typography, wallpaper artwork, shadows, and actual text placement still need rendered review.",
            "The scrim bound assumes the wallpaper does not exceed the supplied glow color behind the tested area.",
            "Use the focus token for the adjacent surface; an external ring on ivory needs the dark focus token.",
            "Disabled controls are not certified by this report; readable unavailable-state explanation is checked separately.",
            "Color must remain accompanied by existing action labels, switch positions, and state indicators.",
        ],
        "references": SOURCES,
    }


def write_notes(report, path):
    summary = report["summary"]
    lines = [
        "TAROT / LG Fx0 Gold: palette contrast check",
        "",
        report["scope"],
        f"Generated: {report['generated_utc']}",
        f"Input: {report['input_file']}",
        f"Input SHA-256: {report['input_sha256']}",
        "",
        "Result",
        f"{summary['required_passed']} of {summary['required_count']} required color pairs pass.",
        "Text uses a 4.5:1 target. Meaningful graphics and focus indicators use 3:1.",
        "Ratios are displayed to six decimal places; pass/fail uses unrounded values.",
        "",
        "Required pairs by group",
    ]
    groups = defaultdict(list)
    for item in report["checks"]:
        if item["required"]:
            groups[item["group"]].append(item)
    for name, items in groups.items():
        weakest = min(items, key=lambda item: item["ratio"])
        strongest = max(items, key=lambda item: item["ratio"])
        status = "PASS" if all(item["passes"] for item in items) else "FAIL"
        range_text = f"{weakest['ratio']:.6f}:1"
        if len(items) > 1 and weakest["ratio"] != strongest["ratio"]:
            range_text += f" to {strongest['ratio']:.6f}:1"
        lines.append(f"{status} | {name}: {range_text}; target {weakest['threshold']:g}:1.")
        lines.append(f"  Weakest pair: {weakest['foreground_source']} on {weakest['background_source']}.")

    lines.extend(["", "Launcher scrim and bright wallpaper"])
    composite = next(item for item in report["checks"] if item["id"] == "launcher_scrim_text")
    lines.append(
        f"The conservative calculation composites {report['used_role_tokens']['launcher.scrim']} "
        f"at {report['method']['launcher_scrim_opacity']:.0%} over fully opaque "
        f"{report['method']['brightest_assumed_wallpaper_color']}. "
        f"The resulting unrounded sRGB channels are {composite['background']}."
    )
    lines.append(f"Exported Launcher scrim check: {report['launcher_scrim_export_check']['status']}.")
    lines.append(f"Opacity source: {report['method']['launcher_scrim_opacity_source']}.")
    lines.append("The following raw wallpaper pairs are context only, not approved text placements:")
    for item in report["checks"]:
        if not item["required"]:
            state = "passes" if item["passes"] else "fails"
            lines.append(f"  {item['background_source']}: {item['ratio']:.6f}:1; {state} 4.5:1 without a scrim.")
    lines.extend(["", "Limits and handoff"] + report["limitations"])
    lines.extend([
        "",
        "Re-run after palette edits",
        "python source/check_gold_contrast.py",
        "Optional arguments: --theme PATH --output-dir PATH --launcher-scrim-opacity 0.5",
        "A required-pair failure or a mismatch in the existing exported Launcher scrim returns exit status 1.",
        "Missing input or invalid tokens return exit status 2. An absent Launcher SVG is reported as not checked.",
        "The JSON report contains every foreground, background, source role, exact ratio, and threshold.",
        "",
        "References",
    ] + SOURCES)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--theme", type=Path, default=ROOT / "tokens/fx0-gold.theme.json")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "handoff")
    parser.add_argument("--launcher-scrim-opacity", type=float, default=None,
                        help="Override the opacity declared in the theme JSON")
    args = parser.parse_args()
    try:
        report = build_report(args.theme.resolve(), args.launcher_scrim_opacity)
        args.output_dir.mkdir(parents=True, exist_ok=True)
        json_path = args.output_dir / "gold-contrast-checks.json"
        notes_path = args.output_dir / "gold-contrast-notes.txt"
        json_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        write_notes(report, notes_path)
    except (OSError, ValueError, KeyError, TypeError, ET.ParseError) as error:
        print(f"Contrast check could not run: {error}", file=sys.stderr)
        return 2
    summary = report["summary"]
    print(f"Required pairs: {summary['required_passed']}/{summary['required_count']} pass")
    print(f"Launcher scrim: {report['launcher_scrim_export_check']['status']}")
    print(f"JSON: {json_path}")
    print(f"Notes: {notes_path}")
    return 0 if (summary["all_required_pairs_pass"] and
                 report["launcher_scrim_export_check"]["status"] != "mismatch") else 1


if __name__ == "__main__":
    sys.exit(main())
