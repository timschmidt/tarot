//! Single-output orientation and presentation identity, independent of graphics.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Orientation {
    #[default]
    Normal,
    LeftUp,
    BottomUp,
    RightUp,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct SensorSnapshot {
    pub enabled: bool,
    pub available: bool,
    pub has_accelerometer: bool,
    pub claimed: bool,
    pub orientation: Option<Orientation>,
    pub epoch: u64,
    pub error: Option<String>,
}

impl std::str::FromStr for Orientation {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "normal" => Ok(Self::Normal),
            "left-up" => Ok(Self::LeftUp),
            "bottom-up" => Ok(Self::BottomUp),
            "right-up" => Ok(Self::RightUp),
            _ => Err("Expected normal, left-up, bottom-up or right-up".into()),
        }
    }
}

impl Orientation {
    pub fn inverse(self) -> Self {
        match self {
            Self::LeftUp => Self::RightUp,
            Self::RightUp => Self::LeftUp,
            other => other,
        }
    }
    pub fn swaps_axes(self) -> bool {
        matches!(self, Self::LeftUp | Self::RightUp)
    }

    /// Panel-normal normalized coordinates to oriented output coordinates.
    pub fn map_normalized(self, x: f64, y: f64) -> (f64, f64) {
        match self {
            Self::Normal => (x, y),
            Self::LeftUp => (1.0 - y, x),
            Self::BottomUp => (1.0 - x, 1.0 - y),
            Self::RightUp => (y, 1.0 - x),
        }
    }

    pub fn map_delta(self, x: f64, y: f64) -> (f64, f64) {
        match self {
            Self::Normal => (x, y),
            Self::LeftUp => (-y, x),
            Self::BottomUp => (-x, -y),
            Self::RightUp => (y, -x),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct OutputGeometry {
    pub orientation: Orientation,
    pub requested: Option<Orientation>,
    pub physical_width: u32,
    pub physical_height: u32,
    pub scale: u32,
    pub revision: u64,
    pub presented_revision: Option<u64>,
}

impl Default for OutputGeometry {
    fn default() -> Self {
        Self {
            orientation: Orientation::Normal,
            requested: None,
            physical_width: 360,
            physical_height: 720,
            scale: 1,
            revision: 0,
            presented_revision: None,
        }
    }
}

impl OutputGeometry {
    pub fn request(&mut self, orientation: Orientation) {
        self.requested = (orientation != self.orientation).then_some(orientation);
    }

    /// Called by the backend only when no old page flip is outstanding.
    pub fn apply(&mut self, width: u32, height: u32, scale: u32, orientation: Orientation) -> bool {
        assert!(width > 0 && height > 0 && scale > 0);
        self.requested = None;
        if (width, height, scale, orientation)
            == (
                self.physical_width,
                self.physical_height,
                self.scale,
                self.orientation,
            )
        {
            return false;
        }
        self.physical_width = width;
        self.physical_height = height;
        self.scale = scale;
        self.orientation = orientation;
        self.revision = self
            .revision
            .checked_add(1)
            .expect("Output revision exhausted");
        true
    }

    pub fn logical_size(&self) -> (f64, f64) {
        let (w, h) = if self.orientation.swaps_axes() {
            (self.physical_height, self.physical_width)
        } else {
            (self.physical_width, self.physical_height)
        };
        (
            f64::from(w) / f64::from(self.scale),
            f64::from(h) / f64::from(self.scale),
        )
    }

    /// Retain the pointer's panel pixel location across output reconfiguration.
    pub fn reproject_pointer(&self, previous: &Self, x: f64, y: f64) -> (f64, f64) {
        let (old_w, old_h) = previous.logical_size();
        let (x, y) = previous
            .orientation
            .inverse()
            .map_normalized(x / old_w, y / old_h);
        let x = (x * f64::from(previous.physical_width) / f64::from(self.physical_width))
            .clamp(0.0, 1.0);
        let y = (y * f64::from(previous.physical_height) / f64::from(self.physical_height))
            .clamp(0.0, 1.0);
        let (x, y) = self.orientation.map_normalized(x, y);
        let (w, h) = self.logical_size();
        (x * w, y * h)
    }

    pub fn presented(&mut self, revision: u64) -> bool {
        if revision != self.revision {
            return false;
        }
        self.presented_revision = Some(revision);
        true
    }

    pub fn blocks_unlock(&self) -> bool {
        self.requested.is_some() || self.presented_revision != Some(self.revision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_square_panel_corners_and_inverse_agree() {
        let cases = [
            (Orientation::Normal, (0.0, 0.0), (360.0, 720.0)),
            (Orientation::LeftUp, (720.0, 0.0), (0.0, 360.0)),
            (Orientation::BottomUp, (360.0, 720.0), (0.0, 0.0)),
            (Orientation::RightUp, (0.0, 360.0), (720.0, 0.0)),
        ];
        for (orientation, first, last) in cases {
            let mut output = OutputGeometry::default();
            output.apply(720, 1440, 2, orientation);
            let (w, h) = output.logical_size();
            let (x, y) = orientation.map_normalized(0.0, 0.0);
            assert_eq!((x * w, y * h), first);
            let (x, y) = orientation.map_normalized(1.0, 1.0);
            assert_eq!((x * w, y * h), last);
            let inverse = match orientation {
                Orientation::LeftUp => Orientation::RightUp,
                Orientation::RightUp => Orientation::LeftUp,
                other => other,
            };
            let (x, y) = orientation.map_normalized(0.25, 0.75);
            assert_eq!(inverse.map_normalized(x, y), (0.25, 0.75));
        }
        assert!("undefined".parse::<Orientation>().is_err());
    }

    #[test]
    fn queued_orientation_keeps_old_input_geometry_until_backend_applies_it() {
        let mut output = OutputGeometry::default();
        output.presented(0);
        output.request(Orientation::LeftUp);
        assert_eq!(output.orientation, Orientation::Normal);
        assert_eq!(output.logical_size(), (360.0, 720.0));
        assert!(output.blocks_unlock());
        output.apply(360, 720, 1, Orientation::LeftUp);
        assert_eq!(output.logical_size(), (720.0, 360.0));
        assert!(!output.presented(0));
        assert!(output.blocks_unlock());
        assert!(output.presented(output.revision));
        assert!(!output.blocks_unlock());
    }

    #[test]
    fn resize_and_scale_require_new_proof_but_repeated_configuration_does_not() {
        let mut output = OutputGeometry::default();
        output.presented(0);
        assert!(!output.apply(360, 720, 1, Orientation::Normal));
        assert!(!output.blocks_unlock());
        output.apply(720, 1440, 2, Orientation::Normal);
        assert_eq!(output.logical_size(), (360.0, 720.0));
        assert!(output.blocks_unlock());
        output.presented(output.revision);
        output.request(Orientation::RightUp);
        output.request(Orientation::Normal);
        assert!(!output.blocks_unlock());
    }

    #[test]
    fn stationary_pointer_keeps_its_panel_pixel_position() {
        let mut normal = OutputGeometry::default();
        normal.apply(720, 1440, 2, Orientation::Normal);
        let mut rotated = normal.clone();
        rotated.apply(720, 1440, 2, Orientation::LeftUp);
        let point = rotated.reproject_pointer(&normal, 100.0, 200.0);
        assert_eq!(point, (520.0, 100.0));
        let restored = normal.reproject_pointer(&rotated, point.0, point.1);
        assert!((restored.0 - 100.0).abs() < 1e-10 && (restored.1 - 200.0).abs() < 1e-10);
        let mut scaled = normal.clone();
        scaled.apply(720, 1440, 1, Orientation::Normal);
        assert_eq!(
            scaled.reproject_pointer(&normal, 100.0, 200.0),
            (200.0, 400.0)
        );
    }
}
