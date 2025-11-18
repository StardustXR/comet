use std::time::Instant;

use glam::{Quat, Vec3};
use stardust_xr_asteroids::{
    client::ClientState,
    elements::{Lines, Pen, PenState, Reparentable},
    CustomElement as _, Migrate, Reify,
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
    last_pen_update_pos: Vec3,
    #[serde(skip)]
    grab_stopped: Option<Instant>,
}
impl Default for State {
    fn default() -> Self {
        State {
            strokes: Vec::new(),
            line_thickness: 0.005,
            pen_pos: Vec3::ZERO,
            pen_rot: Quat::IDENTITY,
            last_pen_update_pos: Vec3::ZERO,
            grab_stopped: None,
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
                    let strength = match pen_state {
                        PenState::StartedDrawing(v) => {
                            if state.grab_stopped.is_none_or(|v| {
                                Instant::now().duration_since(v).as_secs_f32() >= (1.0 / 30.0)
                            }) {
                                state.strokes.push(Line {
                                    points: Vec::with_capacity(128),
                                    cyclic: false,
                                });
                            }
                            v
                        }
                        PenState::Drawing(v) => v,
                        PenState::StoppedDrawing => {
                            state.grab_stopped = Some(Instant::now());
                            return;
                        }
                        PenState::Grabbed => return,
                    };
                    if state.last_pen_update_pos.distance(state.pen_pos) < 0.001 {
                        return;
                    }
                    state.last_pen_update_pos = state.pen_pos;
                    let Some(current_stroke) = state.strokes.last_mut() else {
                        return;
                    };
                    current_stroke.points.push(LinePoint {
                        point: pos,
                        thickness: state.line_thickness * strength,
                        color: rgba_linear!(1.0, 0.0, 0.0, 1.0),
                    });
                    if current_stroke.points.len() >= 128 {
                        println!("making new line because of size");
                        state.strokes.push(Line {
                            points: {
                                let mut vec = Vec::with_capacity(128);
                                vec.push(LinePoint {
                                    point: pos,
                                    thickness: state.line_thickness * strength,
                                    color: rgba_linear!(1.0, 0.0, 0.0, 1.0),
                                });
                                vec
                            },
                            cyclic: false,
                        });
                    }
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
