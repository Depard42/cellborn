mod atlas;
mod audio;
mod body;
mod crest;
mod debug;
mod discovery;
mod fx;
mod hazards;
mod menu;
mod settings;
mod ui;
mod update;
mod wiki;
mod world;

use bevy::post_process::bloom::Bloom;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use cellborn_common::*;
use lightyear::prelude::client::*;
use lightyear::prelude::input::native::*;
use lightyear::prelude::*;
use std::net::SocketAddr;

use menu::Screen;
use world::{palette, SEABED_Y};

/// Camera position relative to the organism it follows.
const CAMERA_OFFSET: Vec3 = Vec3::new(0.0, 11.0, 12.0);

/// Камера, которая рисует мир в окно.
///
/// С появлением камеры превью в атласе органов их стало две, и `With<Camera3d>`
/// перестал быть однозначным: `single()` возвращал ошибку, камера не следовала
/// за организмом, и игрок смотрел на мир из точки старта.
#[derive(Component)]
pub struct MainCamera;

/// Address of the server, overridable with the first CLI argument.
#[derive(Resource)]
pub struct ServerAddress(pub SocketAddr);

fn main() {
    let server_addr = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse::<SocketAddr>().ok())
        .unwrap_or_else(server_addr);

    let mut app = App::new();
    app.add_plugins(DefaultPlugins);

    // The order matters: plugins, then the protocol, then the client entity.
    app.add_plugins(ClientPlugins { tick_duration: tick_duration() });
    app.add_plugins(ProtocolPlugin);
    app.add_plugins(ClientGamePlugin);

    app.insert_resource(ServerAddress(server_addr));
    // Вода вокруг игрока приходит сообщением; до первого пакета интерфейс рисует
    // значения по умолчанию, а не пустоту.
    app.init_resource::<WorldUpdate>();
    app.init_resource::<ui::MutationSelection>();
    app.init_resource::<menu::WikiSelection>();
    app.init_resource::<atlas::AtlasSelection>();
    app.init_state::<Screen>();

    // Меню и справочник: до нажатия «Играть» клиент не трогает сеть.
    app.add_systems(OnEnter(Screen::Menu), menu::setup_menu);
    app.add_systems(OnExit(Screen::Menu), menu::despawn::<menu::MenuRoot>);
    app.add_systems(OnEnter(Screen::Wiki), menu::setup_wiki);
    app.add_systems(OnExit(Screen::Wiki), menu::despawn::<menu::WikiRoot>);
    app.add_systems(
        Update,
        (menu::menu_input, menu::update_menu_status, menu::volume_input)
            .run_if(in_state(Screen::Menu)),
    );
    // Экран выбора сервера: поиск в сети идёт, только пока он открыт.
    app.add_systems(OnEnter(Screen::Servers), menu::setup_servers);
    app.add_systems(OnExit(Screen::Servers), menu::despawn::<menu::ServersRoot>);
    app.add_systems(
        Update,
        (menu::update_servers, menu::servers_input).run_if(in_state(Screen::Servers)),
    );
    app.add_systems(
        Update,
        (menu::wiki_input, menu::update_wiki, menu::update_atlas, atlas::rebuild_preview)
            .run_if(in_state(Screen::Wiki)),
    );
    app.add_systems(Update, menu::game_escape.run_if(in_state(Screen::Game)));
    app.add_systems(Update, atlas::spin_preview);
    ui::install_font(&mut app);
    // После установки шрифта: панель отладки собирается в `Startup` и просит его.
    app.add_plugins(debug::plugin);
    app.add_plugins(update::plugin);
    app.add_plugins(hazards::plugin);
    app.add_plugins(crest::plugin);
    app.add_plugins(settings::plugin);
    app.add_plugins(audio::plugin);
    app.add_plugins(discovery::plugin);
    app.add_systems(Startup, (setup_camera, world::setup_world, atlas::setup_preview));
    app.add_systems(OnEnter(Screen::Game), (ui::setup_hud, connect_client));
    app.add_systems(OnExit(Screen::Game), menu::despawn::<ui::GameUi>);
    app.add_systems(
        Update,
        (
            body::build_bodies,
            body::recolor_bodies,
            body::deform_on_contact,
            body::animate_bodies,
            body::update_health_bars,
            follow_camera,
            world::spawn_food_visuals,
            world::animate_food,
            world::spawn_cloud_visuals,
            world::animate_clouds,
            world::drift_snow,
            world::sway_kelp,
            world::apply_season,
            fx::update_particles,
        )
            .chain(),
    );
    app.add_systems(
        Update,
        (
            ui::update_hud,
            ui::update_death_overlay,
            ui::toggle_mutation_panel,
            ui::update_mutation_panel,
            ui::mutation_navigation,
            ui::mutation_input,
        )
            .run_if(in_state(Screen::Game)),
    );
    app.run();
}

struct ClientGamePlugin;
impl Plugin for ClientGamePlugin {
    fn build(&self, app: &mut App) {
        // Enables prediction/rollback for the entities we control.
        app.insert_resource(PredictionManager::default());
        app.add_systems(Startup, load_fx);
        app.add_systems(
            FixedPreUpdate,
            buffer_input.in_set(lightyear::prelude::client::input::InputSystems::WriteClientInputs),
        );
        app.add_systems(FixedUpdate, predicted_movement);
        app.add_observer(mark_controlled);
    }
}

fn load_fx(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let assets = fx::FxAssets::load(&mut meshes, &mut materials);
    commands.insert_resource(assets);
}

fn connect_client(mut commands: Commands, server: Res<ServerAddress>) {
    let auth = Authentication::Manual {
        server_addr: server.0,
        // Random id so that several clients can connect from the same machine.
        client_id: rand::random::<u64>(),
        private_key: PRIVATE_KEY,
        protocol_id: PROTOCOL_ID,
    };
    let client = commands
        .spawn((
            Name::new("Client"),
            Client::default(),
            NetcodeClient::new(auth, NetcodeConfig::default())
                .expect("failed to build the netcode client"),
            LocalAddr(client_bind_addr()),
            PeerAddr(server.0),
            UdpIo::default(),
            // Measures RTT/jitter; the timeline sync needs it to align with the server.
            PingManager::default(),
            ReplicationReceiver,
            ReplicationSender,
        ))
        .id();
    commands.trigger(Connect { entity: client });
    info!("Connecting to {}", server.0);
}

fn buffer_input(
    mut query: Query<&mut ActionState<Inputs>, With<InputMarker<Inputs>>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let Ok(mut state) = query.single_mut() else { return; };
    let d = Direction {
        up: keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp),
        down: keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown),
        left: keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft),
        right: keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight),
    };
    state.0 = Inputs::Direction(d);
}

fn predicted_movement(
    time: Res<Time>,
    others: Query<(&PlayerPosition, &PlayerVitals), Without<Predicted>>,
    mut query: Query<
        (&mut PlayerPosition, &PlayerGenome, &PlayerProgress, &ActionState<Inputs>),
        With<Predicted>,
    >,
) {
    let dt = time.delta_secs();
    // Neighbours as we currently see them: the prediction has to bump into the
    // same bodies the server will, or swimming into someone looks like passing
    // through them and then being yanked back.
    let neighbours: Vec<(Vec3, f32)> =
        others.iter().map(|(p, v)| (p.0, body_radius(v.mass))).collect();

    for (mut pos, genome, progress, input) in &mut query {
        if progress.dead {
            continue;
        }
        let organism = OrganismState::from_genome(genome.0.clone());
        step_movement(&mut pos.0, &input.0.direction(), movement_speed(&organism), dt);

        // Only our own body is corrected here; the server moves both sides and
        // the difference is reconciled by the usual rollback.
        let radius = body_radius(organism.mass);
        for (other, other_radius) in &neighbours {
            if let Some(push) = overlap_push(pos.0, radius, *other, *other_radius) {
                pos.0 += push;
            }
        }
        pos.0.x = pos.0.x.clamp(-ARENA_HALF_EXTENT, ARENA_HALF_EXTENT);
        pos.0.z = pos.0.z.clamp(-ARENA_HALF_EXTENT, ARENA_HALF_EXTENT);
    }
}

fn mark_controlled(trigger: On<Add, Controlled>, mut commands: Commands) {
    commands.entity(trigger.entity).insert(InputMarker::<Inputs>::default());
}

fn setup_camera(mut commands: Commands) {
    let p = palette(Season::Bloom);
    commands.spawn((
        MainCamera,
        Camera3d::default(),
        // С появлением камеры превью их стало две, и интерфейсу нужно сказать,
        // какая из них его: иначе он уходит в текстуру, а окно чернеет.
        IsDefaultUiCamera,
        bevy::camera::Hdr,
        Transform::from_translation(CAMERA_OFFSET).looking_at(Vec3::ZERO, Vec3::Y),
        // Underwater depth: everything fades into the water colour with distance.
        DistanceFog {
            color: p.water,
            falloff: FogFalloff::ExponentialSquared { density: p.fog_density },
            ..default()
        },
        // Subtle, for bioluminescent organelles and food.
        Bloom { intensity: 0.18, ..Bloom::NATURAL },
    ));
}

/// Follows the controlled organism and pulls back as it grows, so mass is felt as
/// the world getting smaller.
fn follow_camera(
    time: Res<Time>,
    player: Query<(&PlayerPosition, &PlayerVitals), With<Controlled>>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
) {
    let Ok((player, vitals)) = player.single() else { return; };
    let Ok(mut transform) = camera.single_mut() else { return; };
    let zoom = 1.0 + (body_radius(vitals.mass) - body_radius(BASE_MASS)) * 0.9;
    let target = player.0 + CAMERA_OFFSET * zoom;
    let weight = 1.0 - (-6.0 * time.delta_secs()).exp();
    transform.translation = transform.translation.lerp(target, weight);
    transform.translation.y = transform.translation.y.max(SEABED_Y + 3.0);
    transform.look_at(player.0, Vec3::Y);
}
