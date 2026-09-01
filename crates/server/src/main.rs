mod ai;
mod config;
mod grid;
mod life;
mod metrics;

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
use grid::FoodGrid;
use life::{random_position, spawn_organism};
use metrics::TickClock;

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
        app.init_resource::<FoodGrid>();
        app.init_resource::<TickClock>();
        app.init_resource::<PollutionField>();
        app.insert_resource(SeasonWatch(Season::Bloom));
        app.add_systems(Startup, start_server);
        // Порядок несущий: движение → расталкивание → еда → бой. Тело сначала
        // оказывается там, где оказалось, потом перестаёт занимать чужую воду,
        // и только после этого ест и дерётся — иначе можно съесть то, до чего
        // на самом деле не дотянулся.
        //
        // Сетка еды пересобирается в начале тика: ей пользуются и боты при
        // выборе цели, и кормление.
        // Сгруппировано по смыслу, но порядок остаётся сквозным: каждая группа
        // упорядочена внутри себя и относительно соседних.
        app.add_systems(
            FixedUpdate,
            (
                // Кто где оказался.
                (
                    metrics::tick_begin,
                    advance_environment,
                    rebuild_food_grid,
                    ai::bot_perception,
                    ai::bot_movement,
                    ai::bot_mutation,
                    movement,
                    life::separate_bodies,
                )
                    .chain(),
                // Что с ним из-за этого случилось.
                (
                    feeding,
                    life::combat,
                    life::emit_toxins,
                    life::decay_toxins,
                    // Пачкают воду до того, как она их травит: иначе организм
                    // успевает уплыть из грязи, которую сам только что оставил.
                    life::pollute,
                    life::survival,
                )
                    .chain(),
                // Кто родился, кто умер, чем зарос мир.
                (life::divide, life::deaths, life::respawn, spawn_food, ai::maintain_wild).chain(),
                // Что об этом узнают наружу.
                (
                    census_log,
                    project_state,
                    life::project_pollution,
                    broadcast_state,
                    metrics::tick_end,
                )
                    .chain(),
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

    // Карта загрязнения — одна на мир, поэтому и сущность у неё одна. Компонент
    // на каждом организме означал бы семьдесят копий одного и того же.
    commands.spawn((
        Name::new("Вода"),
        Pollution::default(),
        Replicate::to_clients(NetworkTarget::All),
    ));

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

/// Пересобирает сетку еды на начало тика.
///
/// Одна сетка на всех: раньше `feeding` строила свою и выбрасывала её в конце
/// системы, а `bot_perception` отдельно копировала позиции всех частиц, чтобы
/// каждый бот прошёл их линейно.
fn rebuild_food_grid(mut grid: ResMut<FoodGrid>, nutrients: Query<(Entity, &FoodPosition, &Nutrient)>) {
    grid.rebuild(nutrients.iter().map(|(entity, position, nutrient)| {
        (entity, position.0, nutrient.energy)
    }));
}

/// Server-authoritative feeding. A nutrient is consumed by the first organism whose
/// mouth reaches it; eating is never predicted, so nothing can flicker back.
fn feeding(
    mut commands: Commands,
    config: Res<ServerConfig>,
    mut grid: ResMut<FoodGrid>,
    mut organisms: Query<(&PlayerPosition, &mut OrganismState, &mut PlayerProgress)>,
) {
    if grid.is_empty() {
        return;
    }
    let mut eaten: Vec<Entity> = Vec::new();
    for (position, mut organism, mut progress) in &mut organisms {
        if progress.dead {
            continue;
        }
        let Some(reach) = feeding_reach(&organism) else { continue; };
        let cap = organism.energy_cap();
        let mut absorbed = 0.0;
        let mut bites = 0u32;
        grid.for_each_near(position.0, reach, |entry| {
            // Флаг в самой сетке вместо поиска по списку съеденного: частица,
            // доставшаяся одному рту, не достанется никакому другому в этом тике.
            entry.taken = true;
            absorbed += entry.energy;
            bites += 1;
            eaten.push(entry.entity);
        });
        if bites > 0 {
            organism.energy = (organism.energy + absorbed).min(cap);
            organism.absorbed += absorbed;
            // Поел — значит рана снова заживает: не нужно ждать, пока
            // истечёт боевой откат, если ты сумел поесть под огнём.
            organism.combat_timer = 0.0;
            progress.bites = progress.bites.wrapping_add(bites);
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
    grid: Res<FoodGrid>,
    mut budget: Local<f32>,
) {
    // Сколько еды в воде, сетка уже знает: она пересобрана в начале тика.
    let target = (config.food_target as f32 * env.food_density) as usize;
    let live = grid.len();
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

/// Насколько должно сдвинуться число, чтобы это стоило пакета.
///
/// Присваивание помечает компонент изменённым, а изменённый компонент уходит по
/// сети на ближайшей отправке. Здоровье восстанавливается по два в секунду, то
/// есть за тик меняется на три сотых — и раньше каждая такая сотая улетала всем
/// клиентам. Порог мельче, чем видно на полоске, но превращает поток из
/// пятидесяти обновлений в секунду в несколько.
const VITALS_EPSILON: f32 = 0.25;
/// То же для энергии: её шкала — сотня с лишним, полделения не видно.
const ENERGY_EPSILON: f32 = 0.5;

/// Copies the authoritative state into the replicated projection, only when it
/// actually changed, so unchanged organisms cost no bandwidth.
fn project_state(
    config: Res<ServerConfig>,
    mut query: Query<(
        &OrganismState,
        &mut PlayerGenome,
        &mut PlayerVitals,
        &mut PlayerProgress,
        Option<&mut PlayerEnergy>,
    )>,
) {
    for (organism, mut genome, mut vitals, mut progress, energy) in &mut query {
        if genome.0 != organism.genome {
            genome.0 = organism.genome.clone();
        }
        // Масса меняется только при мутации, здоровье — непрерывно; сравнение
        // с порогом покрывает оба случая.
        if (vitals.mass - organism.mass).abs() > f32::EPSILON
            || (vitals.health - organism.health).abs() > VITALS_EPSILON
            // Ноль и полный запас должны доезжать точно: на них смотрит игрок.
            || (organism.health <= 0.0 && vitals.health > 0.0)
            || (organism.health >= MAX_HEALTH && vitals.health < MAX_HEALTH)
        {
            *vitals = PlayerVitals { mass: organism.mass, health: organism.health };
        }
        // Энергия висит только на организмах игроков — у ботов её никто не
        // спрашивает, и незачем гонять по сети самое часто меняющееся число.
        if let Some(mut energy) = energy {
            let cap = organism.energy_cap();
            if (energy.energy - organism.energy).abs() > ENERGY_EPSILON
                || (energy.cap - cap).abs() > f32::EPSILON
                || (organism.energy <= 0.0 && energy.energy > 0.0)
            {
                *energy = PlayerEnergy { energy: organism.energy, cap };
            }
        }
        if progress.points != organism.genome.mutation_points {
            progress.points = organism.genome.mutation_points;
        }
        if progress.max_parts != config.max_parts as u16 {
            progress.max_parts = config.max_parts as u16;
        }
    }
}

/// Кому и по чему сервер пишет: адрес клиента и два его почтовых ящика.
type ClientChannels<'a> = (
    &'a RemoteId,
    &'a mut MessageSender<WorldUpdate>,
    &'a mut MessageSender<ServerStats>,
);

/// Как часто клиент узнаёт о воде вокруг себя.
const WORLD_UPDATE_INTERVAL: f32 = 0.1;
/// Как часто клиент получает сводку о самочувствии сервера.
const STATS_INTERVAL: f32 = 0.5;

/// Рассылает клиентам то, чему не место в репликации компонентов: воду вокруг
/// игрока и сводку о сервере.
///
/// Среда раньше была компонентом на каждом организме — семьдесят копий одного и
/// того же ради одной, которую читает клиент. Здесь она уходит по одному
/// сообщению на клиента десять раз в секунду, и только `toxin` в нём
/// действительно свой: он зависит от того, в каком облаке стоит тело.
#[allow(clippy::too_many_arguments)]
fn broadcast_state(
    time: Res<Time>,
    env: Res<Environment>,
    clock: Res<TickClock>,
    grid: Res<FoodGrid>,
    clouds: Query<&ToxinCloud>,
    field: Res<PollutionField>,
    organisms: Query<(&PlayerId, &PlayerPosition)>,
    census: Query<(), With<PlayerGenome>>,
    mut clients: Query<ClientChannels, With<ClientOf>>,
    mut world_due: Local<f32>,
    mut stats_due: Local<f32>,
) {
    let dt = time.delta_secs();
    *world_due -= dt;
    *stats_due -= dt;
    let send_world = *world_due <= 0.0;
    let send_stats = *stats_due <= 0.0;
    if !send_world && !send_stats {
        return;
    }
    if clients.is_empty() {
        // Таймеры всё равно надо двигать, иначе первый подключившийся получит
        // залп из всего, что накопилось, пока сервер стоял пустым.
        if send_world {
            *world_due = WORLD_UPDATE_INTERVAL;
        }
        if send_stats {
            *stats_due = STATS_INTERVAL;
        }
        return;
    }

    let clouds: Vec<ToxinCloud> = clouds.iter().copied().collect();
    let stats = send_stats.then(|| {
        metrics::snapshot(
            &clock,
            census.iter().count(),
            grid.len(),
            clouds.len(),
            clients.iter().len(),
        )
    });

    for (remote, mut world, mut summary) in &mut clients {
        if send_world {
            // The reported toxin level is the local one, clouds included: the HUD
            // should show the water the organism is actually in.
            let local_toxin = organisms
                .iter()
                .find(|(id, _)| id.0 == remote.0)
                .map(|(_, position)| {
                    clouds.iter().map(|c| c.toxin_at(position.0)).sum::<f32>()
                        + field.at(position.0) * POLLUTION_MAX_TOXIN
                })
                .unwrap_or(0.0);
            world.send::<StateChannel>(WorldUpdate {
                season: env.season,
                temperature: env.temperature,
                salinity: env.salinity,
                oxygen: env.oxygen,
                toxin: env.toxin_level + local_toxin,
            });
        }
        if let Some(stats) = stats {
            summary.send::<StateChannel>(stats);
        }
    }

    if send_world {
        *world_due = WORLD_UPDATE_INTERVAL;
    }
    if send_stats {
        *stats_due = STATS_INTERVAL;
    }
}

/// Population report every 30 seconds: how many organisms, how many lines, and
/// how big the largest colony has grown.
fn census_log(
    config: Res<ServerConfig>,
    time: Res<Time>,
    clock: Res<TickClock>,
    grid: Res<FoodGrid>,
    field: Res<PollutionField>,
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
    let mut total = 0;
    for genome in &organisms {
        *per_lineage.entry(genome.0.lineage).or_default() += 1;
        generation = generation.max(genome.0.generation);
        parts = parts.max(genome.0.parts.len());
        total += 1;
    }
    info!(
        "перепись: организмов {} (предел {}), родов {}, крупнейшая колония {} (предел {}), \
         поколение до {}, частей до {}, облаков {}",
        total,
        config.max_organisms,
        per_lineage.len(),
        per_lineage.values().copied().max().unwrap_or(0),
        config.max_colony_size,
        generation,
        parts,
        clouds.iter().count()
    );
    if field.worst() > 0.05 {
        info!(
            "вода: самая грязная клетка {:.0}% (это {:.2} яда, стойкость голого тела {:.2})",
            field.worst() * 100.0,
            field.worst() * POLLUTION_MAX_TOXIN,
            BASE_TOXIN_RESISTANCE
        );
    }
    // Та же сводка, что уходит в оверлей по F1, но её видно и без клиента.
    // Пик важнее среднего: он показывает, насколько близко тик подходит к своему
    // бюджету в худшем случае, а не в среднем по секунде.
    let budget = tick_duration().as_secs_f32() * 1000.0;
    info!(
        "тик: {:.2} мс сред, {:.2} пик из {:.1} бюджета ({:.0}% занято), {:.1} тиков/с, еды {}",
        clock.avg_ms,
        clock.peak_ms,
        budget,
        clock.avg_ms / budget * 100.0,
        clock.ticks_per_second,
        grid.len()
    );
}
