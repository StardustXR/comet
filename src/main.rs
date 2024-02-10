use color_eyre::eyre::Result;
use glam::Vec3;
use map_range::MapRange;
use mint::Vector3;
use rustc_hash::FxHashMap;
use stardust_xr_fusion::{
    client::{Client, ClientState, FrameInfo, RootHandler},
    core::{schemas::flex::flexbuffers, values::rgba_linear},
    drawable::{Line, LinePoint, Lines, LinesAspect},
    fields::CylinderField,
    input::{InputDataType, InputHandler},
    node::{NodeError, NodeType},
    spatial::{Spatial, SpatialAspect, Transform},
    HandlerWrapper,
};
use stardust_xr_molecules::input_action::{BaseInputAction, InputActionHandler, SingleActorAction};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let (client, event_loop) = Client::connect_with_async_loop().await?;

    let pen = Pen::new(&client, PenSettings::default())?;
    let _wrapped_root = client.wrap_root(pen)?;

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
    client_root: Spatial,
    pen_root: Spatial,
    _field: CylinderField,
    input: HandlerWrapper<InputHandler, InputActionHandler<PenSettings>>,
    hover_action: BaseInputAction<PenSettings>,
    grab_action: SingleActorAction<PenSettings>,
    draw_action: BaseInputAction<PenSettings>,
    _visuals: Lines,
    stroke_lines: Lines,
    strokes: Vec<Line>,
}
impl Pen {
    pub fn new(client: &Client, settings: PenSettings) -> Result<Self, NodeError> {
        let visual_length = 0.075;
        let client_root = client.get_root().alias();
        let pen_root = Spatial::create(client.get_root(), Transform::none(), true)?;
        let field = CylinderField::create(
            &pen_root,
            Transform::from_translation([0.0, 0.0, visual_length * 0.5]),
            visual_length,
            settings.thickness * 0.5,
        )?;
        let input = InputActionHandler::wrap(
            InputHandler::create(client.get_root(), Transform::none(), &field)?,
            settings,
        )?;
        let hover_action = BaseInputAction::new(false, |data, settings: &PenSettings| {
            data.distance < settings.max_distance
        });
        let grab_action = SingleActorAction::new(
            true,
            |data, _| {
                data.datamap.with_data(|datamap| match &data.input {
                    InputDataType::Hand(_) => datamap.idx("grab_strength").as_f32() > 0.90,
                    InputDataType::Tip(_) => datamap.idx("grab").as_f32() > 0.90,
                    _ => false,
                })
            },
            false,
        );
        let draw_action = BaseInputAction::new(false, |data, _| {
            data.datamap.with_data(|datamap| match &data.input {
                InputDataType::Hand(h) => {
                    Vec3::from(h.thumb.tip.position).distance(h.index.tip.position.into()) < 0.03
                }
                InputDataType::Tip(_) => datamap.idx("select").as_f32() > 0.01,
                _ => false,
            })
        });
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
        Ok(Self {
            settings,
            client_root,
            pen_root,
            _field: field,
            input,
            hover_action,
            grab_action,
            draw_action,
            _visuals: visuals,
            stroke_lines,
            strokes: Vec::new(),
        })
    }
}
impl RootHandler for Pen {
    fn frame(&mut self, _info: FrameInfo) {
        self.input.lock_wrapped().update_actions([
            &mut self.hover_action,
            &mut self.draw_action,
            self.grab_action.base_mut(),
        ]);
        self.grab_action.update(Some(&mut self.hover_action));

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
            .set_relative_transform(self.input.node().as_ref(), transform);

        if self.draw_action.started_acting.contains(grab_actor) {
            self.strokes.push(Line {
                points: Vec::new(),
                cyclic: false,
            });
        }
        if let Some(draw_actor) = self.draw_action.currently_acting.get(grab_actor) {
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

    fn save_state(&mut self) -> ClientState {
        ClientState {
            data: Some(flexbuffers::to_vec(&self.strokes).unwrap()),
            root: Some(self.client_root.alias()),
            spatial_anchors: FxHashMap::from_iter([("pen".to_string(), self.pen_root.alias())]),
        }
    }
}
