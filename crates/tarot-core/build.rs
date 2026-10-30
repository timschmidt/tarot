//! Compile the accepted packet tokens into typed, shared layout constants.
use std::{fmt::Write, fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-changed=assets/tarot.tokens.json");
    let data: serde_json::Value =
        serde_json::from_str(&fs::read_to_string("assets/tarot.tokens.json").unwrap()).unwrap();
    assert_eq!(data["schema_version"], 1);
    let g = &data["geometry"];
    let gestures = &data["gesture_proposals"];
    let mut out = String::new();
    for (name, value) in [
        ("STATUS", &g["status_bar_h"]),
        ("GESTURE", &g["gesture_strip_h"]),
        ("DOCK", &g["quick_launch_rect"][3]),
        ("ICON", &g["quick_launch_icon_size"]),
        ("LAUNCHER_ICON", &g["launcher_icon_size"]),
        ("BANNER_ACCESSIBLE", &g["banner_accessible_h"]),
        ("DASHBOARD_ROW", &g["dashboard_row_h"]),
        ("CARD_SCALE", &g["card_scale"]),
        ("INACTIVE_CARD_SCALE", &g["inactive_card_scale"]),
        ("CARD_RADIUS", &g["card_radius_before_scale"]),
        ("CARD_Y", &g["card_anchor"][1]),
        ("AXIS_SLOP", &gestures["axis_lock_dp"]),
        ("TAP_SLOP", &gestures["tap_slop_dp"]),
        ("WAVE_DRAG", &gestures["wave_drag_min_dp"]),
        ("DISMISS_DISTANCE", &gestures["card_dismiss_distance_h"]),
        ("DISMISS_VELOCITY", &gestures["card_dismiss_velocity_dp_s"]),
    ] {
        let n = value.as_f64().expect("numeric design token");
        assert!(n.is_finite() && n > 0.0);
        writeln!(&mut out, "pub const {name}: f32 = {n:?};").unwrap();
    }
    let poses = g["stack_preview_poses"].as_array().unwrap();
    assert_eq!(poses.len(), 3);
    out.push_str("pub const STACK_POSES: [(f32,f32,f32,f32);3] = [");
    for pose in poses {
        let values = ["x", "y", "scale", "degrees"].map(|key| pose[key].as_f64().unwrap());
        assert!(values.iter().all(|v| v.is_finite()));
        assert!((0.1..=1.0).contains(&values[2]) && values[3].abs() <= 15.0);
        writeln!(
            &mut out,
            "({:?},{:?},{:?},{:?}),",
            values[0], values[1], values[2], values[3]
        )
        .unwrap();
    }
    out.push_str("];\n");
    for (name, value) in [
        ("REORDER_HOLD_MS", &gestures["reorder_hold_ms"]),
        ("WAVE_DWELL_MS", &gestures["wave_dwell_ms"]),
    ] {
        let n = value.as_u64().unwrap();
        assert!((1..=5000).contains(&n));
        writeln!(&mut out, "pub const {name}: u64 = {n};").unwrap();
    }
    for name in [
        "maximize",
        "minimize",
        "launch",
        "dismiss",
        "launcher",
        "quick_launch",
        "reduced_motion_fade",
    ] {
        let n = data["motion_ms"][name].as_u64().unwrap();
        assert!((1..=5000).contains(&n));
        writeln!(
            &mut out,
            "pub const {}_MS: u64 = {n};",
            name.to_ascii_uppercase()
        )
        .unwrap();
    }
    for (name, value) in data["fixed_palette"].as_object().unwrap() {
        let s = value.as_str().unwrap().strip_prefix('#').unwrap();
        assert_eq!(s.len(), 6);
        let rgb: Vec<_> = (0..3)
            .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
            .collect();
        writeln!(
            &mut out,
            "pub const {}_COLOR: [u8;3] = {:?};",
            name.to_ascii_uppercase(),
            rgb
        )
        .unwrap();
    }
    let status = g["status_bar_h"].as_f64().unwrap();
    let strip = g["gesture_strip_h"].as_f64().unwrap();
    assert_eq!(g["app_rect"][1].as_f64().unwrap(), status);
    assert_eq!(
        g["app_rect"][3].as_f64().unwrap(),
        data["units"]["canvas_dp"][1].as_f64().unwrap() - status - strip
    );
    assert!((0.1..=1.0).contains(&g["card_scale"].as_f64().unwrap()));
    fs::write(
        PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("design.rs"),
        out,
    )
    .unwrap();
}
