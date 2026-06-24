use glam::{Mat4, Quat, Vec3};
use stardust_xr_asteroids::{
    Context, CustomElement as _, Element, Entity, Migrate, Reify, Tasker,
    client::ClientState,
    components::Reparentable,
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
#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct State {
    strokes: Vec<Line>,
    line_thickness: f32,
    pen_pos: Vec3,
    pen_rot: Quat,
    smooth_cursor: Vec3,
    smooth_threshold: f32,
}
impl Default for State {
    fn default() -> Self {
        State {
            strokes: Vec::new(),
            line_thickness: 0.005,
            pen_pos: Vec3::ZERO,
            pen_rot: Quat::IDENTITY,
            smooth_cursor: Vec3::ZERO,
            smooth_threshold: 0.008, // Tune this (5-15mm)
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
    fn reify(&self, _context: &Context, _tasks: impl Tasker<Self>) -> impl Element<Self> {
        Entity::new(Shape::Transform {
            transform: (Mat4::from_rotation_translation(self.pen_rot, self.pen_pos)
                * Mat4::from_translation([0.0, 0.075 / 2.0, 0.0].into()))
            .into(),
            shape: Box::new(Shape::Cylinder {
                length: 0.075,
                radius: 0.0025 / 2.0,
            }),
        })
        .component(Reparentable::default())
        .build()
        .child(
            Pen::<State>::new(self.pen_pos, self.pen_rot, |state, pen_state, pos, rot| {
                state.pen_pos = pos.into();
                state.pen_rot = rot.into();
                match pen_state {
                    PenState::StartedDrawing(strength) => {
                        state.smooth_cursor = state.pen_pos;
                        state.strokes.push(Line {
                            points: vec![LinePoint {
                                point: pos,
                                thickness: state.line_thickness * strength,
                                color: rgba_linear!(1.0, 0.0, 0.0, 1.0),
                            }],
                            cyclic: false,
                        });
                    }
                    PenState::Drawing(strength) => {
                        if let Some(current_stroke) = state.strokes.last_mut() {
                            let dist = state.smooth_cursor.distance(state.pen_pos);
                            if dist > state.smooth_threshold {
                                // Lazy string: pull cursor toward input
                                let dir = (state.pen_pos - state.smooth_cursor).normalize_or_zero();
                                state.smooth_cursor += dir * state.smooth_threshold.min(dist);

                                current_stroke.points.push(LinePoint {
                                    point: (state.smooth_cursor.into()),
                                    thickness: state.line_thickness * strength,
                                    color: rgba_linear!(1.0, 0.0, 0.0, 1.0),
                                });
                            }
                        }
                    }
                    _ => {}
                };
            })
            .color(rgba_linear!(0.5, 0.0, 0.0, 1.0))
            .hand_draw_threshold(0.65)
            .build(),
        )
        .children(
            self.strokes
                .iter()
                .map(|line| Lines::new(vec![line.clone()]).build()),
        )
    }
}
