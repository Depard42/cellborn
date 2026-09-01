use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{FoodPosition, Genome, Nutrient, OrganismState, PartKind, ToxinCloud};

#[derive(Component, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlayerId(pub PeerId);

#[derive(Component, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlayerPosition(pub Vec3);

impl Ease for PlayerPosition {
    fn interpolating_curve_unbounded(start: Self, end: Self) -> impl Curve<Self> {
        FunctionCurve::new(Interval::EVERYWHERE, move |t| {
            PlayerPosition(start.0.lerp(end.0, t))
        })
    }
}

#[derive(Component, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlayerGenome(pub Genome);

#[derive(Component, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlayerVitals {
    pub mass: f32,
    pub energy: f32,
    pub energy_cap: f32,
    pub health: f32,
}

/// Progress and life state. `bites` is a counter, not a flag: the client compares
/// it against the value it saw last frame to know that a bite happened, which is
/// what triggers the eating animation without needing an event channel.
#[derive(Component, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlayerProgress {
    pub points: u16,
    pub bites: u32,
    pub kills: u16,
    /// Counters, not flags: the client compares them with what it saw last frame
    /// to know that something happened, which drives the effects.
    pub hits: u32,
    pub divisions: u32,
    /// The server's part limit, so the client shows the real ceiling instead of
    /// its own compiled-in guess.
    pub max_parts: u16,
    pub dead: bool,
    pub respawn_in: f32,
}

#[derive(Component, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlayerEnvironment {
    pub season: crate::Season,
    pub temperature: f32,
    pub salinity: f32,
    pub oxygen: f32,
    pub toxin: f32,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Reflect)]
pub struct Direction {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Reflect)]
pub enum Inputs {
    #[default]
    None,
    Direction(Direction),
}

// Required by the native input plugin: inputs may reference entities that need
// to be mapped between the server and client worlds. Ours never do.
impl MapEntities for Inputs {
    fn map_entities<E: EntityMapper>(&mut self, _entity_mapper: &mut E) {}
}

impl Inputs {
    pub fn direction(&self) -> Direction {
        match self {
            Inputs::None => Direction::default(),
            Inputs::Direction(d) => d.clone(),
        }
    }
}

/// Client → server request to grow a part.
///
/// The client sends a `PartKind` and nothing else: never the cost, never its point
/// balance. The server owns both and re-checks everything.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MutationRequest {
    pub kind: PartKind,
}

/// Reliable ordered channel for gameplay requests.
pub struct GameplayChannel;

#[derive(Clone)]
pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(input::native::InputPlugin::<Inputs>::default());

        app.add_channel::<GameplayChannel>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        })
        .add_direction(NetworkDirection::ClientToServer);

        app.register_message::<MutationRequest>()
            .add_direction(NetworkDirection::ClientToServer);

        app.component::<PlayerId>().replicate();
        app.component::<PlayerPosition>()
            .replicate()
            .predict()
            .add_linear_interpolation();
        app.component::<PlayerGenome>().replicate();
        app.component::<PlayerVitals>().replicate();
        app.component::<PlayerProgress>().replicate();
        app.component::<PlayerEnvironment>().replicate();
        app.component::<Nutrient>().replicate();
        app.component::<FoodPosition>().replicate();
        app.component::<ToxinCloud>().replicate();
    }
}

pub fn player_bundle(id: PeerId, state: &OrganismState, position: Vec3) -> impl Bundle {
    (PlayerId(id), organism_bundle(state, position))
}

/// Everything an organism needs on the wire, player or bot.
pub fn organism_bundle(state: &OrganismState, position: Vec3) -> impl Bundle {
    (
        PlayerPosition(position),
        PlayerGenome(state.genome.clone()),
        PlayerVitals {
            mass: state.mass,
            energy: state.energy,
            energy_cap: state.energy_cap(),
            health: state.health,
        },
        PlayerProgress {
            points: state.genome.mutation_points,
            bites: 0,
            kills: 0,
            hits: 0,
            divisions: 0,
            max_parts: crate::MAX_PARTS as u16,
            dead: false,
            respawn_in: 0.0,
        },
        PlayerEnvironment {
            season: crate::Season::Bloom,
            temperature: 0.5,
            salinity: 0.5,
            oxygen: 0.8,
            toxin: 0.05,
        },
    )
}
