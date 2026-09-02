//! Вода: какая она и где.
//!
//! Раньше здесь были сезоны — вода менялась во времени и одинаково для всех.
//! Это не работало: сезон нельзя пережить умением, только перетерпеть, а раз
//! он накрывает всю арену разом, то и выбора он не даёт. Приспособленность
//! оказывалась не решением, а налогом, который платят все.
//!
//! Теперь вода меняется **в пространстве**. Море разбито на биомы, у каждого
//! своя температура, солёность, кислород и фон яда. Это делает адаптацию
//! решением: осморегулятор не «полезен вообще», а «открывает Соляную впадину»,
//! и открывает вместе с ней ту еду, которой в спокойной воде не бывает.
//!
//! Правило простое: **чем тяжелее биом, тем богаче в нём еда.** Жить там, где
//! другие не могут, — это и есть награда за вложенные в тело очки.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::ARENA_HALF_EXTENT;

/// Участок моря со своей водой.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Biome {
    /// Открытое море: ничего особенного и ничего опасного. Дом для тех, кто
    /// ещё не выбрал, кем быть.
    Open,
    /// Отмель: тепло, светло, много мелкой еды. Здесь начинают.
    Shallows,
    /// Соляная впадина: солёная до едкости. Нужен осморегулятор.
    Brine,
    /// Термальный разлом: горячо, душно, ядовито. Самая богатая еда в море и
    /// самая высокая плата за вход.
    Vents,
    /// Ледяная бездна: холодно и прозрачно, зато кислорода вдоволь.
    Abyss,
}

impl Biome {
    pub const ALL: [Biome; 5] =
        [Biome::Open, Biome::Shallows, Biome::Brine, Biome::Vents, Biome::Abyss];

    pub fn name(self) -> &'static str {
        match self {
            Biome::Open => "Открытое море",
            Biome::Shallows => "Отмель",
            Biome::Brine => "Соляная впадина",
            Biome::Vents => "Термальный разлом",
            Biome::Abyss => "Ледяная бездна",
        }
    }

    /// Одна строка о том, чем этот участок живёт.
    pub fn hint(self) -> &'static str {
        match self {
            Biome::Open => "спокойно и небогато",
            Biome::Shallows => "тепло, светло, много мелкой еды",
            Biome::Brine => "едкая соль; нужен осморегулятор",
            Biome::Vents => "жар и духота; самая богатая еда в море",
            Biome::Abyss => "холод и темнота, зато дышится легко",
        }
    }

    /// Вода этого биома.
    pub fn water(self) -> Environment {
        // Числа выбраны так, чтобы каждый биом требовал СВОЙ орган, а не
        // «побольше всего». Открытое море внутри допусков голого тела; в любом
        // другом хотя бы одно давление выходит за них заметно.
        match self {
            Biome::Open => Environment {
                temperature: 0.50,
                salinity: 0.50,
                oxygen: 0.82,
                toxin_level: 0.03,
                food_density: 0.75,
            },
            Biome::Shallows => Environment {
                temperature: 0.62,
                salinity: 0.46,
                oxygen: 0.90,
                toxin_level: 0.02,
                food_density: 1.30,
            },
            Biome::Brine => Environment {
                // Соль далеко за терпимостью голого тела (0.16 от середины).
                temperature: 0.55,
                salinity: 0.86,
                oxygen: 0.74,
                toxin_level: 0.05,
                food_density: 1.75,
            },
            Biome::Vents => Environment {
                temperature: 0.92,
                salinity: 0.58,
                // Ниже порога удушья: без жабр здесь теряешь здоровье. Но с
                // запасом, чтобы **одной** жабры хватало полностью: орган
                // обязан решать задачу, а не подводить к ней вплотную.
                oxygen: 0.44,
                toxin_level: 0.14,
                food_density: 2.40,
            },
            Biome::Abyss => Environment {
                temperature: 0.10,
                salinity: 0.54,
                oxygen: 0.95,
                toxin_level: 0.02,
                food_density: 1.05,
            },
        }
    }

    /// Сколько света доходит: фотосинтез работает не везде.
    pub fn light(self) -> f32 {
        match self {
            Biome::Open => 0.70,
            Biome::Shallows => 1.00,
            Biome::Brine => 0.45,
            Biome::Vents => 0.30,
            Biome::Abyss => 0.15,
        }
    }

    /// Цвет воды. Клиент показывает его тем сильнее, чем хуже тело
    /// приспособлено: своя вода выглядит почти нейтральной, чужая — кричащей.
    pub fn tint(self) -> [f32; 3] {
        match self {
            Biome::Open => [0.10, 0.28, 0.36],
            Biome::Shallows => [0.16, 0.42, 0.38],
            Biome::Brine => [0.34, 0.30, 0.14],
            Biome::Vents => [0.40, 0.14, 0.10],
            Biome::Abyss => [0.08, 0.14, 0.34],
        }
    }

    /// Биом в этой точке.
    ///
    /// Ближайший из опорных точек — простейшая мозаика Вороного. Она
    /// детерминирована и одинакова у сервера и у всех клиентов, поэтому карту
    /// биомов не нужно ни реплицировать, ни согласовывать: обе стороны считают
    /// её одной и той же функцией.
    pub fn at(point: Vec3) -> Biome {
        let mut best = (f32::MAX, Biome::Open);
        for (x, z, biome) in SITES {
            // Опорные точки заданы в долях полуразмера арены — их надо
            // развернуть в мировые координаты. Без этого вся карта схлопывалась
            // в окрестность нуля, и любая точка моря оказывалась «открытой
            // водой»: остальные биомы существовали только в коде.
            let offset = Vec3::new(
                point.x - x * ARENA_HALF_EXTENT,
                0.0,
                point.z - z * ARENA_HALF_EXTENT,
            );
            let distance = offset.length_squared();
            if distance < best.0 {
                best = (distance, biome);
            }
        }
        best.1
    }

    /// Вода в этой точке.
    pub fn water_at(point: Vec3) -> Environment {
        Self::at(point).water()
    }
}

/// Опорные точки биомов в долях полуразмера арены.
///
/// Открытого моря несколько участков, и оно окружает остальные: новичок,
/// появившийся где угодно, чаще всего оказывается в безопасной воде, а тяжёлые
/// биомы приходится искать. Их центры разнесены к краям — за богатой едой надо
/// плыть, а не просто оказаться рядом.
const SITES: [(f32, f32, Biome); 9] = [
    (0.0, 0.0, Biome::Open),
    (0.62, 0.0, Biome::Open),
    (-0.62, 0.0, Biome::Open),
    (0.0, 0.66, Biome::Shallows),
    (0.0, -0.66, Biome::Shallows),
    (0.70, 0.70, Biome::Brine),
    (-0.70, -0.70, Biome::Brine),
    (0.74, -0.74, Biome::Vents),
    (-0.74, 0.74, Biome::Abyss),
];

/// Вода: четыре давления среды и то, сколько в ней еды.
///
/// Больше не меняется во времени — это описание места, а не момента.
#[derive(Resource, Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Environment {
    pub temperature: f32,
    pub salinity: f32,
    pub oxygen: f32,
    pub toxin_level: f32,
    /// Множитель к количеству еды: во сколько раз её здесь больше обычного.
    pub food_density: f32,
}

impl Default for Environment {
    fn default() -> Self {
        Biome::Open.water()
    }
}

/// Опорные точки в мировых координатах — для отрисовки карты биомов.
pub fn biome_sites() -> impl Iterator<Item = (Vec3, Biome)> {
    SITES
        .into_iter()
        .map(|(x, z, biome)| (Vec3::new(x * ARENA_HALF_EXTENT, 0.0, z * ARENA_HALF_EXTENT), biome))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{adaptation_penalty, suffocation, Genome, OrganismState, PartFamily, PartKind};

    /// Каждый биом должен быть где-то на карте, иначе он существует только в
    /// коде: игрок никогда его не встретит.
    #[test]
    fn every_biome_is_somewhere_in_the_sea() {
        let mut seen = std::collections::HashSet::new();
        let step = ARENA_HALF_EXTENT / 12.0;
        let mut x = -ARENA_HALF_EXTENT;
        while x <= ARENA_HALF_EXTENT {
            let mut z = -ARENA_HALF_EXTENT;
            while z <= ARENA_HALF_EXTENT {
                seen.insert(Biome::at(Vec3::new(x, 0.0, z)));
                z += step;
            }
            x += step;
        }
        for biome in Biome::ALL {
            assert!(seen.contains(&biome), "{} нигде не встречается", biome.name());
        }
    }

    /// Тяжёлый биом обязан платить за себя едой, иначе туда незачем плыть.
    #[test]
    fn harsher_water_carries_richer_food() {
        let bare = OrganismState::default();
        let mut ranked: Vec<(f32, f32, &str)> = Biome::ALL
            .into_iter()
            .map(|biome| {
                let water = biome.water();
                (adaptation_penalty(&bare, &water), water.food_density, biome.name())
            })
            .collect();
        ranked.sort_by(|a, b| a.0.total_cmp(&b.0));

        // Самый мягкий биом не должен быть и самым богатым: тогда всё остальное
        // море стало бы бессмысленным.
        let (_, easiest_food, easiest) = ranked[0];
        let (_, hardest_food, hardest) = *ranked.last().unwrap();
        assert!(
            hardest_food > easiest_food,
            "{hardest} тяжелее {easiest}, но еды в нём не больше"
        );
    }

    /// Каждый тяжёлый биом должен требовать СВОЙ орган, а не «побольше всего».
    #[test]
    fn each_hard_biome_asks_for_its_own_organ() {
        let with = |family| {
            let mut genome = Genome::starter_of(1);
            for _ in 0..2 {
                genome.push_part(PartKind::basic(family));
            }
            OrganismState::from_genome(genome)
        };
        let bare = OrganismState::default();

        for (biome, family) in [
            (Biome::Brine, PartFamily::Osmoregulator),
            (Biome::Vents, PartFamily::Gill),
            (Biome::Abyss, PartFamily::ThermalMembrane),
        ] {
            let water = biome.water();
            let adapted = with(family);
            assert!(
                adaptation_penalty(&adapted, &water) < adaptation_penalty(&bare, &water),
                "{}: {} не помогает",
                biome.name(),
                family.name()
            );
        }

        // Разлом душит без жабр — это его подпись, а не общая суровость.
        assert!(suffocation(&bare, &Biome::Vents.water()) > 0.0, "в разломе дышится свободно");
        assert_eq!(
            suffocation(&bare, &Biome::Shallows.water()),
            0.0,
            "на отмели нечем задыхаться"
        );
    }

    /// Открытое море должно быть пригодно для голого тела: с него начинают.
    #[test]
    fn open_water_is_survivable_bare() {
        let bare = OrganismState::default();
        let water = Biome::Open.water();
        assert_eq!(suffocation(&bare, &water), 0.0, "новичок задыхается на старте");
        assert!(
            adaptation_penalty(&bare, &water) < 0.5,
            "открытое море наказывает того, у кого ещё ничего нет"
        );
    }
}
