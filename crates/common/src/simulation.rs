use bevy::prelude::*;

use crate::balance::*;
use crate::{stats, Biome, Direction, Environment, OrganismState, PartFamily, PartKind, PartLevel, BASE_MASS};

/// Half-size of the playable area on the X/Z axes.
pub const ARENA_HALF_EXTENT: f32 = 70.0;

/// How much two bodies may sink into each other before they stop.
///
/// Not zero: cells are soft, and a hard constraint at exactly the sum of radii
/// looks like billiard balls. They squash a little, then hold.
pub const BODY_SOFTNESS: f32 = 0.18;

/// How far `self` has to move to stop overlapping `other`, or `None` if they are
/// already apart.
///
/// Both the server and the client's prediction call this, so bodies cannot end up
/// intersecting on one side and separated on the other.
pub fn overlap_push(
    self_position: Vec3,
    self_radius: f32,
    other_position: Vec3,
    other_radius: f32,
) -> Option<Vec3> {
    let touching = (self_radius + other_radius) * (1.0 - BODY_SOFTNESS);
    let offset = Vec3::new(
        self_position.x - other_position.x,
        0.0,
        self_position.z - other_position.z,
    );
    let distance = offset.length();
    if distance >= touching {
        return None;
    }
    // Exactly concentric bodies have no direction to separate along; nudge them
    // apart deterministically rather than dividing by zero.
    let direction = if distance > 1e-4 { offset / distance } else { Vec3::X };
    Some(direction * (touching - distance))
}

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

/// Energy per second produced from light.
///
/// Свет зависит от места, а не от времени: на отмели его вдоволь, в бездне
/// почти нет. Фотосинтез из-за этого — выбор биома, а не бесплатное бессмертие.
pub fn photosynthesis_gain(organism: &OrganismState, biome: Biome) -> f32 {
    let rate: f32 = organism.genome.parts.iter().map(|p| stats(p.kind).photosynthesis).sum();
    rate * biome.light()
}

/// Сколько здоровья у тела такой массы.
///
/// Растёт линейно от массы: рост обязан давать что-то, кроме медлительности.
pub fn max_health(mass: f32) -> f32 {
    MAX_HEALTH + (mass - BASE_MASS).max(0.0) * HEALTH_PER_MASS
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

/// Насколько далеко видит тело вообще без органов чувств.
pub const BASE_SENSE_RANGE: f32 = 6.0;

/// Distance at which the client highlights nearby food — и, что важнее, на
/// сколько разгоняется муть вокруг игрока.
pub fn sense_range(organism: &OrganismState) -> f32 {
    BASE_SENSE_RANGE + organism.genome.parts.iter().map(|p| stats(p.kind).sense).sum::<f32>()
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
    let organs: f32 = organism.genome.parts.iter().map(|p| stats(p.kind).attack).sum();
    // Масса добавляет немного: таранить собой кого-то мельче должно работать,
    // но не заменять оружие, иначе шипы теряют смысл.
    let bulk = (organism.mass - BASE_MASS).max(0.0) * ATTACK_PER_MASS;
    base + organs + bulk
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

/// Урон здоровью в секунду от нехватки кислорода.
///
/// Единственное давление среды, которое бьёт по здоровью, а не по кошельку.
/// Всё прочее несоответствие среде остаётся платой энергией: температуру и
/// солёность можно перетерпеть, наевшись впрок, а задохнуться — нельзя.
///
/// Заживление при этом **не блокируется**, в отличие от отравы. Разница
/// намеренная: из ядовитого места надо уплыть, а из шторма уплыть некуда, он
/// накрывает всю арену. Поэтому сытый организм шторм переживает, а голодный —
/// нет, и жабра превращается из экономии в спасение.
pub fn suffocation(organism: &OrganismState, env: &Environment) -> f32 {
    suffocation_with(organism, env, SUFFOCATION_DAMAGE)
}

/// То же с серверным коэффициентом.
pub fn suffocation_with(organism: &OrganismState, env: &Environment, rate: f32) -> f32 {
    let deficit = OXYGEN_COMFORT - env.oxygen - organism.oxygen_affinity;
    (deficit - SUFFOCATION_SLACK).max(0.0) * rate
}

/// Урон здоровью в секунду от **местной** отравы: облаков и грязи.
///
/// `local_toxin` — это только то, что добавлено к воде здесь: чужие облака и
/// загрязнение от скопления. Сезонный фон сюда не входит намеренно.
///
/// Разделение не косметическое. Фон шторма — 0.15 яда при базовой стойкости
/// тела 0.06, и если бы урон считался от полного уровня, каждый шторм убивал бы
/// всё живое на всей арене независимо от того, кто где стоит. Это ровно то, что
/// и случилось при первой попытке: двести отравленных за пять минут.
///
/// Поэтому давления два, и они разные. Сезон давит **энергией** через штраф
/// адаптации — от него спасают жабры, термомембрана, осморегулятор. Местная
/// отрава давит **здоровьем** — от неё спасают стойкость и умение уйти.
///
/// Превышение считается над собственной стойкостью: своя железа делает своё же
/// облако терпимым, в этом её смысл.
pub fn toxin_damage(organism: &OrganismState, local_toxin: f32) -> f32 {
    toxin_damage_with(organism, local_toxin, TOXIN_DAMAGE)
}

/// То же с серверным коэффициентом.
pub fn toxin_damage_with(organism: &OrganismState, local_toxin: f32, rate: f32) -> f32 {
    (local_toxin - organism.toxin_resistance).max(0.0) * rate
}

/// Toxin per second this organism leaks into the water around it.
pub fn toxin_emission(organism: &OrganismState) -> f32 {
    organism.genome.parts.iter().map(|p| stats(p.kind).toxin_emission).sum()
}

/// Случайная мутация. Мембрана исключена: мутация должна менять план тела, а
/// не просто раздувать его.
///
/// Уровень смещён к дешёвым: высокий уровень — это то, во что вкладываются, а
/// не то, что достаётся даром. Совершенный орган у новорождённого бота сделал
/// бы прокачку бессмысленной для всех остальных.
pub fn random_part(roll: u64) -> PartKind {
    let families = PartFamily::ALL.len() - 1; // last one is Membrane
    let family = PartFamily::ALL[(roll as usize) % families];
    let level = match (roll >> 8) % 10 {
        0..=4 => PartLevel::Cheap,
        5..=8 => PartLevel::Plain,
        _ => PartLevel::Fine,
    };
    PartKind::new(family, level)
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
    use crate::{Genome, PartFamily, PartKind, Perk, AGGRESSION_THRESHOLD, KIN_SPLIT_THRESHOLD};

    /// Вода биома — теперь это просто его вода, без всякого времени.
    fn env_for(biome: Biome) -> Environment {
        biome.water()
    }

    /// Guards the headline balance numbers: a starving organism should live for
    /// minutes, not seconds, and no season may be instantly lethal.
    #[test]
    fn starvation_lifetime_is_within_design_range() {
        let organism = OrganismState::default();
        for season in Biome::ALL {
            let env = env_for(season);
            let drain = energy_drain(&organism, &env);
            let lifetime = organism.energy_cap() / drain;
            assert!(
                (45.0..=260.0).contains(&lifetime),
                "{season:?}: жизнь {lifetime:.0}s out of range (drain {drain:.3}/s)"
            );
        }
    }

    /// Кислород — единственное давление среды, бьющее по здоровью, и жабра
    /// должна снимать его целиком, а не экономить копейки. Иначе орган снова
    /// станет украшением, каким был осморегулятор.
    #[test]
    fn one_gill_is_enough_to_stop_choking_in_the_vents() {
        let storm = env_for(Biome::Vents);
        let bare = OrganismState::default();
        let choking = suffocation(&bare, &storm);
        assert!(choking > 0.0, "в шторм голое тело обязано задыхаться");
        // Не казнь: сытый организм заживает быстрее, чем задыхается.
        assert!(
            choking < HEALTH_REGEN,
            "удушье {choking} сильнее заживления {HEALTH_REGEN}: шторм убьёт всех"
        );

        let mut genome = Genome::starter();
        genome.push_part(PartKind::basic(PartFamily::Gill));
        let adapted = OrganismState::from_genome(genome);
        assert_eq!(suffocation(&adapted, &storm), 0.0, "жабра не спасла от удушья");

        // В остальных биомах задыхаться не с чего.
        for season in [Biome::Open, Biome::Shallows, Biome::Brine, Biome::Abyss] {
            assert_eq!(
                suffocation(&bare, &env_for(season)),
                0.0,
                "{season:?}: удушье там, где его быть не должно"
            );
        }
    }

    /// Каждый адаптивный орган обязан окупаться хотя бы в одном сезоне.
    ///
    /// Осморегулятор когда-то не окупался ни в одном: солёность гуляла на 0.15
    /// от середины при терпимости тела 0.16, то есть отклонение никогда не
    /// выходило за допуск и компенсировать было нечего.
    #[test]
    fn every_adaptive_organ_pays_for_itself_somewhere() {
        let bare = OrganismState::default();
        for family in [
            PartFamily::Gill,
            PartFamily::ThermalMembrane,
            PartFamily::Osmoregulator,
            PartFamily::MucusCoat,
        ] {
            let mut genome = Genome::starter();
            genome.push_part(PartKind::basic(family));
            let adapted = OrganismState::from_genome(genome);

            let best = Biome::ALL
                .into_iter()
                .map(|biome| {
                    let env = env_for(biome);
                    energy_drain(&bare, &env) - energy_drain(&adapted, &env)
                })
                .fold(f32::MIN, f32::max);

            assert!(
                best > 0.0,
                "{} не окупается ни в одном сезоне (лучший выигрыш {best})",
                family.name()
            );
        }
    }

    /// Adaptation must be worth its upkeep: a gill has to help in Storm.
    #[test]
    fn gills_help_in_storm() {
        let env = env_for(Biome::Vents);
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
            PartKind::new(PartFamily::Spike, PartLevel::Fine),
            PartKind::new(PartFamily::Gill, PartLevel::Perfect),
            PartKind::new(PartFamily::Divisome, PartLevel::Cheap),
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

    /// Каждый орган на каждом уровне существует, строится и осмыслен.
    #[test]
    fn every_organ_exists_at_every_level_and_is_sane() {
        let all: Vec<PartKind> = PartKind::all().collect();
        assert_eq!(all.len(), PartFamily::ALL.len() * PartLevel::ALL.len());
        assert_eq!(PartKind::COUNT, all.len());
        for kind in all {
            let s = stats(kind);
            assert!(s.cost >= 1, "{} стоит 0 очков", kind.name());
            assert!(s.mass > 0.0, "{} невесома", kind.name());
            assert_eq!(PartKind::from_index(kind.index()), kind, "индекс не круговой");
        }
    }

    /// Прокачка обязана **ощущаться**: следующий уровень заметно сильнее и
    /// дороже, но не настолько тяжелее, чтобы съесть весь выигрыш.
    ///
    /// Это то, ради чего уровни и заменили десять вариантов. Если разница
    /// станет косметической, развитие снова сведётся к «набери побольше
    /// органов» вместо «доведи до ума то, что есть».
    #[test]
    fn each_level_is_a_real_step_up() {
        let mut previous = PartLevel::Cheap.mods();
        for level in PartLevel::ALL.iter().skip(1) {
            let m = level.mods();
            assert!(m.effect > previous.effect * 1.4, "{}: прирост силы незаметен", level.name());
            assert!(m.cost > previous.cost, "{}: не дороже предыдущего", level.name());
            // Масса растёт медленнее силы, иначе прокачка только замедляет.
            assert!(
                m.effect / previous.effect > m.mass / previous.mass,
                "{}: масса съедает весь выигрыш",
                level.name()
            );
            previous = m;
        }

        // Между крайними уровнями разница должна быть в разы, а не в проценты.
        let cheap = PartLevel::Cheap.mods();
        let perfect = PartLevel::Perfect.mods();
        assert!(perfect.effect / cheap.effect > 4.0, "совершенный орган недостаточно лучше");
    }

    /// Рост обязан давать власть, а не только медлительность.
    ///
    /// Раньше набор массы был сплошным проигрышем: тело становилось медленнее,
    /// прожорливее и крупнее как мишень, а взамен не получало ничего. Игрок рос
    /// и чувствовал себя слабее.
    #[test]
    fn growing_makes_you_stronger_not_just_slower() {
        let small = OrganismState::default();
        let mut genome = Genome::starter_of(1);
        for _ in 0..8 {
            genome.push_part(PartKind::basic(PartFamily::Carapace));
        }
        let big = OrganismState::from_genome(genome);
        assert!(big.mass > small.mass * 2.0, "тело не выросло");

        // Крупное труднее убить и бьёт больнее.
        assert!(
            max_health(big.mass) > max_health(small.mass) * 1.5,
            "масса не прибавила живучести"
        );
        assert!(attack_power(&big) > attack_power(&small), "масса не прибавила урона");

        // Цена роста никуда не делась: оно медленнее.
        assert!(movement_speed(&big) < movement_speed(&small), "рост перестал что-либо стоить");
    }

    /// Вложенные очки не должны делать тело более раненым, чем оно было.
    #[test]
    fn an_upgrade_does_not_leave_you_hurt() {
        let mut genome = Genome::starter_of(1);
        genome.push_part(PartKind::cheap(PartFamily::Carapace));
        let mut organism = OrganismState::from_genome(genome);
        organism.genome.mutation_points = 5000;

        let before = organism.health / max_health(organism.mass);
        assert!(organism.apply_upgrade(PartFamily::Carapace));
        let after = organism.health / max_health(organism.mass);
        assert!(
            after >= before - 1e-3,
            "после прокачки доля здоровья упала: {before:.3} -> {after:.3}"
        );

        // И то же при отращивании нового органа.
        let before = organism.health / max_health(organism.mass);
        assert!(organism.apply_mutation(PartKind::basic(PartFamily::Carapace)));
        let after = organism.health / max_health(organism.mass);
        assert!(after >= before - 1e-3, "после мутации доля здоровья упала");
    }

    /// Поднять уровень должно быть дешевле, чем отрастить второй такой же —
    /// иначе прокачки не будет вовсе, все просто продолжат набирать органы.
    #[test]
    fn upgrading_beats_growing_another_one() {
        let mut genome = Genome::starter_of(1);
        genome.push_part(PartKind::cheap(PartFamily::Spike));
        let mut organism = OrganismState::from_genome(genome);
        organism.genome.mutation_points = 500;

        let upgrade = organism.upgrade_price(PartFamily::Spike).expect("шип есть");
        let another = organism.price(PartKind::cheap(PartFamily::Spike));
        assert!(
            upgrade < another,
            "прокачка ({upgrade}) не дешевле второго органа ({another})"
        );

        // И она действительно поднимает уровень, а не молча тратит очки.
        let before = attack_power(&organism);
        assert!(organism.apply_upgrade(PartFamily::Spike), "прокачка не прошла");
        assert!(attack_power(&organism) > before, "после прокачки сила не выросла");
        assert_eq!(organism.genome.parts.len(), 4, "прокачка отрастила лишний орган");

        // Совершенный дальше не растёт.
        for _ in 0..5 {
            organism.apply_upgrade(PartFamily::Spike);
        }
        assert_eq!(organism.upgrade_error(PartFamily::Spike), Some("уже совершенный"));
    }

    /// Рост тела обязан дорожать, иначе поздняя игра — список покупок.
    ///
    /// Цена не обязана расти на **каждом** органе: строгий рост держался
    /// плоской надбавкой, и она же делала прокачку резкой. Требуется другое —
    /// чтобы она никогда не падала и заметно росла на дистанции.
    #[test]
    fn growth_gets_dearer_as_the_body_fills_up() {
        let kind = PartKind::basic(PartFamily::Spike);
        let mut genome = Genome::starter();
        let start = crate::mutation_price(&genome, kind);
        let mut previous = start;

        for _ in 0..8 {
            genome.push_part(PartKind::basic(PartFamily::Cilia));
            let price = crate::mutation_price(&genome, kind);
            assert!(price >= previous, "цена подешевела: {previous} -> {price}");
            previous = price;
        }
        assert!(previous > start, "за восемь органов цена не выросла вовсе");
        // Развитое тело платит заметно больше базовой цены.
        assert!(previous >= stats(kind).cost * 2, "поздний орган стоит не дороже раннего");

        // И темп при этом посильный: путь до двадцати органов не должен стоить
        // втрое дороже пути до десяти в пересчёте на орган.
        let cost_to = |count: usize| {
            let mut genome = Genome::starter();
            let mut total = 0u32;
            for _ in 3..count {
                total += crate::mutation_price(&genome, kind) as u32;
                genome.push_part(PartKind::basic(PartFamily::Cilia));
            }
            total
        };
        let per_organ_10 = cost_to(10) as f32 / 7.0;
        let per_organ_20 = cost_to(20) as f32 / 17.0;
        assert!(
            per_organ_20 < per_organ_10 * 2.0,
            "темп слишком резкий: {per_organ_10:.1} -> {per_organ_20:.1} очков за орган"
        );
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

    /// Гистограмма семейств — кэш, и место семейства в ней должно совпадать с
    /// его местом в `ALL`, иначе дистанция считается между разными ячейками.
    #[test]
    fn slot_matches_all() {
        for (index, family) in PartFamily::ALL.iter().enumerate() {
            assert_eq!(family.slot(), index, "{} стоит не на своём месте", family.name());
        }
    }

    /// Кэш обязан совпадать с честным подсчётом после **любого** изменения тела:
    /// мутации, деления, возрождения. Разойдётся — организмы начнут считать
    /// родню чужаками и наоборот, и это не будет видно нигде, кроме боя.
    #[test]
    fn family_counts_cache_never_drifts() {
        let fresh = |state: &OrganismState| crate::FamilyCounts::of(&state.genome);

        let mut organism = OrganismState::from_genome(Genome::starter_of(3));
        assert_eq!(organism.families, fresh(&organism), "стартовое тело");

        // Мутация: очков даём с запасом, чтобы дошло до предела частей.
        organism.genome.mutation_points = 60_000;
        for kind in [
            PartKind::basic(PartFamily::Spike),
            PartKind::new(PartFamily::Gill, PartLevel::Perfect),
            PartKind::new(PartFamily::Spike, PartLevel::Fine),
            PartKind::basic(PartFamily::Mutator),
        ] {
            assert!(organism.apply_mutation(kind), "мутация {} не прошла", kind.name());
            assert_eq!(organism.families, fresh(&organism), "после {}", kind.name());
        }
        // Два шипа разных вариантов — это два шипа: гистограмма считает семейства.
        assert_eq!(organism.families.get(PartFamily::Spike), 2);

        // Деление: потомок собирается через `from_genome`, значит тоже считается.
        let child = OrganismState::from_genome(crate::conceive(&organism.genome, true, 7));
        assert_eq!(child.families, fresh(&child), "потомок");

        // Отказанная мутация не должна трогать ни геном, ни кэш.
        let mut broke = OrganismState::from_genome(Genome::starter_of(9));
        let before = broke.families;
        assert!(!broke.apply_mutation(PartKind::basic(PartFamily::Carapace)), "нечем платить");
        assert_eq!(broke.families, before);
        assert_eq!(broke.families, fresh(&broke));
    }

    /// Быстрое решение по гистограммам и медленное по геномам — одно правило.
    /// Если они разойдутся, бой и поведение ботов начнут спорить друг с другом.
    #[test]
    fn hostile_counts_agrees_with_hostile_with() {
        let mut bodies: Vec<Genome> = Vec::new();
        for lineage in 0..3u64 {
            for extra in 0..9 {
                let mut genome = Genome::starter_of(lineage);
                for i in 0..extra {
                    genome.push_part(crate::random_part(i * 37 + lineage));
                }
                bodies.push(genome);
            }
        }
        for a in &bodies {
            for b in &bodies {
                let slow = crate::hostile_with(a, b, AGGRESSION_THRESHOLD, KIN_SPLIT_THRESHOLD);
                let fast = crate::hostile_counts(
                    &crate::FamilyCounts::of(a),
                    a.lineage,
                    &crate::FamilyCounts::of(b),
                    b.lineage,
                    AGGRESSION_THRESHOLD,
                    KIN_SPLIT_THRESHOLD,
                );
                assert_eq!(slow, fast, "решения разошлись");
                assert_eq!(
                    crate::genetic_distance(a, b),
                    crate::FamilyCounts::of(a).distance(&crate::FamilyCounts::of(b))
                );
            }
        }
    }

    /// Перезарядка обязана расти с массой: это единственное, чем мелкий лучше
    /// крупного, и без этого выбор «расти или остаться быстрым» перестаёт быть
    /// выбором.
    #[test]
    fn heavy_bodies_recharge_slower() {
        let small = OrganismState::default();
        let mut genome = Genome::starter_of(1);
        for _ in 0..14 {
            genome.push_part(PartKind::basic(PartFamily::Carapace));
        }
        let heavy = OrganismState::from_genome(genome);
        assert!(heavy.mass > small.mass * 3.0, "тело не выросло достаточно");

        for perk in Perk::ALL {
            let quick = perk.cooldown(small.mass);
            let slow = perk.cooldown(heavy.mass);
            assert!(
                slow > quick * 1.5,
                "{}: крупное тело перезаряжается почти так же быстро ({quick:.1} против {slow:.1})",
                perk.name()
            );
        }
    }

    /// Способность тратится и восстанавливается, а не срабатывает бесконечно.
    #[test]
    fn a_perk_goes_on_cooldown_and_comes_back() {
        let mut organism = OrganismState::default();
        assert!(organism.perk_ready(Perk::Squid), "новорождённый без способности");

        organism.spend_perk(Perk::Squid);
        assert!(!organism.perk_ready(Perk::Squid), "способность не потратилась");
        assert!(organism.perk_ready(Perk::Lineage), "потратилась не та способность");
        assert!(organism.perk_readiness(Perk::Squid) < 0.1, "шкала врёт сразу после нажатия");

        // Ждём ровно столько, сколько обещано.
        organism.tick_perks(Perk::Squid.cooldown(organism.mass) + 0.01);
        assert!(organism.perk_ready(Perk::Squid), "способность не вернулась вовремя");
        assert!((organism.perk_readiness(Perk::Squid) - 1.0).abs() < 1e-3);
    }

    /// Bodies must not be able to occupy the same water.
    #[test]
    fn overlapping_bodies_are_pushed_apart() {
        let (a, b) = (Vec3::ZERO, Vec3::new(0.5, 0.0, 0.0));
        let push = overlap_push(a, 1.0, b, 1.0).expect("тела перекрываются");
        assert!(push.x < 0.0, "толкает прочь от соседа");
        // Applying the push ends the overlap.
        assert!(overlap_push(a + push, 1.0, b, 1.0).is_none());
        // Far apart: nothing to resolve.
        assert!(overlap_push(a, 1.0, Vec3::new(9.0, 0.0, 0.0), 1.0).is_none());
        // Concentric: still separates instead of dividing by zero.
        assert!(overlap_push(a, 1.0, a, 1.0).is_some());
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
