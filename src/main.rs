use color::rgba;
use color_eyre::eyre::Result;
use glam::Vec3;
use map_range::MapRange;
use mint::Vector3;
use stardust_xr_fusion::{
    client::{Client, FrameInfo, RootHandler},
    core::values::Transform,
    drawable::{LinePoint, Lines},
    fields::CylinderField,
    input::{
        action::{BaseInputAction, InputAction, InputActionHandler},
        InputDataType, InputHandler,
    },
    node::NodeError,
    spatial::Spatial,
    HandlerWrapper,
};
use stardust_xr_molecules::SingleActorAction;

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

struct Stroke {
    lines: Lines,
    points: Vec<LinePoint>,
}

pub struct Pen {
    settings: PenSettings,
    root: Spatial,
    _field: CylinderField,
    input: HandlerWrapper<InputHandler, InputActionHandler<PenSettings>>,
    hover_action: BaseInputAction<PenSettings>,
    grab_action: SingleActorAction<PenSettings>,
    draw_action: BaseInputAction<PenSettings>,
    _visuals: Lines,
    strokes: Vec<Stroke>,
}
impl Pen {
    pub fn new(client: &Client, settings: PenSettings) -> Result<Self, NodeError> {
        let visual_length = 0.075;
        let root = Spatial::create(client.get_root(), Transform::none(), true)?;
        let field = CylinderField::create(
            &root,
            Transform::from_position([0.0, 0.0, visual_length * 0.5]),
            visual_length,
            settings.thickness * 0.5,
        )?;
        let input = InputHandler::create(client.get_root(), Transform::none(), &field)?
            .wrap(InputActionHandler::new(settings))?;
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
            &root,
            Transform::none(),
            &[
                LinePoint {
                    point: [0.0; 3].into(),
                    thickness: 0.0,
                    color: color::rgba!(1.0, 0.0, 0.0, 0.0),
                },
                LinePoint {
                    point: [0.0, 0.0, settings.thickness].into(),
                    thickness: settings.thickness,
                    color: color::rgba!(1.0, 0.0, 0.0, 1.0),
                },
                LinePoint {
                    point: [0.0, 0.0, visual_length].into(),
                    thickness: settings.thickness,
                    color: color::rgba!(1.0, 0.0, 0.0, 1.0),
                },
            ],
            false,
        )?;
        Ok(Self {
            settings,
            root,
            _field: field,
            input,
            hover_action,
            grab_action,
            draw_action,
            _visuals: visuals,
            strokes: Vec::new(),
        })
    }
}
impl RootHandler for Pen {
    fn frame(&mut self, _info: FrameInfo) {
        self.input.lock_wrapped().update_actions([
            self.hover_action.type_erase(),
            self.draw_action.type_erase(),
            self.grab_action.type_erase(),
        ]);
        self.grab_action.update(&mut self.hover_action);

        if self.grab_action.actor_started() {
            let _ = self.root.set_zoneable(false);
        }
        if self.grab_action.actor_stopped() {
            let _ = self.root.set_zoneable(true);
        }
        let Some(grab_actor) = self.grab_action.actor() else {return};
        let transform = match &grab_actor.input {
            InputDataType::Hand(h) => Transform::from_position_rotation(
                (Vec3::from(h.thumb.tip.position) + Vec3::from(h.index.tip.position)) * 0.5,
                h.palm.rotation,
            ),
            InputDataType::Tip(t) => Transform::from_position_rotation(t.origin, t.orientation),
            _ => Transform::none(),
        };
        let _ = self.root.set_transform(Some(self.input.node()), transform);

        if self.draw_action.started_acting.contains(grab_actor) {
            self.strokes.push(Stroke {
                lines: Lines::create(self.input.node(), Transform::identity(), &[], false).unwrap(),
                points: vec![],
            });
        }
        if let Some(draw_actor) = self.draw_action.actively_acting.get(grab_actor) {
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
            let current_stroke = self.strokes.last_mut().unwrap();
            current_stroke.points.push(LinePoint {
                point,
                thickness: self.settings.thickness * strength,
                color: rgba!(1.0, 0.0, 0.0, 1.0),
            });
            current_stroke
                .lines
                .update_points(&current_stroke.points)
                .unwrap();
        }
    }
}
