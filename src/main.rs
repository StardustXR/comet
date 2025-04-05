use std::time::Instant;

use asteroids::{
    client::ClientState,
    custom::{ElementTrait, FnWrapper},
    elements::{Lines, Pen, PenState, Spatial},
    util::Migrate,
};
use glam::{Quat, Vec3};
use map_range::MapRange as _;
use stardust_xr_fusion::{
    drawable::{Line, LinePoint},
    input::{InputData, InputDataType},
    values::color::rgba_linear,
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    asteroids::client::run::<State>(&[]).await;
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
    const QUALIFIER: &'static str = "org";

    const ORGANIZATION: &'static str = "stardust";

    const NAME: &'static str = "comet";

    fn reify(&self) -> asteroids::Element<Self> {
        Spatial::default().zoneable(true).with_children(
            [
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
                .drawing_value({
                    let w: FnWrapper<dyn Fn(&InputData) -> f32 + Send + Sync> =
                        FnWrapper(Box::new(|data: &InputData| {
                            data.datamap.with_data(|datamap| match &data.input {
                                InputDataType::Hand(h) => Vec3::from(h.thumb.tip.position)
                                    .distance(h.index.tip.position.into())
                                    .map_range(0.03..0.01, 0.0..1.0)
                                    .clamp(0.0, 1.0)
                                    .sqrt(),
                                InputDataType::Tip(_) => datamap.idx("select").as_f32().sqrt(),
                                _ => unimplemented!(),
                            })
                        }));
                    w
                })
                .color(rgba_linear!(0.5, 0.0, 0.0, 1.0))
                .build(),
            ]
            .into_iter()
            .chain(
                self.strokes
                    .iter()
                    .map(|line| Lines::default().lines(vec![line.clone()]).build()),
            ),
        )
    }
}
