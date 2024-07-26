use color_eyre::eyre::Result;
use glam::Vec3;
use manifest_dir_macros::directory_relative_path;
use map_range::MapRange;
use mint::Vector3;
use rustc_hash::FxHashMap;
use stardust_xr_fusion::{
    client::Client,
    core::schemas::flex::flexbuffers,
    drawable::{Line, LinePoint, Lines, LinesAspect},
    fields::{CylinderShape, Field, Shape},
    input::{InputDataType, InputHandler},
    node::{MethodResult, NodeError, NodeType},
    root::{ClientState, FrameInfo, RootAspect, RootHandler},
    spatial::{Spatial, SpatialAspect, Transform},
    values::color::rgba_linear,
};
use stardust_xr_molecules::input_action::{InputQueue, InputQueueable, SimpleAction, SingleAction};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let (client, event_loop) = Client::connect_with_async_loop().await?;
    client.set_base_prefixes(&[directory_relative_path!("res")])?;

    let pen = Pen::new(&client, PenSettings::default())?;
    let _wrapped_root = client.get_root().alias().wrap(pen)?;

    tokio::select! {
        _ = tokio::signal::ctrl_c() => (),
        e = event_loop => e??,
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub struct PenSettings {
    pub max_distance: f32,
    pub thickness: f32,
}
impl Default for PenSettings {
    fn default() -> Self {
        Self {
            max_distance: 0.05,
            thickness: 0.005,
        }
    }
}

pub struct Pen {
    settings: PenSettings,
    pen_root: Spatial,
    _field: Field,
    input: InputQueue,
    grab_action: SingleAction,
    draw_action: SimpleAction,
    _visuals: Lines,
    stroke_lines: Lines,
    strokes: Vec<Line>,
}
impl Pen {
    pub fn new(client: &Client, settings: PenSettings) -> Result<Self, NodeError> {
        let visual_length = 0.075;
        let pen_root = Spatial::create(client.get_root(), Transform::none(), true)?;
        let field = Field::create(
            &pen_root,
            Transform::from_translation([0.0, 0.0, visual_length * 0.5]),
            Shape::Cylinder(CylinderShape {
                length: visual_length,
                radius: settings.thickness * 0.5,
            }),
        )?;
        let input = InputHandler::create(client.get_root(), Transform::none(), &field)?.queue()?;

        let visuals = Lines::create(
            &pen_root,
            Transform::none(),
            &[Line {
                points: vec![
                    LinePoint {
                        point: [0.0; 3].into(),
                        thickness: 0.0,
                        color: rgba_linear!(1.0, 0.0, 0.0, 0.0),
                    },
                    LinePoint {
                        point: [0.0, 0.0, settings.thickness].into(),
                        thickness: settings.thickness,
                        color: rgba_linear!(1.0, 0.0, 0.0, 1.0),
                    },
                    LinePoint {
                        point: [0.0, 0.0, visual_length].into(),
                        thickness: settings.thickness,
                        color: rgba_linear!(1.0, 0.0, 0.0, 1.0),
                    },
                ],
                cyclic: false,
            }],
        )?;
        let stroke_lines = Lines::create(client.get_root(), Transform::none(), &[])?;
        stroke_lines.set_zoneable(true)?;
        Ok(Self {
            settings,
            pen_root,
            _field: field,
            input,
            grab_action: Default::default(),
            draw_action: Default::default(),
            _visuals: visuals,
            stroke_lines,
            strokes: Vec::new(),
        })
    }
}
impl RootHandler for Pen {
    fn frame(&mut self, _info: FrameInfo) {
        self.grab_action.update(
            false,
            &self.input,
            |data| data.distance < self.settings.max_distance,
            |data| {
                data.datamap.with_data(|datamap| match &data.input {
                    InputDataType::Hand(_) => datamap.idx("grab_strength").as_f32() > 0.90,
                    InputDataType::Tip(_) => datamap.idx("grab").as_f32() > 0.90,
                    _ => false,
                })
            },
        );
        self.draw_action.update(&self.input, &|data| {
            data.datamap.with_data(|datamap| match &data.input {
                InputDataType::Hand(h) => {
                    Vec3::from(h.thumb.tip.position).distance(h.index.tip.position.into()) < 0.03
                }
                InputDataType::Tip(_) => datamap.idx("select").as_f32() > 0.01,
                _ => false,
            })
        });

        if self.grab_action.actor_started() {
            let _ = self.pen_root.set_zoneable(false);
        }
        if self.grab_action.actor_stopped() {
            let _ = self.pen_root.set_zoneable(true);
        }
        let Some(grab_actor) = self.grab_action.actor() else {
            return;
        };
        let transform = match &grab_actor.input {
            InputDataType::Hand(h) => Transform::from_translation_rotation(
                (Vec3::from(h.thumb.tip.position) + Vec3::from(h.index.tip.position)) * 0.5,
                h.palm.rotation,
            ),
            InputDataType::Tip(t) => Transform::from_translation_rotation(t.origin, t.orientation),
            _ => Transform::none(),
        };
        let _ = self
            .pen_root
            .set_relative_transform(self.input.handler(), transform);

        if self.draw_action.started_acting().contains(grab_actor) {
            self.strokes.push(Line {
                points: Vec::new(),
                cyclic: false,
            });
        }
        if let Some(draw_actor) = self.draw_action.currently_acting().get(grab_actor) {
            let point = match &draw_actor.input {
                InputDataType::Hand(h) => Vector3::from(
                    (Vec3::from(h.thumb.tip.position) + Vec3::from(h.index.tip.position)) * 0.5,
                ),
                InputDataType::Tip(t) => t.origin,
                _ => unreachable!(),
            };
            let strength = draw_actor
                .datamap
                .with_data(|datamap| match &draw_actor.input {
                    InputDataType::Hand(h) => Vec3::from(h.thumb.tip.position)
                        .distance(h.index.tip.position.into())
                        .map_range(0.03..0.0, 0.0..1.0)
                        .clamp(0.0, 1.0)
                        .sqrt(),
                    InputDataType::Tip(_) => datamap.idx("select").as_f32(),
                    _ => unimplemented!(),
                });
            let Some(current_stroke) = self.strokes.last_mut() else {
                return;
            };
            current_stroke.points.push(LinePoint {
                point,
                thickness: self.settings.thickness * strength,
                color: rgba_linear!(1.0, 0.0, 0.0, 1.0),
            });
            self.stroke_lines.set_lines(&self.strokes).unwrap();
        }
    }

    fn save_state(&mut self) -> MethodResult<ClientState> {
        ClientState::new(
            Some(flexbuffers::to_vec(&self.strokes).unwrap()),
            self._field.node().client().unwrap().get_root(),
            FxHashMap::from_iter([("pen".to_string(), &self.pen_root)]),
        )
    }
}
