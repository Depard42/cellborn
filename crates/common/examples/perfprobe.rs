//! Замер горячих циклов серверной симуляции — без Bevy, без сети, без сборки
//! сервера: чистые функции из `common`, вызванные столько же раз, сколько их
//! вызывает один тик.
//!
//! ```bash
//! cargo run --release -p cellborn-common --example perfprobe -- <особей> <частей> <еды>
//! ```
//!
//! Аргументы по умолчанию — 70 20 900: предел популяции, тело среднего возраста
//! и `FOOD_TARGET`. Колонка «% бюджета» считается от 15.6 мс — это длина тика
//! при `FIXED_TIMESTEP_HZ = 64`, то есть всё, что есть у сервера на один шаг.
//!
//! Замеряет и текущий вариант, и предлагаемую замену рядом, чтобы выигрыш был
//! виден без сравнения с числом из головы. См. `docs/PERFORMANCE.md`.
use cellborn_common::*;
use bevy::prelude::*;
use std::time::Instant;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 { self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); self.0 >> 11 }
    fn f32(&mut self) -> f32 { (self.next() % 100000) as f32 / 100000.0 }
    fn range(&mut self, a: f32, b: f32) -> f32 { a + self.f32() * (b - a) }
}

fn make(n: usize, parts: usize, rng: &mut Lcg) -> Vec<(Vec3, OrganismState)> {
    (0..n).map(|i| {
        let mut g = Genome::starter_of((i as u64) % 6);
        for _ in 3..parts { g.push_part(random_part(rng.next())); }
        let pos = Vec3::new(rng.range(-70.0, 70.0), 0.0, rng.range(-70.0, 70.0));
        (pos, OrganismState::from_genome(g))
    }).collect()
}

/// Родство так, как оно считалось до оптимизации: двадцать семейств, и на
/// каждое — по проходу по частям обоих тел.
///
/// Живёт здесь копией нарочно. Иначе строку «было» невозможно перемерить, не
/// откатывая правку: `genetic_distance` теперь считает гистограмму одним
/// проходом, и старое число из `docs/PERFORMANCE.md` не воспроизводилось бы.
fn old_hostile_with(a: &Genome, b: &Genome, strangers: u32, kin: u32) -> bool {
    let distance: u32 = PartFamily::ALL
        .iter()
        .map(|family| {
            (a.count_family(*family) as i32 - b.count_family(*family) as i32).unsigned_abs()
        })
        .sum();
    if a.lineage == b.lineage { distance > kin } else { distance > strangers }
}

fn bench(name: &str, iters: u32, mut f: impl FnMut() -> f32) {
    // прогрев
    let mut sink = 0.0;
    for _ in 0..(iters / 10).max(1) { sink += f(); }
    let t = Instant::now();
    for _ in 0..iters { sink += f(); }
    let per = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
    println!("{name:<52} {per:>9.3} мс/тик   ({:>6.1}% бюджета 15.6мс)  [{sink:.0}]", per / 15.625 * 100.0);
}

fn main() {
    let mut rng = Lcg(12345);
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(70);
    let parts: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(20);
    let food_n: usize = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(900);
    println!("\n=== организмов {n}, частей у каждого {parts}, еды {food_n} ===\n");

    let world = make(n, parts, &mut rng);
    let food: Vec<Vec3> = (0..food_n).map(|_| Vec3::new(rng.range(-70.0,70.0), 0.0, rng.range(-70.0,70.0))).collect();

    // --- 1. genetic_distance O(n^2), как в combat + bot_movement
    bench("combat: родство по всем парам, до расстояния (было)", 200, || {
        let mut acc = 0.0;
        for i in 0..world.len() { for j in (i+1)..world.len() {
            if old_hostile_with(&world[i].1.genome, &world[j].1.genome, 7, 15) { acc += 1.0; }
        }}
        acc
    });

    // Гистограмма семейств, которую тело держит посчитанной (`OrganismState`).
    bench("combat: то же по кэшу гистограммы семейств", 200, || {
        let mut acc = 0.0;
        for i in 0..world.len() { for j in (i+1)..world.len() {
            let (a, b) = (&world[i].1, &world[j].1);
            if hostile_counts(&a.families, a.genome.lineage, &b.families, b.genome.lineage, 7, 15) {
                acc += 1.0;
            }
        }}
        acc
    });


    // --- 1b. так это устроено сейчас: сперва расстояние, потом родство по кэшу
    bench("combat: расстояние, затем родство по кэшу (сейчас)", 200, || {
        let mut acc = 0.0;
        for i in 0..world.len() { for j in (i+1)..world.len() {
            let (a, b) = (&world[i], &world[j]);
            let reach = body_radius(a.1.mass) + body_radius(b.1.mass) + 0.25;
            if a.0.distance_squared(b.0) > reach * reach { continue; }
            if hostile_counts(&a.1.families, a.1.genome.lineage, &b.1.families, b.1.genome.lineage, 7, 15) {
                acc += 1.0;
            }
        }}
        acc
    });

    // --- 1c. восприятие ботов: родство только для тех, кто в поле зрения
    bench("боты: родство по геномам в радиусе обзора (было)", 200, || {
        let mut acc = 0.0;
        for (here, me) in &world {
            for (there, them) in &world {
                if here.distance_squared(*there) > 400.0 { continue; }
                if old_hostile_with(&me.genome, &them.genome, 7, 15) { acc += 1.0; }
            }
        }
        acc
    });
    bench("боты: родство по кэшу в радиусе обзора (сейчас)", 200, || {
        let mut acc = 0.0;
        for (here, me) in &world {
            for (there, them) in &world {
                if here.distance_squared(*there) > 400.0 { continue; }
                if hostile_counts(&me.families, me.genome.lineage, &them.families, them.genome.lineage, 7, 15) {
                    acc += 1.0;
                }
            }
        }
        acc
    });
    // --- 2. снапшот боя: с клоном генома и с гистограммой
    bench("снапшот с clone() генома (было, х2 системы за тик)", 500, || {
        let snap: Vec<(Vec3, f32, f32, Genome)> = world.iter()
            .map(|(p, s)| (*p, attack_power_with(s, 1.5), defense(s), s.genome.clone())).collect();
        snap.len() as f32
    });
    bench("снапшот с гистограммой вместо генома (сейчас)", 500, || {
        let snap: Vec<(Vec3, f32, f32, FamilyCounts, u64)> = world.iter()
            .map(|(p, s)| (*p, attack_power_with(s, 1.5), defense(s), s.families, s.genome.lineage))
            .collect();
        snap.len() as f32
    });

    // --- 3. поиск ближайшей еды линейным перебором (bot_movement)
    bench("боты: ближайшая еда линейным перебором (было)", 200, || {
        let mut acc = 0.0;
        for (here, _) in &world {
            let mut nearest = 20.0f32;
            for f in &food { let d = here.distance(*f); if d < nearest { nearest = d; acc += 0.001; } }
        }
        acc
    });

    // --- 4. пересборка сетки еды каждый тик (feeding)
    bench("feeding: пересборка HashMap-сетки еды (было)", 200, || {
        use bevy::platform::collections::HashMap;
        const CELL: f32 = 4.0;
        let mut grid: HashMap<(i32,i32), Vec<(u32, Vec3, f32)>> = HashMap::default();
        for (i, f) in food.iter().enumerate() {
            grid.entry(((f.x/CELL).floor() as i32, (f.z/CELL).floor() as i32)).or_default().push((i as u32, *f, 9.0));
        }
        grid.len() as f32
    });

    // --- 5. производные от генома, пересчитываемые каждый тик
    bench("производные (speed+attack+defense+upkeep+cap+reach)", 2000, || {
        let mut acc = 0.0;
        for (_, s) in &world {
            acc += movement_speed(s) + attack_power_with(s, 1.5) + defense(s)
                 + metabolic_cost(s) + s.energy_cap() + feeding_reach(s).unwrap_or(0.0);
        }
        acc
    });

    // --- 6. сравнение геномов в project_state
    bench("project_state: PartialEq геномов", 2000, || {
        let mut acc = 0.0;
        for (i, (_, s)) in world.iter().enumerate() {
            if s.genome != world[(i+1) % world.len()].1.genome { acc += 1.0; }
        }
        acc
    });

    // --- 7. отдельно: сама stats()
    bench("stats() 200 раз (таблица частей)", 20000, || {
        let mut acc = 0.0;
        for k in PartKind::all() { acc += stats(k).mass; }
        acc
    });
}
