mod body;
mod fx;
mod ui;
mod world;

use bevy::post_process::bloom::Bloom;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use cellborn_common::*;
use lightyear::prelude::client::*;
use lightyear::prelude::input::native::*;
use lightyear::prelude::*;
use std::net::SocketAddr;

use world::{palette, SEABED_Y};

/// Camera position relative to the organism it follows.
const CAMERA_OFFSET: Vec3 = Vec3::new(0.0, 11.0, 12.0);

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
    app.init_resource::<ui::MutationSelection>();
    app.add_systems(Startup, ui::load_font);
    app.add_systems(Startup, (setup_camera, world::setup_world, ui::setup_hud).after(ui::load_font));
    app.add_systems(
        Update,
        (
            body::build_bodies,
            body::recolor_bodies,
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
        ),
    );
    app.run();
}

struct ClientGamePlugin;
impl Plugin for ClientGamePlugin {
    fn build(&self, app: &mut App) {
        // Enables prediction/rollback for the entities we control.
        app.insert_resource(PredictionManager::default());
        app.add_systems(Startup, (connect_client, load_fx));
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
    mut query: Query<
        (&mut PlayerPosition, &PlayerGenome, &PlayerProgress, &ActionState<Inputs>),
        With<Predicted>,
    >,
) {
    let dt = time.delta_secs();
    for (mut pos, genome, progress, input) in &mut query {
        if progress.dead {
            continue;
        }
        let organism = OrganismState::from_genome(genome.0.clone());
        step_movement(&mut pos.0, &input.0.direction(), movement_speed(&organism), dt);
    }
}

fn mark_controlled(trigger: On<Add, Controlled>, mut commands: Commands) {
    commands.entity(trigger.entity).insert(InputMarker::<Inputs>::default());
}

fn setup_camera(mut commands: Commands) {
    let p = palette(Season::Bloom);
    commands.spawn((
        Camera3d::default(),
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
    mut camera: Query<&mut Transform, With<Camera3d>>,
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
