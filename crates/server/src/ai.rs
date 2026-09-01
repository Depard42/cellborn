//! Bots.
//!
//! Two kinds, both running the exact same simulation as a player: wild organisms
//! that drift, feed, mutate on their own and hunt whatever is different enough
//! from them, and colony cells — the offspring of a lineage, which keep to their
//! kin and only fight outsiders.

use bevy::prelude::*;
use cellborn_common::*;
use rand::Rng;

use crate::config::ServerConfig;
use crate::life::{random_position, spawn_organism, Brain, BotState};

/// Keeps the arena populated with wild organisms.
pub fn maintain_wild(
    mut commands: Commands,
    config: Res<ServerConfig>,
    wild: Query<&Brain>,
    census: Query<&PlayerGenome>,
) {
    let alive = wild.iter().filter(|b| **b == Brain::Wild).count();
    if alive >= config.wild_target || census.iter().count() >= config.max_organisms {
        return;
    }
    let mut rng = rand::rng();
    // A distinct lineage per wild cell: they are strangers to each other too.
    let mut genome = Genome::starter_of(rng.random::<u64>());
    // Wild cells start slightly varied, so the world is not full of clones.
    for _ in 0..rng.random_range(0..3) {
        genome.push_part(random_part(rng.random::<u64>()));
    }
    let state = OrganismState::from_genome(genome);
    spawn_organism(&mut commands, state, random_position(), None, Some(Brain::Wild));
}

/// Wild organisms drift genetically on their own, without a mutation economy.
pub fn wild_mutation(
    config: Res<ServerConfig>,
    time: Res<Time>,
    mut bots: Query<(&Brain, &mut OrganismState, &mut BotState)>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::rng();
    for (brain, mut organism, mut bot) in &mut bots {
        if *brain != Brain::Wild {
            continue;
        }
        bot.mutate_in -= dt;
        if bot.mutate_in > 0.0 {
            continue;
        }
        bot.mutate_in = config.wild_mutation_interval * rng.random_range(0.6..1.6);
        // Wild growth is free, so it stops well short of the player ceiling.
        if organism.genome.parts.len() >= config.wild_max_parts.min(config.max_parts) {
            continue;
        }
        let kind = random_part(rng.random::<u64>());
        organism.genome.push_part(kind);
        organism.recompute();
    }
}

/// One steering pass: eat when hungry, hunt what you can beat, run from what you
/// cannot, and wander when nothing is going on.
pub fn bot_movement(
    config: Res<ServerConfig>,
    time: Res<Time>,
    nutrients: Query<&FoodPosition, With<Nutrient>>,
    // One set: the snapshot of everyone, then the bots we actually steer. Both
    // touch PlayerPosition, so they cannot be two independent queries.
    mut sets: ParamSet<(
        Query<(Entity, &PlayerPosition, &OrganismState, &PlayerGenome)>,
        Query<(Entity, &Brain, &mut PlayerPosition, &OrganismState, &PlayerProgress, &mut BotState)>,
    )>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::rng();

    // The snapshot carries what an organism can judge by looking: how hard the
    // other one hits, how well it takes a hit, and how much life is left in it.
    let world: Vec<(Entity, Vec3, f32, f32, f32, Genome)> = sets
        .p0()
        .iter()
        .map(|(e, p, s, g)| {
            (e, p.0, attack_power_with(s, config.base_attack), defense(s), s.health, g.0.clone())
        })
        .collect();
    let food: Vec<Vec3> = nutrients.iter().map(|p| p.0).collect();

    for (entity, brain, mut position, organism, progress, mut bot) in &mut sets.p1() {
        if progress.dead {
            continue;
        }
        let here = position.0;
        let my_attack = attack_power_with(organism, config.base_attack);
        let my_defense = defense(organism);
        let wounded = organism.health < MAX_HEALTH * 0.45;
        let hungry = organism.energy < organism.energy_cap() * 0.75;

        let mut goal: Option<Vec3> = None;
        // Threats are summed, not picked: cornered between two enemies, a bot
        // should run out of the pincer rather than straight into the second one.
        let mut escape = Vec3::ZERO;
        let mut best_prey = f32::MAX;

        for (other, other_pos, their_attack, their_defense, their_health, genome) in &world {
            if *other == entity {
                continue;
            }
            let distance = here.distance(*other_pos);
            if distance > config.bot_vision
                || !hostile_with(
                    &organism.genome,
                    genome,
                    config.aggression_threshold,
                    config.kin_split_threshold,
                )
            {
                continue;
            }

            // How long each of us would survive the other. This is the whole
            // judgement: not "who is bigger" but "who runs out of health first".
            let incoming = (their_attack * (1.0 - my_defense)).max(0.01);
            let outgoing = (my_attack * (1.0 - their_defense)).max(0.01);
            let i_last = organism.health / incoming;
            let they_last = their_health / outgoing;

            let losing = i_last < they_last * 1.25;
            if losing || wounded {
                // Closer threats pull harder, so a bot flees the nearest first.
                escape += (here - *other_pos).normalize_or_zero() / distance.max(1.0);
            } else if *brain == Brain::Wild && distance < best_prey && they_last < i_last * 0.7 {
                best_prey = distance;
                goal = Some(*other_pos);
            }
        }

        if goal.is_none() && hungry && escape == Vec3::ZERO {
            let mut nearest = config.bot_vision;
            for food_position in &food {
                let distance = here.distance(*food_position);
                if distance < nearest {
                    nearest = distance;
                    goal = Some(*food_position);
                }
            }
        }

        bot.retarget -= dt;
        if bot.retarget <= 0.0 {
            bot.retarget = rng.random_range(1.5..4.0);
            bot.wander = Vec3::new(rng.random_range(-1.0..1.0), 0.0, rng.random_range(-1.0..1.0))
                .normalize_or(Vec3::X);
        }

        let direction = if escape != Vec3::ZERO {
            // Running away also means not running into a wall.
            let away = escape.normalize_or(bot.wander);
            let inward = -here * 0.05;
            (away + inward).normalize_or(away)
        } else if let Some(target) = goal {
            (target - here).normalize_or(bot.wander)
        } else {
            // Drift back toward the middle rather than hugging the wall.
            (bot.wander - here * 0.02).normalize_or(bot.wander)
        };

        // Fear is fast: a fleeing cell spends everything it has on getting away.
        let speed = movement_speed(organism) * if escape != Vec3::ZERO { 1.15 } else { 1.0 };
        step_movement_vec(&mut position.0, direction, speed, dt);
    }
}
