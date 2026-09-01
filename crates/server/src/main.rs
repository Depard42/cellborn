mod ai;
mod config;
mod life;

use bevy::app::ScheduleRunnerPlugin;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use cellborn_common::*;
use lightyear::connection::host::HostServer;
use lightyear::prelude::input::native::*;
use lightyear::prelude::server::*;
use lightyear::prelude::*;
use rand::Rng;

use config::ServerConfig;
use life::{random_position, spawn_organism};

fn main() {
    let mut app = App::new();
    // Headless server: no window, no rendering, just the fixed-timestep loop.
    app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(tick_duration())));
    app.add_plugins(bevy::log::LogPlugin::default());
    app.add_plugins(bevy::state::app::StatesPlugin);

    // The order matters: plugins, then the protocol, then the server entity.
    app.add_plugins(ServerPlugins { tick_duration: tick_duration() });
    app.add_plugins(ProtocolPlugin);
    app.add_plugins(ServerGamePlugin);

    app.run();
}

/// Listening address for the configured port.
fn bind_addr(port: u16) -> std::net::SocketAddr {
    std::net::SocketAddr::new(std::net::Ipv4Addr::UNSPECIFIED.into(), port)
}

/// Per-client anti-spam state for gameplay requests.
#[derive(Resource, Default)]
struct RequestCooldowns(HashMap<Entity, f32>);

/// Remembers the season we last saw, to award points exactly once per change.
#[derive(Resource)]
struct SeasonWatch(Season);

struct ServerGamePlugin;
impl Plugin for ServerGamePlugin {
    fn build(&self, app: &mut App) {
        let config = config::load();
        // The season length is a world setting, so it comes from the config too.
        let environment = Environment { season_length: config.season_length, ..Default::default() };
        app.insert_resource(config);
        app.insert_resource(environment);
        app.insert_resource(ReplicationMetadata::new(SEND_INTERVAL));
        app.init_resource::<RequestCooldowns>();
        app.insert_resource(SeasonWatch(Season::Bloom));
        app.add_systems(Startup, start_server);
        app.add_systems(
            FixedUpdate,
            (
                advance_environment,
                ai::bot_movement,
                ai::bot_mutation,
                movement,
                life::separate_bodies,
                feeding,
                life::combat,
                life::emit_toxins,
                life::decay_toxins,
                life::survival,
                life::divide,
                life::deaths,
                life::respawn,
                spawn_food,
                ai::maintain_wild,
                census_log,
                project_state,
            )
                .chain(),
        );
        app.add_systems(Update, handle_mutation_requests);
        app.add_observer(on_new_client);
        app.add_observer(on_connected);
    }
}

fn start_server(mut commands: Commands, config: Res<ServerConfig>) {
    let netcode = NetcodeConfig {
        // The server listens on 0.0.0.0 and has no idea which address a client will
        // actually dial: a LAN address, a public one, a hostname. Netcode compares
        // the token's address list against the socket's own address and only makes
        // an exception for the 0.0.0.0 ↔ 127.0.0.1 pair, so anything but localhost
        // is rejected with "server address not in connect token whitelist".
        //
        // The check protects a token minted by a backend from being replayed against
        // a different server. Ours are minted by the client itself with a zero key
        // (see network.rs), so there is nothing here for it to protect yet — it comes
        // back with real connect tokens, where the server does know its public address.
        server_addr_check: false,
        ..NetcodeConfig::default().with_protocol_id(PROTOCOL_ID).with_key(PRIVATE_KEY)
    };
    let server = commands
        .spawn((
            Name::new("Server"),
            NetcodeServer::new(netcode),
            LocalAddr(bind_addr(config.port)),
            ServerUdpIo::default(),
        ))
        .id();
    commands.trigger(Start { entity: server });
    info!("Server listening on {}", bind_addr(config.port));
}

fn on_new_client(trigger: On<Add, LinkOf>, mut commands: Commands) {
    commands.entity(trigger.entity).insert((ReplicationSender, Name::new("Client")));
}

fn on_connected(
    trigger: On<Add, Connected>,
    clients: Query<&RemoteId, With<ClientOf>>,
    mut commands: Commands,
) {
    let Ok(remote) = clients.get(trigger.entity) else { return; };
    // Every player is the founder of their own line: their offspring will never
    // attack them, and they can take over one of them when they die.
    let lineage = rand::rng().random::<u64>();
    let state = OrganismState::from_genome(Genome::starter_of(lineage));
    spawn_organism(
        &mut commands,
        state,
        random_position(),
        Some((remote.0, trigger.entity)),
        None,
    );
    info!("Organism spawned for {:?} (lineage {lineage:x})", remote.0);
}

fn advance_environment(
    config: Res<ServerConfig>,
    mut env: ResMut<Environment>,
    time: Res<Time>,
    mut watch: ResMut<SeasonWatch>,
    mut organisms: Query<&mut OrganismState>,
) {
    env.advance(time.delta_secs());
    if env.season != watch.0 {
        watch.0 = env.season;
        // Surviving a season change is worth points: it is the one reward that
        // makes the seasonal system matter strategically.
        for mut organism in &mut organisms {
            organism.genome.mutation_points =
                organism.genome.mutation_points.saturating_add(config.points_per_season);
        }
        info!("Season is now {}", env.season.name());
    }
}

fn movement(
    host_server: Query<(), With<HostServer>>,
    time: Res<Time>,
    mut query: Query<(
        &mut PlayerPosition,
        &OrganismState,
        &PlayerProgress,
        &ActionState<Inputs>,
        Has<Predicted>,
    )>,
) {
    let is_host = !host_server.is_empty();
    let dt = time.delta_secs();
    for (mut pos, organism, progress, input, predicted) in &mut query {
        // In host-server mode the local player is already simulated by prediction.
        if (is_host && predicted) || progress.dead {
            continue;
        }
        step_movement(&mut pos.0, &input.0.direction(), movement_speed(organism), dt);
    }
}

/// Server-authoritative feeding. A nutrient is consumed by the first organism whose
/// mouth reaches it; eating is never predicted, so nothing can flicker back.
fn feeding(
    mut commands: Commands,
    config: Res<ServerConfig>,
    nutrients: Query<(Entity, &FoodPosition, &Nutrient)>,
    mut organisms: Query<(&PlayerPosition, &mut OrganismState, &mut PlayerProgress)>,
) {
    if nutrients.is_empty() {
        return;
    }
    // Bucket nutrients by grid cell: a full scan per organism melts at scale.
    const CELL: f32 = 4.0;
    let mut grid: HashMap<(i32, i32), Vec<(Entity, Vec3, f32)>> = HashMap::default();
    for (entity, pos, nutrient) in &nutrients {
        let key = ((pos.0.x / CELL).floor() as i32, (pos.0.z / CELL).floor() as i32);
        grid.entry(key).or_default().push((entity, pos.0, nutrient.energy));
    }

    let mut eaten: Vec<Entity> = Vec::new();
    for (position, mut organism, mut progress) in &mut organisms {
        if progress.dead {
            continue;
        }
        let Some(reach) = feeding_reach(&organism) else { continue; };
        let cap = organism.energy_cap();
        let (cx, cz) = ((position.0.x / CELL).floor() as i32, (position.0.z / CELL).floor() as i32);
        let span = (reach / CELL).ceil() as i32;
        for dx in -span..=span {
            for dz in -span..=span {
                let Some(cell) = grid.get(&(cx + dx, cz + dz)) else { continue; };
                for (entity, food_pos, energy) in cell {
                    if eaten.contains(entity) {
                        continue;
                    }
                    if position.0.distance_squared(*food_pos) > reach * reach {
                        continue;
                    }
                    organism.energy = (organism.energy + energy).min(cap);
                    organism.absorbed += energy;
                    // Поел — значит рана снова заживает: не нужно ждать, пока
                    // истечёт боевой откат, если ты сумел поесть под огнём.
                    organism.combat_timer = 0.0;
                    progress.bites = progress.bites.wrapping_add(1);
                    eaten.push(*entity);
                }
            }
        }
        organism.claim_points_at(config.energy_per_mutation_point);
    }

    for entity in eaten {
        commands.entity(entity).despawn();
    }
}

/// Keeps the live nutrient count near the target the season asks for.
fn spawn_food(
    mut commands: Commands,
    config: Res<ServerConfig>,
    env: Res<Environment>,
    time: Res<Time>,
    nutrients: Query<(), With<Nutrient>>,
    mut budget: Local<f32>,
) {
    let target = (config.food_target as f32 * env.food_density) as usize;
    let live = nutrients.iter().count();
    if live >= target {
        *budget = 0.0;
        return;
    }
    *budget += config.food_spawn_rate * time.delta_secs();
    let mut rng = rand::rng();
    let weights: Vec<(FoodKind, f32)> = [FoodKind::Plankton, FoodKind::Algae, FoodKind::Detritus]
        .into_iter()
        .map(|k| (k, k.weight(env.season)))
        .collect();
    let total: f32 = weights.iter().map(|(_, w)| w).sum();

    while *budget >= 1.0 && live + (*budget as usize) < target + 8 {
        *budget -= 1.0;
        let mut roll = rng.random_range(0.0..total);
        let mut kind = FoodKind::Plankton;
        for (k, w) in &weights {
            if roll < *w {
                kind = *k;
                break;
            }
            roll -= w;
        }
        // Cluster food instead of spreading it evenly: clusters are worth swimming to.
        let cluster = Vec3::new(
            rng.random_range(-ARENA_HALF_EXTENT..ARENA_HALF_EXTENT),
            0.0,
            rng.random_range(-ARENA_HALF_EXTENT..ARENA_HALF_EXTENT),
        );
        let jitter = Vec3::new(
            rng.random_range(-2.5..2.5),
            rng.random_range(-0.4..0.9),
            rng.random_range(-2.5..2.5),
        );
        let position = (cluster + jitter).clamp(
            Vec3::new(-ARENA_HALF_EXTENT, -0.6, -ARENA_HALF_EXTENT),
            Vec3::new(ARENA_HALF_EXTENT, 1.2, ARENA_HALF_EXTENT),
        );
        commands.spawn((
            Nutrient { kind, energy: kind.energy() },
            FoodPosition(position),
            Replicate::to_clients(NetworkTarget::All),
        ));
    }
}

/// Validates mutation requests. The client is trusted for nothing here: not the
/// cost, not the balance, not even which organism it is asking about.
fn handle_mutation_requests(
    config: Res<ServerConfig>,
    time: Res<Time>,
    mut cooldowns: ResMut<RequestCooldowns>,
    mut receivers: Query<(Entity, &mut MessageReceiver<MutationRequest>, &RemoteId), With<ClientOf>>,
    mut organisms: Query<(&PlayerId, &mut OrganismState, &PlayerProgress)>,
) {
    let now = time.elapsed_secs();
    for (client, mut receiver, remote) in &mut receivers {
        for request in receiver.receive() {
            let last = cooldowns.0.get(&client).copied().unwrap_or(f32::NEG_INFINITY);
            if now - last < config.mutation_cooldown {
                continue;
            }
            let Some((_, mut organism, progress)) =
                organisms.iter_mut().find(|(id, _, _)| id.0 == remote.0)
            else {
                continue;
            };
            if progress.dead {
                continue;
            }
            // The genome enforces its own ceiling; this is the server's, which may
            // be lower than what the client is drawing against.
            if organism.genome.parts.len() >= config.max_parts {
                continue;
            }
            if organism.apply_mutation(request.kind) {
                cooldowns.0.insert(client, now);
                info!("{:?} grew a {}", remote.0, request.kind.name());
            }
        }
    }
}

/// Copies the authoritative state into the replicated projection, only when it
/// actually changed, so unchanged organisms cost no bandwidth.
fn project_state(
    config: Res<ServerConfig>,
    env: Res<Environment>,
    clouds: Query<&ToxinCloud>,
    mut query: Query<(
        &PlayerPosition,
        &OrganismState,
        &mut PlayerGenome,
        &mut PlayerVitals,
        &mut PlayerProgress,
        &mut PlayerEnvironment,
    )>,
) {
    let clouds: Vec<ToxinCloud> = clouds.iter().copied().collect();
    for (position, organism, mut genome, mut vitals, mut progress, mut player_env) in &mut query {
        if genome.0 != organism.genome {
            genome.0 = organism.genome.clone();
        }
        let new_vitals = PlayerVitals {
            mass: organism.mass,
            energy: organism.energy,
            energy_cap: organism.energy_cap(),
            health: organism.health,
        };
        if *vitals != new_vitals {
            *vitals = new_vitals;
        }
        if progress.points != organism.genome.mutation_points {
            progress.points = organism.genome.mutation_points;
        }
        if progress.max_parts != config.max_parts as u16 {
            progress.max_parts = config.max_parts as u16;
        }
        // The reported toxin level is the local one, clouds included: the HUD
        // should show the water the organism is actually in.
        let local_toxin =
            env.toxin_level + clouds.iter().map(|c| c.toxin_at(position.0)).sum::<f32>();
        let new_env = PlayerEnvironment {
            season: env.season,
            temperature: env.temperature,
            salinity: env.salinity,
            oxygen: env.oxygen,
            toxin: local_toxin,
        };
        if *player_env != new_env {
            *player_env = new_env;
        }
    }
}

/// Population report every 30 seconds: how many organisms, how many lines, and
/// how big the largest colony has grown.
fn census_log(
    config: Res<ServerConfig>,
    time: Res<Time>,
    mut next: Local<f32>,
    organisms: Query<&PlayerGenome>,
    clouds: Query<(), With<ToxinCloud>>,
) {
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + 30.0;

    let mut per_lineage: HashMap<u64, usize> = HashMap::default();
    let mut generation = 0;
    let mut parts = 0;
    for genome in &organisms {
        *per_lineage.entry(genome.0.lineage).or_default() += 1;
        generation = generation.max(genome.0.generation);
        parts = parts.max(genome.0.parts.len());
    }
    info!(
        "перепись: организмов {} (предел {}), родов {}, крупнейшая колония {} (предел {}), \
         поколение до {}, частей до {}, облаков {}",
        organisms.iter().count(),
        config.max_organisms,
        per_lineage.len(),
        per_lineage.values().copied().max().unwrap_or(0),
        config.max_colony_size,
        generation,
        parts,
        clouds.iter().count()
    );
}
