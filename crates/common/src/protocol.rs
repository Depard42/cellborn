use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{FoodPosition, Genome, Nutrient, OrganismState, PartKind, Pollution, ToxinCloud};

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

/// Что видно у любого тела со стороны: насколько оно большое и насколько живое.
///
/// Энергии здесь нет намеренно. Она утекает непрерывно, то есть менялась бы
/// каждый тик и улетала бы по сети всем клиентам про все семьдесят организмов —
/// а нужна она только владельцу, для собственной полоски. Она живёт в
/// [`PlayerEnergy`], который висит только на организмах игроков.
#[derive(Component, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlayerVitals {
    pub mass: f32,
    pub health: f32,
}

/// Запас энергии — только у организмов, которыми управляют игроки.
///
/// Ботам он не нужен ни на клиенте, ни в интерфейсе: клиент рисует чужим телам
/// массу и здоровье. Компонента просто нет на теле бота, поэтому самое часто
/// меняющееся число в игре перестаёт реплицироваться семьюдесятью копиями.
#[derive(Component, Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlayerEnergy {
    pub energy: f32,
    pub cap: f32,
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

/// Вода вокруг игрока: сезон, три глобальных давления среды и один локальный яд.
///
/// Сообщение, а не компонент. Компонентом это ехало по одной копии на каждый
/// организм в мире, хотя клиент читает ровно одну — свою; из пяти полей четыре
/// у всех одинаковы, а различается только `toxin`, который зависит от того, в
/// каком облаке стоит конкретное тело.
///
/// На клиенте последнее принятое значение лежит ресурсом того же типа.
#[derive(Resource, Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct WorldUpdate {
    pub season: crate::Season,
    pub temperature: f32,
    pub salinity: f32,
    pub oxygen: f32,
    /// Локальный уровень: сезонный фон плюс облака, в которых стоит организм.
    pub toxin: f32,
    /// Пороги вражды, по которым судит **этот** сервер.
    ///
    /// Едут клиенту, потому что иначе интерфейс врёт. Раньше клиент красил тела
    /// по вшитым в себя числам, а дрался сервер по своим из конфига: стоило
    /// администратору поменять порог, и зелёная «родня» начинала наносить урон.
    pub aggression_threshold: u32,
    pub kin_split_threshold: u32,
}

impl Default for WorldUpdate {
    fn default() -> Self {
        Self {
            season: crate::Season::Bloom,
            temperature: 0.5,
            salinity: 0.5,
            oxygen: 0.8,
            toxin: 0.05,
            aggression_threshold: crate::AGGRESSION_THRESHOLD,
            kin_split_threshold: crate::KIN_SPLIT_THRESHOLD,
        }
    }
}

/// Что сервер думает о собственном самочувствии — для оверлея по F1.
///
/// Пиковое время тика важнее среднего: провал в 40 мс раз в секунду среднее
/// почти не двигает, а в игре чувствуется. Счётчики мира едут вместе с ним,
/// чтобы клиент мог положить их рядом со своими: расхождение между «еды на
/// сервере» и «еды у клиента» — это и есть отставание репликации.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ServerStats {
    /// Длительность последнего тика симуляции.
    pub tick_ms: f32,
    /// Среднее и пик за окно наблюдения.
    pub tick_ms_avg: f32,
    pub tick_ms_peak: f32,
    /// Сколько тиков в секунду сервер успевает делать на самом деле.
    pub ticks_per_second: f32,
    pub organisms: u16,
    pub food: u16,
    pub clouds: u16,
    pub clients: u16,
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

/// Сервер → клиент, для того, что теряется без последствий.
///
/// `SequencedUnreliable`: и среда, и метрики шлются несколько раз в секунду,
/// потерянный пакет догонится следующим, а вот опоздавший старый пакет принять
/// нельзя — сезон мигнёт назад. Секвенированный канал такие просто отбрасывает.
pub struct StateChannel;

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

        app.add_channel::<StateChannel>(ChannelSettings {
            mode: ChannelMode::SequencedUnreliable,
            ..default()
        })
        .add_direction(NetworkDirection::ServerToClient);

        app.register_message::<MutationRequest>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<WorldUpdate>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<ServerStats>()
            .add_direction(NetworkDirection::ServerToClient);

        app.component::<PlayerId>().replicate();
        app.component::<PlayerPosition>()
            .replicate()
            .predict()
            .add_linear_interpolation();
        app.component::<PlayerGenome>().replicate();
        app.component::<PlayerVitals>().replicate();
        app.component::<PlayerEnergy>().replicate();
        app.component::<PlayerProgress>().replicate();
        app.component::<Nutrient>().replicate();
        app.component::<FoodPosition>().replicate();
        app.component::<ToxinCloud>().replicate();
        app.component::<Pollution>().replicate();
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
        PlayerVitals { mass: state.mass, health: state.health },
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
    )
}
