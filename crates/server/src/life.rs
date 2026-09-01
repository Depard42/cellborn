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
}

impl Default for BotState {
    fn default() -> Self {
        Self { wander: Vec3::ZERO, retarget: 0.0, mutate_in: WILD_MUTATION_INTERVAL }
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

/// Contact damage between organisms that are far enough apart genetically.
///
/// Kin never fight, however different they have grown: the lineage check comes
/// before the distance check in [`hostile`].
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
    let snapshot: Vec<(Entity, Vec3, f32, f32, f32, Genome, bool)> = organisms
        .iter()
        .map(|(e, pos, state, progress)| {
            (
                e,
                pos.0,
                body_radius(state.mass),
                attack_power_with(&state, config.base_attack),
                defense(&state),
                state.genome.clone(),
                progress.dead,
            )
        })
        .collect();

    let mut damage: Vec<(Entity, Entity, f32)> = Vec::new();
    for i in 0..snapshot.len() {
        for j in (i + 1)..snapshot.len() {
            let (a, b) = (&snapshot[i], &snapshot[j]);
            if a.6 || b.6 {
                continue;
            }
            if !hostile_with(&a.5, &b.5, config.aggression_threshold, config.kin_split_threshold) {
                continue;
            }
            let reach = a.2 + b.2 + config.attack_margin;
            if a.1.distance_squared(b.1) > reach * reach {
                continue;
            }
            damage.push((b.0, a.0, a.3 * (1.0 - b.4) * dt));
            damage.push((a.0, b.0, b.3 * (1.0 - a.4) * dt));
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
        cloud.radius = config.toxin_radius * (1.4 - 0.4 * fade);
        cloud.strength = cloud.strength.min(fade * 0.5 + 0.05);
    }
}

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
) {
    let dt = time.delta_secs();
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
