pub mod pen;

use asteroids::{
    client::ClientState,
    custom::{ElementTrait, FnWrapper},
    elements::{Lines, Spatial},
    util::Migrate,
};
use glam::Vec3;
use map_range::MapRange as _;
use pen::{Pen, PenChildAnchor};
use stardust_xr_fusion::{
    drawable::{Line, LinePoint},
    input::InputDataType,
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
}
impl Default for State {
    fn default() -> Self {
        State {
            strokes: Vec::new(),
            line_thickness: 0.005,
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
            [Pen::<State> {
                move_resolution: 0.001,
                length: 0.075,
                thickness: 0.005,
                grab_distance: 0.1,
                color: rgba_linear!(0.5, 0.0, 0.0, 1.0),
                child_anchor: PenChildAnchor::Tip,
                should_interact: FnWrapper(Box::new(|data| {
                    data.datamap.with_data(|datamap| match &data.input {
                        InputDataType::Hand(h) => {
                            Vec3::from(h.thumb.tip.position).distance(h.index.tip.position.into())
                                < 0.03
                        }
                        InputDataType::Tip(_) => datamap.idx("select").as_f32() > 0.01,
                        _ => false,
                    })
                })),
                on_interact_start: FnWrapper(Box::new(|state| {
                    state.strokes.push(Line {
                        points: Vec::with_capacity(350),
                        cyclic: false,
                    });
                })),
                on_interact: FnWrapper(Box::new(|state, point, actor| {
                    let strength = actor.datamap.with_data(|datamap| match &actor.input {
                        InputDataType::Hand(h) => Vec3::from(h.thumb.tip.position)
                            .distance(h.index.tip.position.into())
                            .map_range(0.03..0.0, 0.0..1.0)
                            .clamp(0.0, 1.0)
                            .sqrt(),
                        InputDataType::Tip(_) => datamap.idx("select").as_f32(),
                        _ => unimplemented!(),
                    });

                    let Some(current_stroke) = state.strokes.last_mut() else {
                        return;
                    };
                    current_stroke.points.push(LinePoint {
                        point: point.into(),
                        thickness: state.line_thickness * strength,
                        color: rgba_linear!(1.0, 0.0, 0.0, 1.0),
                    });
                    if current_stroke.points.len() >= 350 {
                        state.strokes.push(Line {
                            points: {
                                let mut vec = Vec::with_capacity(350);
                                vec.push(LinePoint {
                                    point: point.into(),
                                    thickness: state.line_thickness * strength,
                                    color: rgba_linear!(1.0, 0.0, 0.0, 1.0),
                                });
                                vec
                            },
                            cyclic: false,
                        });
                    }
                })),
                on_interact_stop: FnWrapper(Box::new(|_| {})),
            }
            .build()]
            .into_iter()
            .chain(
                self.strokes
                    .iter()
                    .map(|line| Lines::default().lines(vec![line.clone()]).build()),
            ),
        )
    }
}
