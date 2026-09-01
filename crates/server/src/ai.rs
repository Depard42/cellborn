//! Bots.
//!
//! Two kinds, both running the exact same simulation as a player: wild organisms
//! that drift, feed, mutate on their own and hunt whatever is different enough
//! from them, and colony cells — the offspring of a lineage, which keep to their
//! kin and only fight outsiders.
//!
//! Восприятие и движение здесь разделены намеренно. Решение «куда я хочу» — это
//! решение, а не движение: оно принимается несколько раз в секунду, и никто на
//! это не смотрит. Шаг тела остаётся на частоте физики, иначе бот задёргается.

use bevy::prelude::*;
use cellborn_common::*;
use rand::Rng;

use crate::config::ServerConfig;
use crate::grid::FoodGrid;
use crate::life::{random_position, spawn_organism, Brain, BotState};

/// Сколько раз в секунду бот пересматривает, куда плывёт.
///
/// Шестьдесят четыре раза в секунду он делал это раньше — и это была вторая по
/// стоимости вещь на сервере. Десять раз в секунду неотличимо на глаз: жертва за
/// сотую долю секунды успевает сместиться на несколько сантиметров.
pub const PERCEPTION_HZ: f32 = 10.0;

/// Тот же интервал в секундах — для разноса фаз при рождении бота.
pub const PERCEPTION_PERIOD: f32 = 1.0 / PERCEPTION_HZ;

/// Как часто бот думает о том, чтобы потратить очки.
const MUTATION_CHECK_INTERVAL: f32 = 0.5;

/// Как часто сервер проверяет, хватает ли в мире диких.
const WILD_CHECK_INTERVAL: f32 = 1.0;

/// Keeps the arena populated with wild organisms.
pub fn maintain_wild(
    mut commands: Commands,
    config: Res<ServerConfig>,
    time: Res<Time>,
    wild: Query<&Brain>,
    census: Query<&PlayerGenome>,
    mut since: Local<f32>,
) {
    // Два прохода по всем организмам ради решения, которое меняется раз в
    // десятки секунд, — раз в секунду этого более чем достаточно.
    *since += time.delta_secs();
    if *since < WILD_CHECK_INTERVAL {
        return;
    }
    *since = 0.0;

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

/// Bots grow on the same economy as the player: they earn points by eating,
/// killing and surviving seasons, and pay the same rising price for each organ.
///
/// Earlier they mutated for free on a timer, which meant a bot that never ate
/// still ended up covered in organs while a player had to work for every one.
pub fn bot_mutation(
    config: Res<ServerConfig>,
    time: Res<Time>,
    mut bots: Query<(&mut OrganismState, &PlayerProgress, &mut BotState), With<Brain>>,
    mut since: Local<f32>,
) {
    // Между решениями бота о росте проходят секунды: накапливаем время и
    // раздаём его пачкой, чтобы таймеры шли ровно так же, как шли.
    *since += time.delta_secs();
    if *since < MUTATION_CHECK_INTERVAL {
        return;
    }
    let dt = std::mem::take(&mut *since);

    let mut rng = rand::rng();
    for (mut organism, progress, mut bot) in &mut bots {
        bot.mutate_in -= dt;
        if bot.mutate_in > 0.0 || progress.dead {
            continue;
        }
        // Bots think about growing on their own rhythm; the decision itself is
        // still paid for out of the same points a player would spend.
        bot.mutate_in = config.wild_mutation_interval * rng.random_range(0.6..1.6);

        let limit = config.wild_max_parts.min(config.max_parts);
        if organism.genome.parts.len() >= limit {
            continue;
        }

        // A little instinct: a mouth first, otherwise it never learns to feed.
        let wanted = if organism.genome.count_family(PartFamily::Mouth) < 2
            && organism.mutation_error(PartKind::basic(PartFamily::Mouth)).is_none()
        {
            Some(PartKind::basic(PartFamily::Mouth))
        } else {
            // Everything it can currently afford, then one at random: bots
            // explore the tree instead of following one optimal build.
            let affordable: Vec<PartKind> = PartKind::all()
                .filter(|kind| organism.mutation_error(*kind).is_none())
                .collect();
            (!affordable.is_empty())
                .then(|| affordable[rng.random_range(0..affordable.len())])
        };

        if let Some(kind) = wanted {
            organism.apply_mutation(kind);
        }
    }
}

/// Что бот решил в последний раз, когда смотрел вокруг.
///
/// Живёт в [`BotState`] между тиками: восприятие обновляет это поле десять раз
/// в секунду, рулевое управление читает его каждый тик.
#[derive(Clone, Copy, Default)]
pub struct Perception {
    /// Куда плыть: еда или жертва.
    pub goal: Option<Vec3>,
    /// Суммарное направление «прочь» от всех угроз, ноль — если бояться некого.
    pub escape: Vec3,
    /// Куда сместиться из отравы. Отдельно от `escape` намеренно: от хищника
    /// бегут, бросив всё, а из грязи выползают по пути, продолжая жить.
    ///
    /// Когда яд считался паникой, боты переставали есть и вымирали от голода
    /// посреди еды — при том, что уплыть было всего на клетку в сторону.
    pub avoid: Vec3,
}

/// Как далеко бот щупает воду, выбирая, куда отплыть из отравы.
///
/// Порядка клетки загрязнения: щупать ближе бессмысленно (попадёшь в ту же
/// клетку), дальше — начнёшь «видеть» грязь за пределами того, куда успеешь
/// доплыть до следующего раздумья.
const POISON_PROBE: f32 = POLLUTION_CELL;

/// Оценка обстановки: кого бояться, кого есть, куда плыть.
///
/// Работает на [`PERCEPTION_HZ`], а не на частоте тика. Фазы разнесены по ботам
/// (см. `BotState::think_in` при рождении), иначе все семьдесят особей думают в
/// один и тот же тик и вместо ровной нагрузки получается пила.
pub fn bot_perception(
    config: Res<ServerConfig>,
    time: Res<Time>,
    food: Res<FoodGrid>,
    field: Res<PollutionField>,
    clouds: Query<&ToxinCloud>,
    beasts: Query<&Leviathan>,
    thorns: Query<&Thorn>,
    // One set: the snapshot of everyone, then the bots we actually steer. Both
    // touch PlayerPosition, so they cannot be two independent queries.
    mut sets: ParamSet<(
        Query<(Entity, &PlayerPosition, &OrganismState)>,
        Query<(Entity, &Brain, &PlayerPosition, &OrganismState, &PlayerProgress, &mut BotState)>,
    )>,
) {
    let dt = time.delta_secs();

    // Снимок несёт то, что видно со стороны: как сильно бьёт, как держит удар,
    // сколько в нём жизни. Гистограмма семейств вместо клона генома — родство
    // считается по ней.
    let world: Vec<Neighbour> = sets
        .p0()
        .iter()
        .map(|(entity, position, state)| Neighbour {
            entity,
            position: position.0,
            attack: attack_power_with(state, config.base_attack),
            defense: defense(state),
            health: state.health,
            families: state.families,
            lineage: state.genome.lineage,
        })
        .collect();

    let vision_squared = config.bot_vision * config.bot_vision;
    let clouds: Vec<ToxinCloud> = clouds.iter().copied().collect();

    // Отрава в точке: чужие облака плюс грязь. Ровно то же, что считает
    // `survival` — бот обязан бояться того же, от чего умирает.
    let poison_at = |point: Vec3| {
        clouds.iter().map(|c| c.toxin_at(point)).sum::<f32>()
            + field.at(point) * POLLUTION_MAX_TOXIN
    };

    for (entity, brain, position, organism, progress, mut bot) in &mut sets.p1() {
        bot.think_in -= dt;
        if bot.think_in > 0.0 {
            continue;
        }
        // Следующий раз — через период, но без накопленного долга: мёртвый бот
        // не думает, и без `max` он бы отработал все пропущенные раздумья разом
        // в тик после возвращения.
        bot.think_in = (bot.think_in + PERCEPTION_PERIOD).max(0.0);
        if progress.dead {
            bot.perception = Perception::default();
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

        for other in &world {
            if other.entity == entity {
                continue;
            }
            // Дешёвая проверка первой: квадрат расстояния вместо корня, и только
            // для тех, кто действительно в поле зрения, — сравнение родства.
            let distance_squared = here.distance_squared(other.position);
            if distance_squared > vision_squared
                || !hostile_counts(
                    &organism.families,
                    organism.genome.lineage,
                    &other.families,
                    other.lineage,
                    config.aggression_threshold,
                    config.kin_split_threshold,
                )
            {
                continue;
            }
            let distance = distance_squared.sqrt();

            // How long each of us would survive the other. This is the whole
            // judgement: not "who is bigger" but "who runs out of health first".
            let incoming = (other.attack * (1.0 - my_defense)).max(0.01);
            let outgoing = (my_attack * (1.0 - other.defense)).max(0.01);
            let i_last = organism.health / incoming;
            let they_last = other.health / outgoing;

            let losing = i_last < they_last * 1.25;
            if losing || wounded {
                // Closer threats pull harder, so a bot flees the nearest first.
                escape += (here - other.position).normalize_or_zero() / distance.max(1.0);
            } else if *brain == Brain::Wild && distance < best_prey && they_last < i_last * 0.7 {
                best_prey = distance;
                goal = Some(other.position);
            }
        }

        // Отрава — это градиент, вдоль которого сползают, а не повод бросить всё.
        //
        // Без этого урон от яда был лотереей: организмы стояли в чужом облаке,
        // пока не умирали, потому что в их картине мира яда не существовало.
        let mut avoid = Vec3::ZERO;
        let poison_here = poison_at(here);
        if poison_here > organism.toxin_resistance {
            // Щупаем четыре стороны и уходим в самую чистую. Четырёх хватает:
            // раздумье повторяется десять раз в секунду, и бот доворачивает на
            // ходу, а не выбирает идеальный маршрут один раз.
            let mut best = poison_here;
            let mut away = Vec3::ZERO;
            for direction in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
                let there = poison_at(here + direction * POISON_PROBE);
                if there < best {
                    best = there;
                    away = direction;
                }
            }
            // Со всех сторон одинаково плохо — плывём хоть куда-нибудь, но
            // прочь: стоять на месте в отраве худший из вариантов.
            let away = away.normalize_or(bot.wander);
            // Чем гуще отрава, тем сильнее она перетягивает курс. Но именно
            // перетягивает: еду бот при этом искать не перестаёт.
            let urgency = (poison_here / organism.toxin_resistance.max(0.01)).min(4.0);
            avoid = away * urgency;
        }

        if goal.is_none() && hungry && escape == Vec3::ZERO {
            // Сетка вместо перебора всех девятисот частиц: поиск идёт кольцами
            // от клетки бота и обрывается, как только ближе уже быть не может.
            goal = food.nearest(here, config.bot_vision);
        }

        // Левиафан — не противник, а погода: от него не отбиваются, от него
        // уходят. Причём заранее и в сторону, а не по курсу: убегать по прямой
        // от того, кто быстрее тебя, бессмысленно.
        for beast in &beasts {
            let offset = here - beast.position;
            let distance = offset.length();
            if distance > beast.radius + config.bot_vision {
                continue;
            }
            // Вбок от его курса, а не прочь от туши: так уходят с дороги.
            let sideways = beast.heading.cross(Vec3::Y).normalize_or(Vec3::X);
            let side = if sideways.dot(offset) < 0.0 { -1.0 } else { 1.0 };
            let urgency = (beast.radius * 2.5 / distance.max(1.0)).min(6.0);
            escape += sideways * side * urgency;
        }

        // Куст: укрытие для мелкого, стена для крупного. Крупный обходит его
        // так же, как обходил бы отраву.
        if Thorn::hurts(body_radius(organism.mass)) {
            for thorn in &thorns {
                let offset = here - thorn.position;
                let distance = offset.length();
                if distance > thorn.radius + body_radius(organism.mass) + 2.0 {
                    continue;
                }
                avoid += offset.normalize_or(Vec3::X) * 2.0;
            }
        }

        bot.perception = Perception { goal, escape, avoid };
    }
}

/// Всё, что бот может понять о соседе, просто посмотрев на него.
struct Neighbour {
    entity: Entity,
    position: Vec3,
    attack: f32,
    defense: f32,
    health: f32,
    families: FamilyCounts,
    lineage: u64,
}

/// Рулевое управление: превращает решение восприятия в шаг тела.
///
/// Работает каждый тик, потому что это и есть движение. Плетение, рывки в
/// сторону и снос к центру арены считаются здесь, а не в восприятии, — иначе
/// побег стал бы дёрганым в такт частоте раздумий.
pub fn bot_movement(
    time: Res<Time>,
    mut bots: Query<(&mut PlayerPosition, &OrganismState, &PlayerProgress, &mut BotState)>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::rng();

    for (mut position, organism, progress, mut bot) in &mut bots {
        if progress.dead {
            continue;
        }
        let here = position.0;
        let Perception { goal, escape, avoid } = bot.perception;

        bot.retarget -= dt;
        if bot.retarget <= 0.0 {
            bot.retarget = rng.random_range(1.5..4.0);
            bot.wander = Vec3::new(rng.random_range(-1.0..1.0), 0.0, rng.random_range(-1.0..1.0))
                .normalize_or(Vec3::X);
        }

        let direction = if escape != Vec3::ZERO {
            // Panic is not a straight line. A cell that flees on a fixed bearing
            // is trivially chased down, so the escape vector gets a sideways
            // weave plus occasional hard breaks — enough to be hard to predict.
            bot.panic_break -= dt;
            if bot.panic_break <= 0.0 {
                bot.panic_break = rng.random_range(0.35..1.1);
                bot.panic_side = if rng.random::<bool>() { 1.0 } else { -1.0 };
                bot.panic_rate = rng.random_range(2.5..6.5);
            }
            bot.panic_phase += dt * bot.panic_rate;

            let away = escape.normalize_or(bot.wander);
            let sideways = Vec3::new(-away.z, 0.0, away.x);
            let weave = bot.panic_phase.sin() * 0.75 * bot.panic_side;
            // Running away also means not running into a wall.
            let inward = -here * 0.05;
            (away + sideways * weave + inward).normalize_or(away)
        } else if let Some(target) = goal {
            // Плывём к цели, но отрава по пути сдвигает курс: можно есть и
            // одновременно выбираться из грязного места.
            ((target - here).normalize_or(bot.wander) + avoid).normalize_or(bot.wander)
        } else {
            bot.panic_break = 0.0;
            // Drift back toward the middle rather than hugging the wall.
            (bot.wander - here * 0.02 + avoid).normalize_or(bot.wander)
        };

        // Fear is fast: a fleeing cell spends everything it has on getting away.
        let hurried = escape != Vec3::ZERO || avoid != Vec3::ZERO;
        let speed = movement_speed(organism) * if hurried { 1.15 } else { 1.0 };
        step_movement_vec(&mut position.0, direction, speed, dt);
    }
}
