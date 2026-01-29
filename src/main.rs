use glam::{Quat, Vec3};
use stardust_xr_asteroids::{
    CustomElement as _, Migrate, Reify,
    client::ClientState,
    elements::{Lines, Pen, PenState, Reparentable},
};
use stardust_xr_fusion::{
    drawable::{Line, LinePoint},
    values::color::rgba_linear,
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    stardust_xr_asteroids::client::run::<State>(&[]).await;
}
#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct State {
    strokes: Vec<Line>,
    line_thickness: f32,
    pen_pos: Vec3,
    pen_rot: Quat,
    prev_pos: Vec3,
}
impl Default for State {
    fn default() -> Self {
        State {
            strokes: Vec::new(),
            line_thickness: 0.005,
            pen_pos: Vec3::ZERO,
            pen_rot: Quat::IDENTITY,
            prev_pos: Vec3::ZERO,
        }
    }
}
impl Migrate for State {
    type Old = State;
}
impl ClientState for State {
    const APP_ID: &'static str = "org.stardustxr.comet";
}
impl Reify for State {
    fn reify(&self) -> impl stardust_xr_asteroids::Element<Self> {
        Reparentable::default()
            .build()
            .child(
                Pen::<State>::new(self.pen_pos, self.pen_rot, |state, pen_state, pos, rot| {
                    state.pen_pos = pos.into();
                    state.pen_rot = rot.into();
                    match pen_state {
                        PenState::StartedDrawing(strength) => {
                            state.prev_pos = state.pen_pos;
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
                            if state.pen_pos.distance(state.prev_pos) > 0.001 {
                                state.prev_pos = state.pen_pos;
                                if let Some(current_stroke) = state.strokes.last_mut() {
                                    current_stroke.points.push(LinePoint {
                                        point: pos,
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
