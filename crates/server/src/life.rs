//! Life and death: energy, combat, toxin clouds, division, dying and coming back.
//!
//! Everything here is server-authoritative. The client is told what happened
//! through replicated state and counters; it never decides any of it.

use bevy::prelude::*;
use cellborn_common::*;

use crate::config::ServerConfig;
use lightyear::prelude::*;
use rand::Rng;

/// A cell's readiness to split.
#[derive(Component)]
pub struct Divider {
    pub timer: f32,
}

impl Default for Divider {
    fn default() -> Self {
        // Newborns cannot split immediately, or a colony explodes in one second.
        Self { timer: -6.0 }
    }
}

/// A cell that leaks toxin into the water.
#[derive(Component, Default)]
pub struct Emitter {
    pub timer: f32,
}

/// How long a toxin cloud has left.
#[derive(Component)]
pub struct CloudLife(pub f32);

/// Bots are despawned when they die; players stay and respawn.
#[derive(Component, Clone, Copy, PartialEq)]
pub enum Brain {
    /// Free-living organism: eats, mutates on its own, attacks strangers.
    Wild,
    /// Offspring of a lineage: keeps to its kin.
    Colony,
}

#[derive(Component)]
pub struct BotState {
    pub wander: Vec3,
    pub retarget: f32,
    pub mutate_in: f32,
    /// Evasion state: a weaving phase, its direction, its speed, and the timer
    /// until the next hard change of course.
    pub panic_phase: f32,
    pub panic_side: f32,
    pub panic_rate: f32,
    pub panic_break: f32,
    /// Через сколько секунд бот снова осмотрится.
    pub think_in: f32,
    /// Что он решил, когда смотрел в последний раз.
    pub perception: crate::ai::Perception,
}

impl Default for BotState {
    fn default() -> Self {
        Self {
            wander: Vec3::ZERO,
            retarget: 0.0,
            mutate_in: WILD_MUTATION_INTERVAL,
            panic_phase: 0.0,
            panic_side: 1.0,
            panic_rate: 4.0,
            panic_break: 0.0,
            // Случайная фаза раздумий. Колония рождается пачкой, и без сдвига
            // все её клетки осматривались бы в один и тот же тик: вместо ровной
            // нагрузки сервер получил бы пилу.
            think_in: rand::rng().random_range(0.0..crate::ai::PERCEPTION_PERIOD),
            perception: crate::ai::Perception::default(),
        }
    }
}

/// Spawns an organism, player-controlled or not.
pub fn spawn_organism(
    commands: &mut Commands,
    state: OrganismState,
    position: Vec3,
    owner: Option<(PeerId, Entity)>,
    brain: Option<Brain>,
) -> Entity {
    // Считаем до того, как состояние уйдёт в сущность: дальше его уже не занять.
    let vitals = PlayerEnergy { energy: state.energy, cap: state.energy_cap() };
    let mut entity = commands.spawn((
        organism_bundle(&state, position),
        Divider::default(),
        Emitter::default(),
        Replicate::to_clients(NetworkTarget::All),
    ));
    if toxin_emission(&state) > 0.0 {
        // Emitters start on a random phase so a colony does not pulse in unison.
        entity.insert(Emitter { timer: rand::rng().random_range(0.0..TOXIN_INTERVAL) });
    }
    entity.insert(state);
    match owner {
        Some((peer, link)) => {
            entity.insert((
                PlayerId(peer),
                // Запас энергии есть только у тела игрока: его рисует полоска в
                // интерфейсе. У бота его никто не спрашивает, а меняется оно
                // каждый тик — самое дорогое, что можно реплицировать зря.
                vitals,
                PredictionTarget::to_clients(NetworkTarget::Single(peer)),
                InterpolationTarget::to_clients(NetworkTarget::AllExceptSingle(peer)),
                ControlledBy { owner: link, lifetime: Default::default() },
            ));
        }
        None => {
            entity.insert(InterpolationTarget::to_clients(NetworkTarget::All));
        }
    }
    if let Some(brain) = brain {
        entity.insert((brain, BotState::default()));
    }
    entity.id()
}

pub fn random_position() -> Vec3 {
    let mut rng = rand::rng();
    let edge = ARENA_HALF_EXTENT * 0.85;
    Vec3::new(rng.random_range(-edge..edge), 0.0, rng.random_range(-edge..edge))
}

/// Keeps bodies out of each other.
///
/// Runs after movement, before combat: two cells may touch and hurt each other,
/// but never occupy the same water. Heavier bodies give way less, so a big cell
/// shoves a small one aside rather than being pushed by it.
pub fn separate_bodies(mut organisms: Query<(Entity, &mut PlayerPosition, &OrganismState)>) {
    let snapshot: Vec<(Entity, Vec3, f32, f32)> = organisms
        .iter()
        .map(|(e, p, s)| (e, p.0, body_radius(s.mass), s.mass))
        .collect();

    let mut moves: Vec<(Entity, Vec3)> = Vec::new();
    for i in 0..snapshot.len() {
        for j in (i + 1)..snapshot.len() {
            let (a, b) = (&snapshot[i], &snapshot[j]);
            let Some(push) = overlap_push(a.1, a.2, b.1, b.2) else { continue };
            // Split the correction by mass: the lighter cell moves further.
            let total = a.3 + b.3;
            moves.push((a.0, push * (b.3 / total)));
            moves.push((b.0, -push * (a.3 / total)));
        }
    }

    for (entity, push) in moves {
        let Ok((_, mut position, _)) = organisms.get_mut(entity) else { continue };
        position.0 += push;
        position.0.x = position.0.x.clamp(-ARENA_HALF_EXTENT, ARENA_HALF_EXTENT);
        position.0.z = position.0.z.clamp(-ARENA_HALF_EXTENT, ARENA_HALF_EXTENT);
    }
}

/// Всё, что бой знает об организме: снимок на начало тика.
///
/// Урон симметричен, поэтому его нельзя применять по ходу обхода пар — сначала
/// снимок, потом решения, потом записи.
struct Combatant {
    entity: Entity,
    position: Vec3,
    radius: f32,
    attack: f32,
    defense: f32,
    families: FamilyCounts,
    lineage: u64,
    dead: bool,
}

/// Contact damage between organisms that are far enough apart genetically.
///
/// Kin never fight, however different they have grown — [`hostile_counts`] checks
/// the lineage before the distance it has drifted. Порядок самих проверок в паре
/// обратный: сперва дешёвое расстояние, и только для соприкоснувшихся — родство.
pub fn combat(
    config: Res<ServerConfig>,
    time: Res<Time>,
    mut organisms: Query<(
        Entity,
        &PlayerPosition,
        &mut OrganismState,
        &mut PlayerProgress,
    )>,
) {
    let dt = time.delta_secs();
    // Snapshot first: damage is symmetric, so it cannot be applied while iterating.
    //
    // Геном сюда больше не копируется: для родства нужна гистограмма семейств,
    // которую тело и так держит посчитанной, а это двадцать байт вместо клона
    // вектора частей на каждую особь каждый тик.
    let snapshot: Vec<Combatant> = organisms
        .iter()
        .map(|(entity, pos, state, progress)| Combatant {
            entity,
            position: pos.0,
            radius: body_radius(state.mass),
            attack: attack_power_with(state, config.base_attack),
            defense: defense(state),
            families: state.families,
            lineage: state.genome.lineage,
            dead: progress.dead,
        })
        .collect();

    let mut damage: Vec<(Entity, Entity, f32)> = Vec::new();
    for i in 0..snapshot.len() {
        for j in (i + 1)..snapshot.len() {
            let (a, b) = (&snapshot[i], &snapshot[j]);
            if a.dead || b.dead {
                continue;
            }
            // Расстояние — первым. Оно стоит одно вычитание и отсекает почти
            // всё: пар при полной арене больше двух тысяч, а в контакте всегда
            // единицы. Раньше первой шла проверка родства, то есть самая дорогая
            // операция сервера выполнялась для каждой пары, чтобы почти всегда
            // быть выброшенной следующей строкой.
            let reach = a.radius + b.radius + config.attack_margin;
            if a.position.distance_squared(b.position) > reach * reach {
                continue;
            }
            if !hostile_counts(
                &a.families,
                a.lineage,
                &b.families,
                b.lineage,
                config.aggression_threshold,
                config.kin_split_threshold,
            ) {
                continue;
            }
            damage.push((b.entity, a.entity, a.attack * (1.0 - b.defense) * dt));
            damage.push((a.entity, b.entity, b.attack * (1.0 - a.defense) * dt));
        }
    }

    let mut kills: Vec<(Entity, f32)> = Vec::new();
    for (victim, attacker, amount) in damage {
        let Ok((_, _, mut state, mut progress)) = organisms.get_mut(victim) else { continue; };
        if progress.dead {
            continue;
        }
        state.health = (state.health - amount).max(0.0);
        // Being in a fight stops the healing; otherwise regeneration outruns
        // bare-membrane damage and unarmed organisms never die.
        state.combat_timer = config.combat_regen_block;
        progress.hits = progress.hits.wrapping_add(1);
        if state.health <= 0.0 {
            kills.push((attacker, state.mass));
        }
    }

    // The killer absorbs the body: this is what makes hunting worth its risk.
    for (attacker, mass) in kills {
        let Ok((_, _, mut state, mut progress)) = organisms.get_mut(attacker) else { continue; };
        let cap = state.energy_cap();
        state.energy = (state.energy + mass * config.kill_energy_yield).min(cap);
        state.absorbed += mass * config.kill_energy_yield;
        state.genome.mutation_points =
            state.genome.mutation_points.saturating_add(config.points_per_kill);
        progress.kills = progress.kills.saturating_add(1);
    }
}

/// Cells with a gland poison the water around them. The cloud hurts everyone in
/// it, its owner included — the gland's own resistance is what makes that bearable.
pub fn emit_toxins(
    mut commands: Commands,
    config: Res<ServerConfig>,
    time: Res<Time>,
    mut emitters: Query<(&PlayerPosition, &OrganismState, &PlayerProgress, &mut Emitter)>,
) {
    let dt = time.delta_secs();
    for (position, state, progress, mut emitter) in &mut emitters {
        let rate = toxin_emission(state);
        if rate <= 0.0 || progress.dead {
            continue;
        }
        emitter.timer += dt;
        if emitter.timer < config.toxin_interval {
            continue;
        }
        emitter.timer = 0.0;
        commands.spawn((
            ToxinCloud {
                position: position.0,
                radius: config.toxin_radius,
                strength: rate * 4.0,
            },
            CloudLife(config.toxin_lifetime),
            Replicate::to_clients(NetworkTarget::All),
        ));
    }
}

pub fn decay_toxins(
    mut commands: Commands,
    config: Res<ServerConfig>,
    time: Res<Time>,
    mut clouds: Query<(Entity, &mut CloudLife, &mut ToxinCloud)>,
) {
    let dt = time.delta_secs();
    for (entity, mut life, mut cloud) in &mut clouds {
        life.0 -= dt;
        if life.0 <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        // Clouds disperse: they widen and thin out as they age.
        let fade = (life.0 / config.toxin_lifetime).clamp(0.0, 1.0);
        let radius = config.toxin_radius * (1.4 - 0.4 * fade);
        let strength = cloud.strength.min(fade * 0.5 + 0.05);
        // Присваивание помечает компонент изменённым, а изменённый компонент
        // уходит по сети — то есть безусловная запись отправляла каждое облако
        // всем клиентам пятьдесят раз в секунду ради движения радиуса на доли
        // процента. Пишем, только когда есть что показать.
        if (cloud.radius - radius).abs() > CLOUD_EPSILON
            || (cloud.strength - strength).abs() > CLOUD_EPSILON
        {
            cloud.radius = radius;
            cloud.strength = strength;
        }
    }
}

/// Насколько должно измениться облако, чтобы это стоило пакета.
///
/// Облако живёт одиннадцать секунд и за это время расширяется примерно на метр;
/// сотая доля — это шаг мельче, чем видно на экране, но она превращает поток из
/// пятидесяти обновлений в секунду в несколько.
const CLOUD_EPSILON: f32 = 0.01;

pub fn survival(
    config: Res<ServerConfig>,
    env: Res<Environment>,
    time: Res<Time>,
    clouds: Query<&ToxinCloud>,
    mut query: Query<(&PlayerPosition, &mut OrganismState, &PlayerProgress)>,
) {
    let dt = time.delta_secs();
    let clouds: Vec<ToxinCloud> = clouds.iter().copied().collect();
    for (position, mut organism, progress) in &mut query {
        if progress.dead {
            continue;
        }
        organism.age += dt;

        // Local water, not global: a toxin cloud is a place, not a season.
        let mut local = env.clone();
        local.toxin_level += clouds.iter().map(|c| c.toxin_at(position.0)).sum::<f32>();

        let cap = organism.energy_cap();
        let drain =
            energy_drain_with(&organism, &local, config.base_upkeep, config.penalty_upkeep);
        let gain = photosynthesis_gain(&organism, &local);
        organism.energy = (organism.energy + (gain - drain) * dt).clamp(0.0, cap);

        organism.combat_timer = (organism.combat_timer - dt).max(0.0);
        if organism.energy <= 0.0 {
            organism.health = (organism.health - config.starvation_damage * dt).max(0.0);
        } else if organism.energy > cap * config.well_fed_fraction && organism.combat_timer <= 0.0 {
            organism.health = (organism.health + config.health_regen * dt).min(MAX_HEALTH);
        }
    }
}

/// Как часто сервер вообще думает о делении.
///
/// Между делениями проходят десятки секунд, а перепись колонии стоит прохода по
/// всем организмам. Четыре раза в секунду — предел, за которым разницу уже никто
/// не заметит.
const DIVISION_CHECK_INTERVAL: f32 = 0.25;

/// Cell division. The offspring inherits the genome and may be born changed.
pub fn divide(
    mut commands: Commands,
    config: Res<ServerConfig>,
    time: Res<Time>,
    mut parents: Query<(
        &PlayerPosition,
        &mut OrganismState,
        &mut PlayerProgress,
        &mut Divider,
        Option<&Brain>,
    )>,
    census: Query<&PlayerGenome>,
    mut since: Local<f32>,
) {
    // Деление — решение на десятки секунд, а перепись ради него шла каждый тик.
    // Накапливаем время и просыпаемся несколько раз в секунду: таймеры получают
    // ровно то же суммарное dt, значит скорость размножения не меняется.
    *since += time.delta_secs();
    if *since < DIVISION_CHECK_INTERVAL {
        return;
    }
    let dt = std::mem::take(&mut *since);

    let total = census.iter().count();
    if total >= config.max_organisms {
        return;
    }
    let mut newborns: Vec<(OrganismState, Vec3)> = Vec::new();

    for (position, mut organism, mut progress, mut divider, brain) in &mut parents {
        divider.timer += dt;
        if progress.dead {
            continue;
        }
        let cap = organism.energy_cap();
        if organism.energy < cap * config.division_energy_fraction {
            continue;
        }
        if divider.timer < division_time_with(&organism, config.base_division_time) {
            continue;
        }
        // A lineage stops growing once it fills its niche, so one colony cannot
        // eat the whole arena.
        let kin = census.iter().filter(|g| g.0.lineage == organism.genome.lineage).count();
        if kin >= config.max_colony_size || total + newborns.len() >= config.max_organisms {
            continue;
        }
        divider.timer = 0.0;

        // Inheritance lives in common and is covered by a test: the child carries
        // every part the parent grew, and may add one of its own.
        let mut rng = rand::rng();
        // A newborn may grow one extra organ, unless the server's ceiling is reached.
        let mutates = rng.random::<f32>()
            < mutation_chance_with(&organism, config.base_mutation_chance)
            && organism.genome.parts.len() < config.max_parts;
        let child_genome = conceive(&organism.genome, mutates, rng.random::<u64>());
        let mut child = OrganismState::from_genome(child_genome);
        let share = organism.energy * config.division_energy_share;
        organism.energy -= share;
        child.energy = share.min(child.energy_cap());
        progress.divisions = progress.divisions.wrapping_add(1);
        debug!(
            "деление: родитель {} частей -> потомок {} частей (поколение {})",
            organism.genome.parts.len(),
            child.genome.parts.len(),
            child.genome.generation
        );

        let offset = Vec3::new(rng.random_range(-1.0..1.0), 0.0, rng.random_range(-1.0..1.0))
            .normalize_or(Vec3::X)
            * (body_radius(organism.mass) + body_radius(child.mass) + 0.4);
        let _ = brain;
        newborns.push((child, position.0 + offset));
    }

    for (child, position) in newborns {
        spawn_organism(&mut commands, child, position, None, Some(Brain::Colony));
    }
}

/// Turns organisms that ran out of health into corpses, and decides what happens
/// next: bots are gone for good, players wait to take over one of their offspring.
pub fn deaths(
    mut commands: Commands,
    config: Res<ServerConfig>,
    mut query: Query<(
        Entity,
        &PlayerPosition,
        &OrganismState,
        &mut PlayerProgress,
        Option<&Brain>,
    )>,
) {
    let mut rng = rand::rng();
    for (entity, position, organism, mut progress, brain) in &mut query {
        if progress.dead || organism.health > 0.0 {
            continue;
        }
        // The body becomes food. This is what makes hunting, and dying, matter.
        let count = config.corpse_nutrients + (organism.mass / 4.0) as usize;
        for _ in 0..count.min(14) {
            let offset =
                Vec3::new(rng.random_range(-1.6..1.6), 0.0, rng.random_range(-1.6..1.6));
            commands.spawn((
                Nutrient { kind: FoodKind::Detritus, energy: FoodKind::Detritus.energy() },
                FoodPosition(position.0 + offset),
                Replicate::to_clients(NetworkTarget::All),
            ));
        }
        // combat_timer is still running if the last damage came from a fight,
        // which is what separates "killed" from "starved" in the log.
        info!(
            "died: {} | поколение {}, частей {}, масса {:.1}{}",
            if organism.combat_timer > 0.0 { "убит" } else { "голод" },
            organism.genome.generation,
            organism.genome.parts.len(),
            organism.mass,
            if brain.is_some() { "" } else { " (игрок)" }
        );
        if brain.is_some() {
            commands.entity(entity).despawn();
        } else {
            progress.dead = true;
            progress.respawn_in = config.respawn_delay;
        }
    }
}

/// A dead player takes over one of its own offspring; with none left, it starts
/// again from a fresh cell.
pub fn respawn(
    mut commands: Commands,
    config: Res<ServerConfig>,
    time: Res<Time>,
    // Players never carry a Brain and bots never carry a PlayerId, so the two
    // queries are disjoint — but that has to be spelled out for the scheduler.
    mut players: Query<
        (&mut PlayerPosition, &mut OrganismState, &mut PlayerProgress, &mut Divider),
        (With<PlayerId>, Without<Brain>),
    >,
    offspring: Query<(Entity, &PlayerPosition, &OrganismState), (With<Brain>, Without<PlayerId>)>,
) {
    let dt = time.delta_secs();
    for (mut position, mut organism, mut progress, mut divider) in &mut players {
        if !progress.dead {
            continue;
        }
        progress.respawn_in -= dt;
        if progress.respawn_in > 0.0 {
            continue;
        }

        let lineage = organism.genome.lineage;
        let heir = offspring
            .iter()
            .filter(|(_, _, state)| state.genome.lineage == lineage)
            // Take the most developed descendant: dying should not cost the line
            // everything it grew while you were alive.
            .max_by_key(|(_, _, state)| state.genome.parts.len());

        match heir {
            Some((entity, heir_position, heir_state)) => {
                let kept =
                    (organism.genome.mutation_points as f32 * config.death_point_retention) as u16;
                let mut inherited = heir_state.clone();
                inherited.genome.mutation_points = kept;
                *organism = inherited;
                position.0 = heir_position.0;
                commands.entity(entity).despawn();
                info!("player took over its own offspring (gen {})", organism.genome.generation);
            }
            None => {
                let kept =
                    (organism.genome.mutation_points as f32 * config.death_point_retention) as u16;
                let mut fresh = OrganismState::from_genome(Genome::starter_of(lineage));
                fresh.genome.mutation_points = kept;
                *organism = fresh;
                position.0 = random_position();
                info!("no offspring left, the line starts over");
            }
        }
        divider.timer = -6.0;
        progress.dead = false;
        progress.respawn_in = 0.0;
    }
}
