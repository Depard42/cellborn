//! Опасности и приманки: колючки, левиафаны, лакомые места.
//!
//! Всё три системы существуют ради одного — чтобы у моря появился рельеф. До
//! них арена была однородной: еда сыпалась ровным слоем, опасность исходила
//! только от соседей, и в любой точке карты происходило одно и то же. Плыть
//! было незачем, и все стояли в середине.
//!
//! Теперь у каждого места есть свойство. Колючка — укрытие для мелких и стена
//! для крупных. Лакомое место — повод рискнуть. Левиафан — повод бросить и
//! укрытие, и лакомое место.

use bevy::prelude::*;
use cellborn_common::*;
use lightyear::prelude::*;
use rand::Rng;

use crate::config::ServerConfig;
use crate::life::{spawn_organism, Brain};

/// Расставляет колючки один раз при старте мира.
///
/// Неподвижные и вечные: укрытие бесполезно, если его нельзя запомнить.
pub fn place_thorns(commands: &mut Commands, count: usize) {
    let mut rng = rand::rng();
    let mut placed: Vec<Vec3> = Vec::new();
    // Не ближе трёх радиусов друг к другу: слипшиеся колючки перестают быть
    // укрытием и превращаются в стену.
    let spacing = THORN_RADIUS * 3.0;

    for _ in 0..count {
        // Несколько попыток найти свободное место; не нашли — пропускаем, а не
        // ставим внахлёст.
        for _ in 0..24 {
            let edge = ARENA_HALF_EXTENT * 0.88;
            let candidate =
                Vec3::new(rng.random_range(-edge..edge), 0.0, rng.random_range(-edge..edge));
            if placed.iter().any(|p| p.distance(candidate) < spacing) {
                continue;
            }
            placed.push(candidate);
            commands.spawn((
                Thorn { position: candidate, radius: THORN_RADIUS },
                Replicate::to_clients(NetworkTarget::All),
            ));
            break;
        }
    }
    info!("расставлено колючек: {}", placed.len());
}

/// Колючки колют тех, кто слишком велик, чтобы пройти насквозь.
pub fn thorn_damage(
    config: Res<ServerConfig>,
    time: Res<Time>,
    thorns: Query<&Thorn>,
    mut organisms: Query<(&PlayerPosition, &mut OrganismState, &PlayerProgress)>,
) {
    if thorns.is_empty() {
        return;
    }
    let dt = time.delta_secs();
    let thorns: Vec<Thorn> = thorns.iter().copied().collect();

    for (position, mut organism, progress) in &mut organisms {
        if progress.dead {
            continue;
        }
        let radius = body_radius(organism.mass);
        if !Thorn::hurts_with(radius, squeeze(&organism)) {
            // Мелкий проходит насквозь — и крупный с присосками тоже: в этом
            // весь смысл колючки как укрытия, а не как стены.
            continue;
        }
        if thorns.iter().any(|thorn| thorn.touches(position.0, radius)) {
            organism.health = (organism.health - config.thorn_damage * dt).max(0.0);
            // Колючка считается боем: пока сидишь на ней, раны не заживают.
            organism.combat_timer = config.combat_regen_block;
        }
    }
}

/// Раз в несколько минут через море проплывает гигант.
///
/// Это **обычный организм**, просто огромный и вкачанный: те же формулы, тот же
/// бой, та же смерть. Раньше он был отдельной сущностью с собственным уроном —
/// с ним нельзя было драться, и он оказывался не участником игры, а движущейся
/// стеной. Стая, которая может его завалить, — это событие; стена событием быть
/// не может.
pub fn summon_leviathan(
    mut commands: Commands,
    config: Res<ServerConfig>,
    time: Res<Time>,
    existing: Query<(), With<Leviathan>>,
    mut next: Local<f32>,
) {
    // Первый заплыв не сразу после запуска: пусть мир сначала обживётся.
    if *next <= 0.0 {
        *next = config.leviathan_interval;
    }
    *next -= time.delta_secs();
    if *next > 0.0 || config.leviathan_interval <= 0.0 {
        return;
    }
    let mut rng = rand::rng();
    *next = config.leviathan_interval * rng.random_range(0.6..1.5);

    // Одного за раз достаточно: двое превращают событие в погоду.
    if !existing.is_empty() {
        return;
    }

    // Тело гиганта собирается из тех же органов, что и любое другое, — просто
    // их много и они совершенные. Отсюда и его сила: она не выдумана, а
    // посчитана теми же формулами, что и у игрока.
    let mut genome = Genome::starter_of(rng.random::<u64>());
    for (family, count) in [
        (PartFamily::Membrane, 5),
        (PartFamily::Carapace, 4),
        (PartFamily::Spike, 4),
        (PartFamily::Nematocyst, 3),
        (PartFamily::Mouth, 3),
        (PartFamily::Flagellum, 4),
        (PartFamily::Gill, 2),
        (PartFamily::ThermalMembrane, 2),
        (PartFamily::Osmoregulator, 2),
        (PartFamily::StorageVacuole, 3),
    ] {
        for _ in 0..count {
            genome.push_part(PartKind::new(family, PartLevel::Perfect));
        }
    }
    let state = OrganismState::from_genome(genome);

    // Входит из-за края и уходит за противоположный, слегка наискось.
    let edge = ARENA_HALF_EXTENT * 0.97;
    let along = rng.random_range(-ARENA_HALF_EXTENT..ARENA_HALF_EXTENT);
    let (from, heading) = if rng.random::<bool>() {
        let side = if rng.random::<bool>() { -1.0 } else { 1.0 };
        (Vec3::new(side * edge, 0.0, along), Vec3::new(-side, 0.0, 0.0))
    } else {
        let side = if rng.random::<bool>() { -1.0 } else { 1.0 };
        (Vec3::new(along, 0.0, side * edge), Vec3::new(0.0, 0.0, -side))
    };
    let drift = rng.random_range(-0.35..0.35);
    let heading = (heading + heading.cross(Vec3::Y) * drift).normalize_or(heading);

    let mass = state.mass;
    let entity = spawn_organism(&mut commands, state, from, None, Some(Brain::Wild));
    commands.entity(entity).insert(Leviathan { heading, swim: 0.0, fed: 0.0 });

    info!("через море идёт гигант: масса {mass:.0}");
}

/// Ведёт гиганта через море.
///
/// Он живёт по общим правилам — ест, дерётся, умирает, — но не как местный:
/// он **проходит насквозь**. Своего курса держится упрямо, а уйдя за
/// противоположный край, исчезает совсем.
pub fn leviathan_pass(
    mut commands: Commands,
    config: Res<ServerConfig>,
    time: Res<Time>,
    mut beasts: Query<(Entity, &mut Leviathan, &mut PlayerPosition, &OrganismState)>,
) {
    let dt = time.delta_secs();
    let gone = ARENA_HALF_EXTENT * 1.02;

    for (entity, mut beast, mut position, organism) in &mut beasts {
        beast.swim += dt * 1.8;

        // Курс держится сам: обычное поведение бота увело бы его гоняться за
        // едой, а он проплывает мимо. Это и делает его событием, а не жильцом.
        let step = beast.heading * movement_speed(organism) * config.leviathan_speed * dt;
        position.0 += step;

        // Ушёл за край — исчез. Никаких разворотов.
        let out = position.0.x.abs() >= gone || position.0.z.abs() >= gone;
        if out {
            info!("гигант ушёл в открытое море");
            commands.entity(entity).despawn();
        }
    }
}

/// Держит в море несколько лакомых мест/// Держит в море несколько лакомых мест и обновляет их по мере угасания.
pub fn maintain_feasts(
    mut commands: Commands,
    config: Res<ServerConfig>,
    time: Res<Time>,
    mut feasts: Query<(Entity, &mut Feast)>,
    mut since: Local<f32>,
) {
    // Пятна живут десятками секунд; проверять их чаще, чем пару раз в секунду,
    // незачем.
    *since += time.delta_secs();
    if *since < 0.5 {
        return;
    }
    let dt = std::mem::take(&mut *since);

    let mut alive = 0;
    for (entity, mut feast) in &mut feasts {
        feast.strength -= dt / FEAST_LIFETIME.max(1.0);
        if feast.strength <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        alive += 1;
    }

    let mut rng = rand::rng();
    for _ in alive..config.feast_count {
        // Не у самой стенки: пятно у края арены наполовину пропадает впустую.
        let edge = ARENA_HALF_EXTENT - FEAST_RADIUS * 1.5;
        commands.spawn((
            Feast {
                position: Vec3::new(
                    rng.random_range(-edge..edge),
                    0.0,
                    rng.random_range(-edge..edge),
                ),
                radius: FEAST_RADIUS,
                strength: 1.0,
            },
            Replicate::to_clients(NetworkTarget::All),
        ));
    }
}

/// Куда сыпать следующую частицу еды.
///
/// Большая часть достаётся лакомым местам — ради этого они и существуют. Но не
/// всё: если бы вне пятен еды не было совсем, море за их пределами стало бы
/// мёртвым коридором, по которому только перебегают.
pub fn feeding_spot(feasts: &[Feast], rng: &mut impl Rng) -> Vec3 {
    let pick = (!feasts.is_empty() && rng.random::<f32>() < FEAST_SHARE)
        .then(|| feasts[rng.random_range(0..feasts.len())]);

    match pick {
        Some(feast) => {
            // Гуще к середине пятна: у него должен быть центр, а не ровный диск.
            let angle = rng.random_range(0.0..std::f32::consts::TAU);
            let distance = rng.random::<f32>().powf(0.5) * feast.radius;
            feast.position + Vec3::new(angle.cos() * distance, 0.0, angle.sin() * distance)
        }
        None => Vec3::new(
            rng.random_range(-ARENA_HALF_EXTENT..ARENA_HALF_EXTENT),
            0.0,
            rng.random_range(-ARENA_HALF_EXTENT..ARENA_HALF_EXTENT),
        ),
    }
}
