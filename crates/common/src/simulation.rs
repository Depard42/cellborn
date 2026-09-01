use bevy::prelude::*;

use crate::balance::*;
use crate::{stats, Direction, Environment, OrganismState, PartFamily, PartKind, PartVariant, Season, BASE_MASS};

/// Half-size of the playable area on the X/Z axes.
pub const ARENA_HALF_EXTENT: f32 = 35.0;

/// Oxygen level below which an organism starts to suffer.
pub const OXYGEN_COMFORT: f32 = 0.80;
/// Baseline hardship even in a perfect environment.
pub const BASE_PENALTY: f32 = 0.10;

/// How badly the environment is treating this organism, in "penalty units".
///
/// Unlike the first version this keeps growing past the tolerance limit, so an
/// environment can actually kill a badly adapted organism instead of merely
/// costing it a fixed amount.
pub fn adaptation_penalty(organism: &OrganismState, env: &Environment) -> f32 {
    let temperature =
        ((env.temperature - 0.5).abs() - organism.temperature_tolerance).max(0.0);
    let salinity = ((env.salinity - 0.5).abs() - organism.salinity_tolerance).max(0.0);
    let toxin = (env.toxin_level - organism.toxin_resistance).max(0.0);
    let oxygen = (OXYGEN_COMFORT - env.oxygen - organism.oxygen_affinity).max(0.0);

    (BASE_PENALTY + temperature * 3.0 + salinity * 2.0 + toxin * 2.5 + oxygen * 2.0)
        .clamp(0.0, 3.0)
}

/// Energy per second the body costs just to exist, before adaptation.
pub fn metabolic_cost(organism: &OrganismState) -> f32 {
    organism.genome.parts.iter().map(|p| stats(p.kind).upkeep).sum()
}

/// Total energy drain per second, with the server's own upkeep numbers.
pub fn energy_drain_with(
    organism: &OrganismState,
    env: &Environment,
    base_upkeep: f32,
    penalty_upkeep: f32,
) -> f32 {
    base_upkeep + adaptation_penalty(organism, env) * penalty_upkeep + metabolic_cost(organism)
}

/// Total energy drain per second at the default balance.
pub fn energy_drain(organism: &OrganismState, env: &Environment) -> f32 {
    energy_drain_with(organism, env, BASE_UPKEEP, PENALTY_UPKEEP)
}

/// How much light reaches the organism this season.
pub fn season_light(season: Season) -> f32 {
    match season {
        Season::Bloom => 0.95,
        Season::Hot => 0.70,
        Season::Storm => 0.20,
        Season::Cold => 0.45,
    }
}

/// Energy per second produced from light. Not a flat constant, so photosynthesis
/// is a seasonal strategy rather than an on/off immortality switch.
pub fn photosynthesis_gain(organism: &OrganismState, env: &Environment) -> f32 {
    let rate: f32 = organism.genome.parts.iter().map(|p| stats(p.kind).photosynthesis).sum();
    rate * season_light(env.season)
}

/// Visible radius of the body, driven by mass. Used by feeding and by the renderer,
/// so what you see is what you can eat with.
pub fn body_radius(mass: f32) -> f32 {
    0.62 + (mass / BASE_MASS).sqrt() * 0.42
}

/// Distance at which the organism can swallow a nutrient. Requires a mouth.
pub fn feeding_reach(organism: &OrganismState) -> Option<f32> {
    let mouths: f32 = organism.genome.parts.iter().map(|p| stats(p.kind).reach).sum();
    if mouths <= 0.0 {
        return None;
    }
    Some(body_radius(organism.mass) + mouths)
}

/// Distance at which the client highlights nearby food.
pub fn sense_range(organism: &OrganismState) -> f32 {
    6.0 + organism.genome.parts.iter().map(|p| stats(p.kind).sense).sum::<f32>()
}

/// Swimming speed in units per second: parts add thrust, mass takes it away.
pub fn movement_speed(organism: &OrganismState) -> f32 {
    let thrust: f32 = organism.genome.parts.iter().map(|p| stats(p.kind).speed).sum();
    ((BASE_SPEED + thrust) / (1.0 + organism.mass * MASS_DRAG)).max(0.5)
}

/// Damage per second this organism deals on contact.
pub fn attack_power(organism: &OrganismState) -> f32 {
    attack_power_with(organism, BASE_ATTACK)
}

/// Same, with the server's configured base damage.
pub fn attack_power_with(organism: &OrganismState, base: f32) -> f32 {
    base + organism.genome.parts.iter().map(|p| stats(p.kind).attack).sum::<f32>()
}

/// Fraction of incoming damage the body absorbs.
pub fn defense(organism: &OrganismState) -> f32 {
    organism
        .genome
        .parts
        .iter()
        .map(|p| stats(p.kind).defense)
        .sum::<f32>()
        .min(0.75)
}

/// Seconds this organism needs between divisions.
pub fn division_time(organism: &OrganismState) -> f32 {
    division_time_with(organism, BASE_DIVISION_TIME)
}

/// Same, with the server's configured base interval.
pub fn division_time_with(organism: &OrganismState, base: f32) -> f32 {
    let boost: f32 = organism.genome.parts.iter().map(|p| stats(p.kind).reproduction).sum();
    // Mass slows division down: a big body costs more to copy.
    (base * (1.0 + organism.mass * 0.02) / (1.0 + boost)).max(6.0)
}

/// Chance that an offspring is born with an extra random part.
pub fn mutation_chance(organism: &OrganismState) -> f32 {
    mutation_chance_with(organism, BASE_MUTATION_CHANCE)
}

/// Same, with the server's configured base chance.
pub fn mutation_chance_with(organism: &OrganismState, base: f32) -> f32 {
    (base + organism.genome.parts.iter().map(|p| stats(p.kind).mutagen).sum::<f32>())
        .clamp(0.0, 0.95)
}

/// Toxin per second this organism leaks into the water around it.
pub fn toxin_emission(organism: &OrganismState) -> f32 {
    organism.genome.parts.iter().map(|p| stats(p.kind).toxin_emission).sum()
}

/// One of the 200 mutations, picked at random. The membrane family is excluded:
/// a mutation should change the body plan, not just inflate it.
pub fn random_part(roll: u64) -> PartKind {
    let families = PartFamily::ALL.len() - 1; // last one is Membrane
    let family = PartFamily::ALL[(roll as usize) % families];
    let variant = PartVariant::ALL[((roll >> 8) as usize) % PartVariant::ALL.len()];
    PartKind::new(family, variant)
}

/// Moves along an arbitrary direction. Bots steer with vectors; players steer with
/// four booleans, and both end up here so the arena rules cannot diverge.
pub fn step_movement_vec(position: &mut Vec3, direction: Vec3, speed: f32, dt: f32) {
    let flat = Vec3::new(direction.x, 0.0, direction.z);
    if flat.length_squared() > 1e-6 {
        *position += flat.normalize() * speed * dt;
    }
    position.x = position.x.clamp(-ARENA_HALF_EXTENT, ARENA_HALF_EXTENT);
    position.z = position.z.clamp(-ARENA_HALF_EXTENT, ARENA_HALF_EXTENT);
}

/// Advances an organism by one tick of movement.
///
/// Client prediction and the server authority must produce identical results,
/// otherwise every tick ends in a rollback correction, so both call this.
pub fn step_movement(position: &mut Vec3, direction: &Direction, speed: f32, dt: f32) {
    let mut v = Vec3::ZERO;
    if direction.up {
        v.z -= 1.0;
    }
    if direction.down {
        v.z += 1.0;
    }
    if direction.left {
        v.x -= 1.0;
    }
    if direction.right {
        v.x += 1.0;
    }
    step_movement_vec(position, v, speed, dt);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Genome, PartFamily, PartKind, AGGRESSION_THRESHOLD, KIN_SPLIT_THRESHOLD};

    fn env_for(season: Season) -> Environment {
        let mut env = Environment::default();
        env.season = season;
        // Advance by zero to let the season's values be applied.
        env.advance(0.0);
        env
    }

    /// Guards the headline balance numbers: a starving organism should live for
    /// minutes, not seconds, and no season may be instantly lethal.
    #[test]
    fn starvation_lifetime_is_within_design_range() {
        let organism = OrganismState::default();
        for season in [Season::Bloom, Season::Hot, Season::Storm, Season::Cold] {
            let env = env_for(season);
            let drain = energy_drain(&organism, &env);
            let lifetime = organism.energy_cap() / drain;
            assert!(
                (45.0..=260.0).contains(&lifetime),
                "{season:?}: lifetime {lifetime:.0}s out of range (drain {drain:.3}/s)"
            );
        }
    }

    /// Adaptation must be worth its upkeep: a gill has to help in Storm.
    #[test]
    fn gills_help_in_storm() {
        let env = env_for(Season::Storm);
        let plain = OrganismState::default();
        let mut genome = Genome::starter();
        genome.push_part(PartKind::basic(PartFamily::Gill));
        genome.push_part(PartKind::basic(PartFamily::Gill));
        let adapted = OrganismState::from_genome(genome);
        assert!(
            adaptation_penalty(&adapted, &env) < adaptation_penalty(&plain, &env),
            "gills must lower the Storm penalty"
        );
    }

    /// Mass has to cost speed, or growth is free.
    #[test]
    fn mass_slows_the_organism() {
        let light = OrganismState::default();
        let mut genome = Genome::starter();
        for _ in 0..4 {
            genome.push_part(PartKind::basic(PartFamily::Spike));
        }
        let heavy = OrganismState::from_genome(genome);
        assert!(heavy.mass > light.mass);
        assert!(movement_speed(&heavy) < movement_speed(&light));
    }

    /// The clamp must be applied identically wherever movement runs.
    #[test]
    fn movement_stays_inside_the_arena() {
        let mut position = Vec3::new(34.0, 0.0, -34.0);
        let dir = Direction { right: true, up: true, ..Default::default() };
        for _ in 0..600 {
            step_movement(&mut position, &dir, 10.0, 1.0 / 64.0);
        }
        assert!(position.x <= ARENA_HALF_EXTENT && position.z >= -ARENA_HALF_EXTENT);
    }

    /// Kin never fight, strangers do once they have drifted far enough apart.
    #[test]
    fn aggression_needs_distance_and_a_different_lineage() {
        let mine = Genome::starter_of(1);
        let mut twin = Genome::starter_of(2);
        assert!(!crate::hostile(&mine, &twin), "near-identical strangers are peaceful");
        for _ in 0..8 {
            twin.push_part(PartKind::basic(PartFamily::Spike));
        }
        assert!(crate::hostile(&mine, &twin), "8 parts apart is past the threshold");
        let mut kin = twin.clone();
        kin.lineage = 1;
        assert!(!crate::hostile(&mine, &kin), "kin never fight, however different");
    }

    /// A divisome has to be worth its upkeep.
    #[test]
    fn divisome_speeds_up_reproduction() {
        let plain = OrganismState::default();
        let mut genome = Genome::starter();
        genome.push_part(PartKind::basic(PartFamily::Divisome));
        let fast = OrganismState::from_genome(genome);
        assert!(division_time(&fast) < division_time(&plain));
    }

    /// A child must carry everything its parent grew, or a lineage means nothing.
    #[test]
    fn offspring_inherit_every_parent_part() {
        let mut parent = Genome::starter_of(7);
        for kind in [
            PartKind::new(PartFamily::Spike, PartVariant::Large),
            PartKind::new(PartFamily::Gill, PartVariant::Refined),
            PartKind::new(PartFamily::Divisome, PartVariant::Twin),
            PartKind::basic(PartFamily::Photosynthesis),
        ] {
            parent.push_part(kind);
        }
        parent.mutation_points = 9;

        let child = crate::conceive(&parent, false, 0);
        assert_eq!(child.lineage, parent.lineage, "род наследуется");
        assert_eq!(child.generation, parent.generation + 1);
        assert_eq!(child.parts.len(), parent.parts.len(), "без мутации — точная копия");
        for part in &parent.parts {
            assert_eq!(
                child.count(part.kind),
                parent.count(part.kind),
                "потомок потерял часть {}",
                part.kind.name()
            );
        }
        assert_eq!(child.mutation_points, 0, "очки не наследуются");
        // Kin by construction: a child can never be an enemy of its parent.
        assert!(!crate::hostile(&parent, &child));

        // With a mutation the child keeps everything and gains exactly one part.
        let mutant = crate::conceive(&parent, true, 12345);
        assert_eq!(mutant.parts.len(), parent.parts.len() + 1);
        for part in &parent.parts {
            assert!(mutant.count(part.kind) >= parent.count(part.kind));
        }
    }

    /// Two unarmed organisms have to be able to kill each other, eventually.
    #[test]
    fn bare_contact_out_damages_regeneration() {
        let plain = OrganismState::default();
        assert!(
            attack_power(&plain) * (1.0 - defense(&plain)) > 0.0,
            "даже голая мембрана наносит урон"
        );
        // Healing is blocked while the fight lasts, so any damage accumulates.
        assert!(COMBAT_REGEN_BLOCK > 0.0);
    }

    /// There really are 200 mutations, and every one of them is buildable.
    #[test]
    fn two_hundred_mutations_exist_and_are_sane() {
        let all: Vec<PartKind> = PartKind::all().collect();
        assert_eq!(all.len(), 200);
        assert_eq!(PartKind::COUNT, 200);
        for kind in all {
            let s = stats(kind);
            assert!(s.cost >= 1, "{} стоит 0 очков", kind.name());
            assert!(s.mass > 0.0, "{} невесома", kind.name());
            assert_eq!(PartKind::from_index(kind.index()), kind);
        }
    }

    /// Each organ has to make the next one dearer, or the late game is a shopping list.
    #[test]
    fn every_mutation_raises_the_price_of_the_next() {
        let kind = PartKind::basic(PartFamily::Spike);
        let mut genome = Genome::starter();
        let mut previous = crate::mutation_price(&genome, kind);
        for _ in 0..8 {
            genome.push_part(PartKind::basic(PartFamily::Cilia));
            let price = crate::mutation_price(&genome, kind);
            assert!(price > previous, "цена должна расти: {previous} -> {price}");
            previous = price;
        }
        // A well-grown body pays several times the base price.
        assert!(previous >= stats(kind).cost * 2);
    }

    /// A lineage is not a permanent truce: a branch that drifted far enough
    /// becomes an enemy of its own family.
    #[test]
    fn kin_split_when_they_drift_far_enough() {
        let parent = Genome::starter_of(42);
        let mut branch = parent.clone();
        for _ in 0..(AGGRESSION_THRESHOLD + 2) {
            branch.push_part(PartKind::basic(PartFamily::Spike));
        }
        assert!(!crate::hostile(&parent, &branch), "родня терпит больше чужаков");

        let mut far = parent.clone();
        for _ in 0..(KIN_SPLIT_THRESHOLD + 1) {
            far.push_part(PartKind::basic(PartFamily::Nematocyst));
        }
        assert_eq!(far.lineage, parent.lineage);
        assert!(crate::hostile(&parent, &far), "род раскалывается после {KIN_SPLIT_THRESHOLD}");
    }

    /// Client prediction and server authority must not drift apart over time.
    #[test]
    fn client_and_server_movement_do_not_diverge() {
        let organism = OrganismState::default();
        let speed = movement_speed(&organism);
        let dir = Direction { up: true, right: true, ..Default::default() };
        let (mut client, mut server) = (Vec3::ZERO, Vec3::ZERO);
        for _ in 0..10_000 {
            step_movement(&mut client, &dir, speed, 1.0 / 64.0);
            step_movement(&mut server, &dir, speed, 1.0 / 64.0);
        }
        assert_eq!(client, server);
    }
}
