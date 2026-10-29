use super::{PageId, Settings};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Axis {
    Pending,
    Horizontal,
    Vertical,
}
#[derive(Clone, Debug)]
struct Contact {
    start: (f32, f32),
    point: (f32, f32),
    started: u64,
    axis: Axis,
    settings: Settings,
    width: f32,
}
#[derive(Clone, Debug, Default)]
pub struct Swipe {
    contact: Option<Contact>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwipePreview {
    pub page: PageId,
    pub neighbor: Option<PageId>,
    pub offset: f32,
}

impl Swipe {
    pub fn begin(&mut self, point: (f32, f32), now: u64, settings: &Settings, width: f32) {
        self.contact =
            (point.0.is_finite() && point.1.is_finite() && width.is_finite() && width > 0.0).then(
                || Contact {
                    start: point,
                    point,
                    started: now,
                    axis: Axis::Pending,
                    settings: settings.clone(),
                    width,
                },
            );
    }
    pub fn cancel(&mut self) -> bool {
        let consumed = self.horizontal();
        self.contact = None;
        consumed
    }
    pub fn horizontal(&self) -> bool {
        self.contact
            .as_ref()
            .is_some_and(|c| c.axis == Axis::Horizontal)
    }
    pub fn motion(&mut self, point: (f32, f32)) {
        if !point.0.is_finite() || !point.1.is_finite() {
            self.cancel();
            return;
        }
        if let Some(c) = &mut self.contact {
            c.point = point;
            let dx = (point.0 - c.start.0).abs();
            let dy = (point.1 - c.start.1).abs();
            if c.axis == Axis::Pending && dx.max(dy) >= crate::design::AXIS_SLOP {
                c.axis = if dx > dy * 1.25 {
                    Axis::Horizontal
                } else {
                    Axis::Vertical
                };
            }
        }
    }
    pub fn preview(&self, settings: &Settings, width: f32) -> Option<SwipePreview> {
        let c = self.contact.as_ref()?;
        if c.axis != Axis::Horizontal || c.settings != *settings || c.width != width {
            return None;
        }
        let dx = (c.point.0 - c.start.0).clamp(-width, width);
        let neighbor = settings.neighbor(settings.selected, if dx < 0.0 { 1 } else { -1 });
        Some(SwipePreview {
            page: settings.selected,
            neighbor,
            offset: if neighbor.is_some() { dx } else { dx * 0.2 },
        })
    }
    pub fn release(&mut self, now: u64, settings: &Settings, width: f32) -> (bool, Option<PageId>) {
        let preview = self.preview(settings, width);
        let consumed = self.horizontal();
        let target = preview.and_then(|p| {
            let c = self.contact.as_ref()?;
            let distance = p.offset.abs();
            let elapsed = now.saturating_sub(c.started).max(1) as f32;
            (distance >= width * 0.2 || (distance >= 28.0 && distance / elapsed >= 0.6))
                .then_some(p.neighbor)
                .flatten()
        });
        self.contact = None;
        (consumed, target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::Edit;
    #[test]
    fn horizontal_swipe_and_flick_select_only_a_neighbor_while_vertical_scroll_stays_owned() {
        let mut settings = Settings::default();
        settings.edit(Edit::Create("Work".into()), &[]).unwrap();
        settings.edit(Edit::Select(PageId(1)), &[]).unwrap();
        let mut swipe = Swipe::default();
        swipe.begin((180.0, 300.0), 0, &settings, 360.0);
        swipe.motion((80.0, 302.0));
        assert_eq!(swipe.preview(&settings, 360.0).unwrap().offset, -100.0);
        assert_eq!(
            swipe.release(400, &settings, 360.0),
            (true, Some(PageId(2)))
        );
        assert_eq!(swipe.release(401, &settings, 360.0), (false, None));
        swipe.begin((180.0, 300.0), 0, &settings, 360.0);
        swipe.motion((150.0, 300.0));
        assert_eq!(swipe.release(40, &settings, 360.0), (true, Some(PageId(2))));
        swipe.begin((180.0, 300.0), 0, &settings, 360.0);
        swipe.motion((179.0, 270.0));
        swipe.motion((70.0, 250.0));
        assert_eq!(swipe.release(400, &settings, 360.0), (false, None));
    }
    #[test]
    fn ends_small_drags_and_changed_scenes_cannot_commit_a_page() {
        let settings = Settings::default();
        let mut swipe = Swipe::default();
        swipe.begin((180.0, 300.0), 0, &settings, 360.0);
        swipe.motion((300.0, 300.0));
        assert_eq!(swipe.release(100, &settings, 360.0), (true, None));
        let mut two = settings.clone();
        two.edit(Edit::Create("Work".into()), &[]).unwrap();
        swipe.begin((180.0, 300.0), 0, &two, 360.0);
        swipe.motion((300.0, 300.0));
        assert_eq!(swipe.release(400, &two, 720.0), (true, None));
        swipe.begin((180.0, 300.0), 0, &two, 360.0);
        swipe.motion((210.0, 300.0));
        assert_eq!(swipe.release(400, &two, 360.0), (true, None));
    }
}
