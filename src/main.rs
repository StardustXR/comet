use glam::{FloatExt, Mat4, Quat, Vec3};
use stardust_xr_asteroids::{
    Context, CustomElement as _, Element, Entity, Migrate, Reify, Tasker,
    client::ClientState,
    components::Containable,
    elements::{Lines, Pen, PenState},
};
use stardust_xr_fusion::{
    drawable::{Line, LinePoint},
    fields::Shape,
    types::rgba_linear,
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    stardust_xr_asteroids::client::run::<State>(&[])
        .await
        .unwrap();
}
const SPACING: f32 = 0.004;
const SWEEPS: usize = 60;
/// m/s where a point gets the full pull toward its chord, slower strokes keep their detail
const REFERENCE_SPEED: f32 = 0.3;

/// Index–thumb pinch strength to *start* drawing (asteroids `hand_draw_threshold`).
/// Tuned live on Quest via WiVRn (2026-11-05).
const PINCH_START: f32 = 0.7000931;
/// Keep drawing until pinch drops below this (hysteresis vs `PINCH_START`).
const PINCH_RELEASE: f32 = 0.6928417;

#[derive(Clone, Copy, Debug)]
struct StrokePoint {
    pos: Vec3,
    thickness: f32,
    t: f32,
}

/// the whole stroke is rebuilt from the raw samples every frame, so the tip settles as you keep drawing
fn stabilize(raw: &[StrokePoint]) -> Vec<LinePoint> {
    let mut points = resample(raw, SPACING);
    // weights come from the unsmoothed points so relaxing can't feed back into them
    let weights: Vec<(f32, f32)> = points
        .windows(3)
        .map(|w| {
            let before = w[0].pos.distance(w[1].pos);
            let total = before + w[1].pos.distance(w[2].pos);
            let t = if total > 0.0 { before / total } else { 0.5 };
            let dt = w[2].t - w[0].t;
            let pull = if dt > 0.0 {
                (total / dt / REFERENCE_SPEED).min(1.0)
            } else {
                1.0
            };
            (t, pull)
        })
        .collect();
    for _ in 0..SWEEPS {
        for (i, &(t, pull)) in weights.iter().enumerate() {
            let target = points[i].pos.lerp(points[i + 2].pos, t);
            points[i + 1].pos = points[i + 1].pos.lerp(target, pull);
        }
    }
    points
        .into_iter()
        .map(|p| LinePoint {
            point: p.pos.into(),
            thickness: p.thickness,
            color: rgba_linear!(1.0, 0.0, 0.0, 1.0),
        })
        .collect()
}

/// even arc length spacing so the smoothing pulls the same amount no matter how fast you draw
fn resample(raw: &[StrokePoint], spacing: f32) -> Vec<StrokePoint> {
    let Some(&first) = raw.first() else {
        return Vec::new();
    };
    let mut out = vec![first];
    let mut residual = 0.0;
    for w in raw.windows(2) {
        let (a, b) = (w[0], w[1]);
        let seg = a.pos.distance(b.pos);
        if seg < 1e-5 {
            continue;
        }
        let mut d = spacing - residual;
        while d <= seg {
            let t = d / seg;
            out.push(StrokePoint {
                pos: a.pos.lerp(b.pos, t),
                thickness: a.thickness.lerp(b.thickness, t),
                t: a.t.lerp(b.t, t),
            });
            d += spacing;
        }
        residual = seg - (d - spacing);
    }
    let tip = raw[raw.len() - 1];
    if out[out.len() - 1].pos.distance(tip.pos) > 1e-5 {
        out.push(tip);
    }
    out
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct State {
    strokes: Vec<Line>,
    line_thickness: f32,
    pen_pos: Vec3,
    pen_rot: Quat,
    #[serde(skip)]
    raw: Vec<StrokePoint>,
    /// True while Pen reports Drawing/StartedDrawing — feeds pinch-release hysteresis.
    #[serde(skip)]
    pen_drawing: bool,
}
impl Default for State {
    fn default() -> Self {
        State {
            strokes: Vec::new(),
            line_thickness: 0.005,
            pen_pos: Vec3::ZERO,
            pen_rot: Quat::IDENTITY,
            raw: Vec::new(),
            pen_drawing: false,
        }
    }
}

impl State {
    fn pinch_threshold(&self) -> f32 {
        if self.pen_drawing {
            PINCH_RELEASE.min(PINCH_START)
        } else {
            PINCH_START
        }
    }

    fn apply_pen_state(&mut self, pen_state: PenState, now: f32) {
        match pen_state {
            PenState::StartedDrawing(strength) | PenState::Drawing(strength) => {
                let start = matches!(pen_state, PenState::StartedDrawing(_)) || self.raw.is_empty();
                self.pen_drawing = true;
                if start {
                    self.raw.clear();
                }
                self.raw.push(StrokePoint {
                    pos: self.pen_pos,
                    thickness: self.line_thickness * strength,
                    t: now,
                });
                let points = stabilize(&self.raw);
                if start {
                    self.strokes.push(Line {
                        points,
                        cyclic: false,
                    });
                } else if let Some(stroke) = self.strokes.last_mut() {
                    stroke.points = points;
                }
            }
            // Any non-drawing state closes the stroke, including grab/tracking loss.
            _ => {
                self.pen_drawing = false;
                self.raw.clear();
            }
        }
    }
}

impl Migrate for State {
    type Old = State;
}
impl ClientState for State {
    const APP_ID: &'static str = "org.stardustxr.Comet";
}
impl Reify for State {
    fn reify(
        &self,
        context: &Context,
        _tasks: impl Tasker<Self>,
        _props: (),
    ) -> impl Element<Self> {
        Entity::new(Shape::Transform {
            transform: (Mat4::from_rotation_translation(self.pen_rot, self.pen_pos)
                * Mat4::from_translation([0.0, 0.075 / 2.0, 0.0].into()))
            .into(),
            shape: Box::new(Shape::Cylinder {
                length: 0.075,
                radius: 0.0025 / 2.0,
            }),
        })
        .component(Containable::default())
        .build()
        .child(
            Pen::<State>::new(self.pen_pos, self.pen_rot, {
                let context = context.clone();
                move |state, pen_state, pos, rot| {
                    state.pen_pos = pos.into();
                    state.pen_rot = rot.into();
                    state.apply_pen_state(pen_state, context.time());
                }
            })
            .color(rgba_linear!(0.5, 0.0, 0.0, 1.0))
            .hand_draw_threshold(self.pinch_threshold())
            .build(),
        )
        .children(
            self.strokes
                .iter()
                .map(|line| Lines::new(vec![line.clone()]).build()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_match_live_tuning() {
        let state = State::default();
        assert_eq!(state.pinch_threshold(), 0.7000931);
        assert_eq!(PINCH_RELEASE, 0.6928417);
        assert!(PINCH_RELEASE < PINCH_START);
    }

    #[test]
    fn hysteresis_survives_the_gap_and_resets_on_release() {
        let mut state = State::default();
        state.apply_pen_state(PenState::StartedDrawing(1.0), 0.0);
        let in_gap = (PINCH_START + PINCH_RELEASE) * 0.5;
        assert!(in_gap < PINCH_START);
        assert!(in_gap > state.pinch_threshold());
        state.apply_pen_state(PenState::StoppedDrawing, 0.1);
        assert!(!state.pen_drawing);
        assert!(state.raw.is_empty());
        assert_eq!(state.pinch_threshold(), PINCH_START);
    }

    #[test]
    fn hand_loss_or_grab_without_draw_ends_the_stroke() {
        for stop in [
            PenState::Floating,
            PenState::Grabbed,
            PenState::StoppedDrawing,
        ] {
            let mut state = State::default();
            state.apply_pen_state(PenState::StartedDrawing(1.0), 0.0);
            state.pen_pos = Vec3::X * 0.01;
            state.apply_pen_state(PenState::Drawing(1.0), 0.1);
            let saved = state.strokes[0].points.clone();
            state.apply_pen_state(stop, 0.2);
            assert!(!state.pen_drawing);
            assert!(state.raw.is_empty());
            assert_eq!(state.strokes[0].points, saved);
            state.apply_pen_state(stop, 0.3);
            assert_eq!(state.strokes.len(), 1);
        }
    }

    #[test]
    fn drawing_after_loss_starts_fresh_without_overwriting_previous_ink() {
        let mut state = State::default();
        state.apply_pen_state(PenState::StartedDrawing(1.0), 0.0);
        state.pen_pos = Vec3::X * 0.01;
        state.apply_pen_state(PenState::Drawing(1.0), 0.1);
        let saved = state.strokes[0].points.clone();
        state.apply_pen_state(PenState::Floating, 0.2);
        state.pen_pos = Vec3::X;
        state.apply_pen_state(PenState::Drawing(1.0), 0.3);
        assert_eq!(state.strokes.len(), 2);
        assert_eq!(state.raw.len(), 1);
        assert_eq!(state.strokes[0].points, saved);
        assert_eq!(Vec3::from(state.strokes[1].points[0].point), Vec3::X);
    }
}
