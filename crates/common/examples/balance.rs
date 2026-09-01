//! Считает баланс выживания по тем же функциям, которыми его считает сервер.
//!
//! Отвечает на вопросы вида «а яд вообще ощутим?» числами, а не на глаз.
//! Ничего не меняет — только печатает.
//!
//! ```bash
//! cargo run --release -p cellborn-common --example balance
//! ```

use bevy::prelude::*;
use cellborn_common::*;

fn env_for(season: Season) -> Environment {
    let mut env = Environment::default();
    env.season = season;
    env.advance(0.0);
    env
}

/// Организм со стартовым телом плюс перечисленные органы.
fn body(extra: &[PartKind]) -> OrganismState {
    let mut genome = Genome::starter_of(1);
    for kind in extra {
        genome.push_part(*kind);
    }
    OrganismState::from_genome(genome)
}

/// Облако от одной железы, в самом своём ядовитом виде: только что выпущено.
fn fresh_cloud() -> ToxinCloud {
    let emitter = body(&[PartKind::basic(PartFamily::ToxinGland)]);
    ToxinCloud {
        position: Vec3::ZERO,
        radius: TOXIN_RADIUS,
        strength: toxin_emission(&emitter) * 4.0,
    }
}

/// Сколько секунд организм проживёт на полном запасе, если не будет есть.
fn starve_seconds(organism: &OrganismState, env: &Environment) -> f32 {
    let drain = energy_drain(organism, env) - photosynthesis_gain(organism, env);
    if drain <= 0.0 {
        return f32::INFINITY;
    }
    organism.energy_cap() / drain
}

fn main() {
    let plain = body(&[]);
    let cloud = fresh_cloud();
    let at_centre = cloud.toxin_at(Vec3::ZERO);

    println!("\n=== ЯД ===\n");
    println!("Одна железа даёт облако силой {:.3} в центре, радиус {TOXIN_RADIUS}.", at_centre);
    println!("Базовая стойкость тела: {BASE_TOXIN_RESISTANCE:.2}.");
    println!("Железа добавляет своему носителю стойкости: {:.2}.\n", {
        let g = body(&[PartKind::basic(PartFamily::ToxinGland)]);
        g.toxin_resistance - BASE_TOXIN_RESISTANCE
    });

    println!(
        "{:<8} {:>9} {:>9} {:>9} {:>9} {:>9}",
        "сезон", "фон", "расход", "+1 обл.", "+3 обл.", "штраф%"
    );
    for season in [Season::Bloom, Season::Hot, Season::Storm, Season::Cold] {
        let env = env_for(season);
        let clean = energy_drain(&plain, &env);

        let mut one = env.clone();
        one.toxin_level += at_centre;
        let mut three = env.clone();
        three.toxin_level += at_centre * 3.0;

        // Какую долю потолка штрафа (3.0) выедает яд трёх облаков.
        let share = adaptation_penalty(&plain, &three) / 3.0 * 100.0;

        println!(
            "{:<8} {:>9.3} {:>9.3} {:>9.3} {:>9.3} {:>8.0}%",
            season.name(),
            env.toxin_level,
            clean,
            energy_drain(&plain, &one),
            energy_drain(&plain, &three),
            share
        );
    }

    println!("\nСколько живёт голодающий (секунд, полный запас энергии):\n");
    println!("{:<8} {:>10} {:>10} {:>10}", "сезон", "чисто", "+1 обл.", "+3 обл.");
    for season in [Season::Bloom, Season::Hot, Season::Storm, Season::Cold] {
        let env = env_for(season);
        let mut one = env.clone();
        one.toxin_level += at_centre;
        let mut three = env.clone();
        three.toxin_level += at_centre * 3.0;
        println!(
            "{:<8} {:>10.0} {:>10.0} {:>10.0}",
            season.name(),
            starve_seconds(&plain, &env),
            starve_seconds(&plain, &one),
            starve_seconds(&plain, &three)
        );
    }

    println!("\n=== ЧЕМ ЯД НЕ ЯВЛЯЕТСЯ ===\n");
    println!("Яд не наносит урона здоровью. Совсем. Единственный его эффект —");
    println!("расход энергии; здоровье падает только когда энергия дошла до нуля.");
    println!("То есть сытый организм в облаке просто ест чуть чаще.\n");

    let env = env_for(Season::Storm);
    let mut poisoned = env.clone();
    poisoned.toxin_level += at_centre * 3.0;
    let extra = energy_drain(&plain, &poisoned) - energy_drain(&plain, &env);
    let plankton = FoodKind::Plankton.energy();
    println!(
        "Три облака в шторм стоят {extra:.2} энергии/с — это {:.1} планктона в минуту.",
        extra * 60.0 / plankton
    );
    println!("Одна частица планктона даёт {plankton:.0}.");

    println!("\n=== ЗАГРЯЗНЕНИЕ ОТ СКОПЛЕНИЯ ===\n");
    // Клетка теряет грязь двумя способами сразу: очищением и растеканием по
    // соседям. Равновесный уровень — это приток, делённый на сумму того и
    // другого, и считать его надо так, а не подбирать коэффициент на глаз.
    let loss = POLLUTION_DECAY + POLLUTION_SPREAD;
    let upkeep = metabolic_cost(&plain);
    println!("Содержание голого тела: {upkeep:.2} энергии/с.");
    println!("Клетка теряет {:.2} своего уровня в секунду (очищение + растекание).\n", loss);

    println!("{:<10} {:>10} {:>10} {:>10} {:>12}", "тел в клетке", "уровень", "яд", "сверх стойк.", "урон/с");
    for count in [1, 3, 10, 20, 40] {
        let level = (upkeep * POLLUTION_PER_UPKEEP * count as f32 / loss).min(1.0);
        let toxin = level * POLLUTION_MAX_TOXIN;
        let excess = (toxin - BASE_TOXIN_RESISTANCE).max(0.0);
        println!(
            "{count:<10} {:>10.2} {:>10.3} {:>12.3} {:>12.2}",
            level, toxin, excess, excess * TOXIN_DAMAGE
        );
    }
    println!("\nОриентир: одно тело — незаметно, десяток — неприятно,");
    println!("толпа — смертельно. Здоровья {MAX_HEALTH:.0}, регенерация {HEALTH_REGEN} в секунду.");

    println!("\n=== ТО ЖЕ, НО ПРОГОНОМ НАСТОЯЩЕГО ПОЛЯ ===\n");
    // Формула выше — оценка. Здесь крутится тот самый `PollutionField`,
    // которым пользуется сервер: если он разойдётся с оценкой, врёт одно из
    // двух, и лучше узнать это здесь, а не по трупам в логе сервера.
    println!("{:<12} {:>10} {:>10} {:>10}", "тел рядом", "пик", "сумма", "яд в пике");
    for count in [1, 5, 10, 20, 40] {
        let mut field = PollutionField::default();
        let dt = 1.0 / 64.0;
        // Тела стоят кучкой в одной клетке — худший случай, ради которого всё
        // и затевалось.
        for _ in 0..(64 * 120) {
            for _ in 0..count {
                field.add(Vec3::ZERO, upkeep * POLLUTION_PER_UPKEEP * dt);
            }
            field.settle(dt);
        }
        let total: f32 = field.levels.iter().sum();
        println!(
            "{count:<12} {:>10.3} {:>10.2} {:>10.3}",
            field.worst(),
            total,
            field.worst() * POLLUTION_MAX_TOXIN
        );
    }

    println!("\n=== БОЙ ДЛЯ СРАВНЕНИЯ ===\n");
    let spiky = body(&[PartKind::basic(PartFamily::Spike)]);
    println!(
        "Голая мембрана бьёт {:.1} здоровья/с, с одним шипом — {:.1}.",
        attack_power(&plain),
        attack_power(&spiky)
    );
    println!("Здоровья у тела {MAX_HEALTH:.0}, то есть шип убивает за {:.0} секунд контакта.",
        MAX_HEALTH / attack_power(&spiky));
    println!("Голод при нулевой энергии убивает за {:.0} секунд.\n", MAX_HEALTH / STARVATION_DAMAGE);
}
