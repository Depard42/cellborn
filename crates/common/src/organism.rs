use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::balance::*;

/// What an organ *is*. Twenty of these, each with its own role in the body.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PartFamily {
    Membrane,
    Flagellum,
    Cilia,
    Mouth,
    Ram,
    Spike,
    ToxinGland,
    Osmoregulator,
    ThermalMembrane,
    Photosynthesis,
    StorageVacuole,
    MucusCoat,
    Gill,
    Divisome,
    Mutator,
    Pseudopod,
    Nematocyst,
    Filter,
    Chemoreceptor,
    Carapace,
    Holdfast,
    Bladder,
}

/// Насколько хорошо развит орган. Один и тот же орган на четырёх уровнях.
///
/// Раньше здесь было десять вариантов — «крупный», «хрупкий», «двойной» и так
/// далее, — каждый со своим набором компромиссов. Красиво на бумаге и
/// нечитаемо в игре: чтобы выбрать, приходилось сравнивать четыре числа между
/// десятью карточками, и большинство вариантов оказывались просто хуже других.
///
/// Четыре уровня решают это одной строкой: следующий лучше предыдущего во всём,
/// кроме цены. Выбирать нужно не «какой», а «насколько сейчас по карману».
///
/// Уровень можно **поднять у уже отращённого органа**, а можно сразу купить
/// высокий и перескочить ступени — за полную цену.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PartLevel {
    /// Дешёвый: слабее обычного, но по карману с самого начала.
    Cheap,
    /// Обычный: то, чем орган задуман.
    Plain,
    /// Улучшенный: заметно сильнее.
    Fine,
    /// Совершенный: столько, сколько орган способен дать.
    Perfect,
}

/// Орган определённого уровня развития.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PartKind {
    pub family: PartFamily,
    pub level: PartLevel,
}

impl PartFamily {
    pub const ALL: [PartFamily; 22] = [
        PartFamily::Flagellum,
        PartFamily::Cilia,
        PartFamily::Pseudopod,
        PartFamily::Mouth,
        PartFamily::Filter,
        PartFamily::Holdfast,
        PartFamily::Chemoreceptor,
        PartFamily::Gill,
        PartFamily::Osmoregulator,
        PartFamily::ThermalMembrane,
        PartFamily::Photosynthesis,
        PartFamily::StorageVacuole,
        PartFamily::MucusCoat,
        PartFamily::Carapace,
        PartFamily::Spike,
        PartFamily::Nematocyst,
        PartFamily::ToxinGland,
        PartFamily::Divisome,
        PartFamily::Mutator,
        PartFamily::Ram,
        PartFamily::Bladder,
        PartFamily::Membrane,
    ];

    /// Место семейства в [`PartFamily::ALL`], то есть его ячейка в гистограмме
    /// тела. Матч, а не поиск по массиву: гистограмма пересчитывается на каждую
    /// часть каждого тела, и линейный проход по двадцати элементам здесь виден.
    ///
    /// Порядок обязан совпадать с `ALL` — за этим следит тест `slot_matches_all`.
    pub fn slot(self) -> usize {
        match self {
            PartFamily::Flagellum => 0,
            PartFamily::Cilia => 1,
            PartFamily::Pseudopod => 2,
            PartFamily::Mouth => 3,
            PartFamily::Filter => 4,
            PartFamily::Holdfast => 5,
            PartFamily::Chemoreceptor => 6,
            PartFamily::Gill => 7,
            PartFamily::Osmoregulator => 8,
            PartFamily::ThermalMembrane => 9,
            PartFamily::Photosynthesis => 10,
            PartFamily::StorageVacuole => 11,
            PartFamily::MucusCoat => 12,
            PartFamily::Carapace => 13,
            PartFamily::Spike => 14,
            PartFamily::Nematocyst => 15,
            PartFamily::ToxinGland => 16,
            PartFamily::Divisome => 17,
            PartFamily::Mutator => 18,
            PartFamily::Ram => 19,
            PartFamily::Bladder => 20,
            PartFamily::Membrane => 21,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            PartFamily::Membrane => "Мембрана",
            PartFamily::Flagellum => "Жгутик",
            PartFamily::Cilia => "Реснички",
            PartFamily::Mouth => "Рот",
            PartFamily::Ram => "Таран",
            PartFamily::Spike => "Шип",
            PartFamily::ToxinGland => "Токсиновая железа",
            PartFamily::Osmoregulator => "Осморегулятор",
            PartFamily::ThermalMembrane => "Термомембрана",
            PartFamily::Photosynthesis => "Фотосинтез",
            PartFamily::StorageVacuole => "Вакуоль",
            PartFamily::MucusCoat => "Слизь",
            PartFamily::Gill => "Жабра",
            PartFamily::Divisome => "Делитель",
            PartFamily::Mutator => "Мутатор",
            PartFamily::Pseudopod => "Ложноножка",
            PartFamily::Nematocyst => "Стрекало",
            PartFamily::Filter => "Фильтр",
            PartFamily::Chemoreceptor => "Хеморецептор",
            PartFamily::Carapace => "Панцирь",
            PartFamily::Holdfast => "Присоска",
            PartFamily::Bladder => "Пузырь",
        }
    }

    pub fn tradeoff(self) -> &'static str {
        match self {
            PartFamily::Membrane => "+запас энергии, +масса",
            PartFamily::Flagellum => "+скорость, дорогой в содержании",
            PartFamily::Cilia => "+немного скорости, дёшево",
            PartFamily::Mouth => "+радиус захвата пищи",
            PartFamily::Ram => "урон растёт от массы и разгона",
            PartFamily::Spike => "+урон при контакте",
            PartFamily::ToxinGland => "+стойкость к яду, отравляет воду вокруг",
            PartFamily::Osmoregulator => "+солёность",
            PartFamily::ThermalMembrane => "+температура",
            PartFamily::Photosynthesis => "энергия из света",
            PartFamily::StorageVacuole => "+запас энергии",
            PartFamily::MucusCoat => "+защита и яд, −скорость",
            PartFamily::Gill => "+кислород: главное в шторм",
            PartFamily::Divisome => "делится быстрее",
            PartFamily::Mutator => "потомки мутируют чаще",
            PartFamily::Pseudopod => "скорость и немного урона",
            PartFamily::Nematocyst => "много урона, хрупкое",
            PartFamily::Filter => "очищает воду вокруг, стойкость к яду",
            PartFamily::Chemoreceptor => "видит еду издалека, дёшево",
            PartFamily::Carapace => "много защиты, много массы",
            PartFamily::Holdfast => "пролезаешь в кусты, будучи крупным",
            PartFamily::Bladder => "лёгкость: масса меньше давит на скорость",
        }
    }

    /// Whether the organ sits on the membrane surface or floats inside the cell.
    pub fn is_external(self) -> bool {
        matches!(
            self,
            PartFamily::Flagellum
                | PartFamily::Cilia
                | PartFamily::Mouth
                | PartFamily::Ram
                | PartFamily::Spike
                | PartFamily::MucusCoat
                | PartFamily::ThermalMembrane
                | PartFamily::Pseudopod
                | PartFamily::Nematocyst
                | PartFamily::Carapace
                | PartFamily::Holdfast
        )
    }

    /// Base stats before the variant reshapes them.
    pub fn base(self) -> PartStats {
        use PartFamily::*;
        let d = PartStats::NONE;
        match self {
            Membrane => PartStats {
                cost: 1, mass: 2.0, upkeep: 0.02, storage: 10.0,
                temperature: 0.02, salinity: 0.02, defense: 0.05, ..d
            },
            Flagellum => PartStats { cost: 2, mass: 1.0, upkeep: 0.10, speed: 1.8, ..d },
            Cilia => PartStats { cost: 2, mass: 0.5, upkeep: 0.05, speed: 0.8, ..d },
            Mouth => PartStats { cost: 2, mass: 0.8, upkeep: 0.06, reach: 0.55, ..d },
            // Таран: урон от массы, а не вместо неё. Единственный орган,
            // который делает рост оружием — тем, кто вложился в размер.
            Ram => PartStats { cost: 3, mass: 1.6, upkeep: 0.06, ram: 0.9, speed: -0.2, ..d },
            Spike => PartStats { cost: 3, mass: 1.2, upkeep: 0.07, speed: -0.1, attack: 7.0, ..d },
            ToxinGland => PartStats {
                cost: 4, mass: 1.0, upkeep: 0.12, toxin: 0.10, toxin_emission: 0.06,
                attack: 1.5, ..d
            },
            Osmoregulator => PartStats { cost: 3, mass: 1.0, upkeep: 0.10, salinity: 0.15, ..d },
            ThermalMembrane => PartStats {
                cost: 3, mass: 1.5, upkeep: 0.10, temperature: 0.15, ..d
            },
            Photosynthesis => PartStats {
                cost: 4, mass: 1.0, upkeep: 0.06, photosynthesis: 0.55, ..d
            },
            StorageVacuole => PartStats { cost: 2, mass: 1.2, upkeep: 0.04, storage: 25.0, ..d },
            MucusCoat => PartStats {
                cost: 3, mass: 0.8, upkeep: 0.06, speed: -0.6, toxin: 0.05, oxygen: 0.06,
                defense: 0.30, ..d
            },
            Gill => PartStats { cost: 3, mass: 0.6, upkeep: 0.08, oxygen: 0.18, ..d },
            Divisome => PartStats { cost: 4, mass: 0.9, upkeep: 0.14, reproduction: 0.7, ..d },
            Mutator => PartStats { cost: 3, mass: 0.5, upkeep: 0.09, mutagen: 0.25, ..d },
            // A crawling foot: pushes and shoves.
            Pseudopod => PartStats { cost: 3, mass: 1.1, upkeep: 0.09, speed: 1.1, attack: 2.5, ..d },
            // A stinging cell: hits hard, made of glass.
            Nematocyst => PartStats {
                cost: 5, mass: 0.7, upkeep: 0.15, attack: 12.0, defense: -0.05, ..d
            },
            // Фильтр: чистит воду вокруг себя. Ответ на загрязнение от толпы —
            // с ним можно жить там, где стоят все, и не травиться.
            Filter => PartStats {
                cost: 4, mass: 0.9, upkeep: 0.14, toxin: 0.09, cleansing: 0.8, ..d
            },
            Chemoreceptor => PartStats { cost: 1, mass: 0.2, upkeep: 0.02, sense: 4.0, ..d },
            Carapace => PartStats {
                cost: 4, mass: 3.0, upkeep: 0.05, defense: 0.28, speed: -0.4, ..d
            },
            // Присоска: позволяет крупному телу протискиваться в кусты. Ответ
            // на то, что укрытия достаются только мелким.
            Holdfast => PartStats { cost: 3, mass: 0.7, upkeep: 0.07, squeeze: 0.35, ..d },
            // Пузырь: снимает часть тормоза от массы. Ответ на то, что рост
            // всегда стоил скорости, — теперь за неё можно доплатить.
            Bladder => PartStats { cost: 3, mass: 0.5, upkeep: 0.09, buoyancy: 0.30, ..d },
        }
    }
}

/// Multipliers that turn one organ into ten.
pub struct VariantMods {
    pub cost: f32,
    pub mass: f32,
    pub upkeep: f32,
    /// Applied to everything the part actually does.
    pub effect: f32,
}

impl PartLevel {
    pub const ALL: [PartLevel; 4] =
        [PartLevel::Cheap, PartLevel::Plain, PartLevel::Fine, PartLevel::Perfect];

    pub fn name(self) -> &'static str {
        match self {
            PartLevel::Cheap => "дешёвый",
            PartLevel::Plain => "обычный",
            PartLevel::Fine => "улучшенный",
            PartLevel::Perfect => "совершенный",
        }
    }

    /// One line explaining what this level is for.
    pub fn hint(self) -> &'static str {
        match self {
            PartLevel::Cheap => "слабее, зато по карману сразу",
            PartLevel::Plain => "как орган задуман",
            PartLevel::Fine => "заметно сильнее, дороже в содержании",
            PartLevel::Perfect => "всё, что орган способен дать",
        }
    }

    pub fn step(self) -> usize {
        match self {
            PartLevel::Cheap => 0,
            PartLevel::Plain => 1,
            PartLevel::Fine => 2,
            PartLevel::Perfect => 3,
        }
    }

    /// Следующий уровень, если он есть.
    pub fn next(self) -> Option<PartLevel> {
        Self::ALL.get(self.step() + 1).copied()
    }

    pub fn mods(self) -> VariantMods {
        let m = |cost, mass, upkeep, effect| VariantMods { cost, mass, upkeep, effect };
        // Эффект растёт круче, чем масса и содержание, — и это главное, ради
        // чего уровни вообще существуют. Прокачка обязана **ощущаться**: между
        // дешёвым и совершенным разница почти впятеро по силе при вдвое
        // большем весе. Иначе развитие снова сведётся к «набери побольше
        // органов», а не «доведи до ума то, что уже есть».
        match self {
            PartLevel::Cheap => m(0.55, 0.70, 0.70, 0.60),
            PartLevel::Plain => m(1.00, 1.00, 1.00, 1.00),
            PartLevel::Fine => m(2.20, 1.35, 1.50, 1.75),
            PartLevel::Perfect => m(4.50, 1.80, 2.10, 2.90),
        }
    }
}

impl PartKind {
    pub const fn new(family: PartFamily, level: PartLevel) -> Self {
        Self { family, level }
    }

    /// The plain version of an organ.
    pub const fn basic(family: PartFamily) -> Self {
        Self::new(family, PartLevel::Plain)
    }

    /// Самый дешёвый уровень: с него начинают, если считают очки.
    pub const fn cheap(family: PartFamily) -> Self {
        Self::new(family, PartLevel::Cheap)
    }

    /// Все мутации: каждый орган на каждом уровне.
    pub fn all() -> impl Iterator<Item = PartKind> {
        PartFamily::ALL
            .into_iter()
            .flat_map(|family| PartLevel::ALL.into_iter().map(move |v| PartKind::new(family, v)))
    }

    /// How many mutations exist in total.
    pub const COUNT: usize = PartFamily::ALL.len() * PartLevel::ALL.len();

    pub fn index(self) -> usize {
        self.family.slot() * PartLevel::ALL.len() + self.level.step()
    }

    pub fn from_index(index: usize) -> Self {
        let index = index % Self::COUNT;
        let family = PartFamily::ALL[index / PartLevel::ALL.len()];
        let level = PartLevel::ALL[index % PartLevel::ALL.len()];
        Self::new(family, level)
    }

    /// Тот же орган на следующем уровне, если он есть.
    pub fn upgraded(self) -> Option<PartKind> {
        self.level.next().map(|level| Self::new(self.family, level))
    }

    pub fn name(self) -> String {
        format!("{} ({})", self.family.name(), self.level.name())
    }

    pub fn tradeoff(self) -> String {
        format!("{} · {}", self.family.tradeoff(), self.level.hint())
    }

    pub fn is_external(self) -> bool {
        self.family.is_external()
    }
}

/// One table drives the simulation, the UI tooltips and the balance tests, so a
/// part's advertised trade-off cannot drift from its real one.
#[derive(Debug, Clone, Copy)]
pub struct PartStats {
    pub cost: u16,
    pub mass: f32,
    /// Energy per second this part costs to keep.
    pub upkeep: f32,
    /// Units per second added to swimming speed (may be negative).
    pub speed: f32,
    pub temperature: f32,
    pub salinity: f32,
    pub toxin: f32,
    pub oxygen: f32,
    /// Extra feeding radius.
    pub reach: f32,
    /// Extra energy capacity.
    pub storage: f32,
    /// Energy per second produced from light.
    pub photosynthesis: f32,
    /// Extra distance at which the client highlights food.
    pub sense: f32,
    /// Damage per second dealt on contact.
    pub attack: f32,
    /// Fraction of incoming damage absorbed.
    pub defense: f32,
    /// Multiplier on how fast the cell can divide.
    pub reproduction: f32,
    /// Added chance that an offspring mutates.
    pub mutagen: f32,
    /// Toxin released into the water around the cell, per second.
    pub toxin_emission: f32,
    /// Урон от тарана: множитель к вкладу массы в атаку.
    pub ram: f32,
    /// Насколько орган очищает воду вокруг тела.
    pub cleansing: f32,
    /// Насколько тело протискивается в кусты сверх своего размера.
    pub squeeze: f32,
    /// Насколько масса меньше давит на скорость.
    pub buoyancy: f32,
}

impl PartStats {
    pub const NONE: PartStats = PartStats {
        cost: 2,
        mass: 0.6,
        upkeep: 0.05,
        speed: 0.0,
        temperature: 0.0,
        salinity: 0.0,
        toxin: 0.0,
        oxygen: 0.0,
        reach: 0.0,
        storage: 0.0,
        photosynthesis: 0.0,
        sense: 0.0,
        attack: 0.0,
        defense: 0.0,
        reproduction: 0.0,
        mutagen: 0.0,
        toxin_emission: 0.0,
        ram: 0.0,
        cleansing: 0.0,
        squeeze: 0.0,
        buoyancy: 0.0,
    };
}

/// Stats of one of the 200 parts: the family's base, reshaped by the variant.
pub fn stats(kind: PartKind) -> PartStats {
    let base = kind.family.base();
    let m = kind.level.mods();
    let e = m.effect;
    PartStats {
        cost: ((base.cost as f32 * m.cost).round() as u16).max(1),
        mass: base.mass * m.mass,
        upkeep: base.upkeep * m.upkeep,
        speed: base.speed * e,
        temperature: base.temperature * e,
        salinity: base.salinity * e,
        toxin: base.toxin * e,
        oxygen: base.oxygen * e,
        reach: base.reach * e,
        storage: base.storage * e,
        photosynthesis: base.photosynthesis * e,
        sense: base.sense * e,
        attack: base.attack * e,
        defense: base.defense * e,
        reproduction: base.reproduction * e,
        mutagen: base.mutagen * e,
        toxin_emission: base.toxin_emission * e,
        ram: base.ram * e,
        cleansing: base.cleansing * e,
        squeeze: base.squeeze * e,
        buoyancy: base.buoyancy * e,
    }
}

/// Human-readable summary of what a part actually does, built from its numbers.
///
/// Generated rather than written by hand: with 200 mutations, any hand-written
/// description would drift away from the maths within a week.
pub fn effect_summary(kind: PartKind) -> String {
    let s = stats(kind);
    let mut parts: Vec<String> = Vec::new();
    let signed = |v: f32| if v > 0.0 { format!("+{v:.1}") } else { format!("{v:.1}") };
    let signed2 = |v: f32| if v > 0.0 { format!("+{v:.2}") } else { format!("{v:.2}") };

    if s.speed != 0.0 {
        parts.push(format!("скорость {}", signed(s.speed)));
    }
    if s.attack != 0.0 {
        parts.push(format!("урон {}/с", signed(s.attack)));
    }
    if s.defense != 0.0 {
        parts.push(format!("защита {:+.0}%", s.defense * 100.0));
    }
    if s.reach != 0.0 {
        parts.push(format!("захват {}", signed2(s.reach)));
    }
    if s.sense != 0.0 {
        parts.push(format!("обзор {}", signed(s.sense)));
    }
    if s.storage != 0.0 {
        parts.push(format!("запас {}", signed(s.storage)));
    }
    if s.photosynthesis != 0.0 {
        parts.push(format!("свет {}/с", signed2(s.photosynthesis)));
    }
    if s.temperature != 0.0 {
        parts.push(format!("температура {}", signed2(s.temperature)));
    }
    if s.salinity != 0.0 {
        parts.push(format!("солёность {}", signed2(s.salinity)));
    }
    if s.toxin != 0.0 {
        parts.push(format!("стойкость к яду {}", signed2(s.toxin)));
    }
    if s.oxygen != 0.0 {
        parts.push(format!("кислород {}", signed2(s.oxygen)));
    }
    if s.reproduction != 0.0 {
        parts.push(format!("деление {:+.0}%", s.reproduction * 100.0));
    }
    if s.mutagen != 0.0 {
        parts.push(format!("мутации потомков {:+.0}%", s.mutagen * 100.0));
    }
    if s.toxin_emission != 0.0 {
        parts.push(format!("травит воду {:.2}/с", s.toxin_emission));
    }

    if parts.is_empty() {
        "без прямого эффекта".to_string()
    } else {
        parts.join(", ")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BodyPart {
    pub kind: PartKind,
    pub position: Vec3,
    pub rotation: Quat,
    pub level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Genome {
    pub parts: Vec<BodyPart>,
    pub mutation_points: u16,
    /// Family line: offspring inherit it, and kin never fight each other.
    pub lineage: u64,
    pub generation: u16,
}

/// Two strangers tolerate each other while their bodies differ by at most this
/// many organs. Past that they are different enough to be prey.
pub const AGGRESSION_THRESHOLD: u32 = 7;

/// Kin tolerate far more — but not everything. Once a branch of the family has
/// drifted this far from another, they stop recognising each other as kin and
/// the line splits.
pub const KIN_SPLIT_THRESHOLD: u32 = 15;

/// Способность, которую игрок применяет сам.
///
/// Перк — это не орган. Орган работает всегда и сам, перк ждёт нажатия и потом
/// молчит, пока не остынет. Отсюда и разница в ощущении: органы делают тебя
/// таким, какой ты есть, а перк — это решение в конкретную секунду.
///
/// **Перезарядка зависит от массы.** Крупное тело копит силу дольше — и это
/// единственное место, где масса мешает, а не помогает. Так у мелких остаётся
/// то, чем они лучше больших.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Perk {
    /// Спрут: выброс ядовитого облака и рывок по направлению взгляда.
    ///
    /// Оружие догоняющего и убегающего сразу: облако остаётся позади, рывок
    /// уносит вперёд. Что из этого важнее — решает тот, кто нажал.
    Squid,
    /// Продолжение рода: тело делится на три, каждое сохраняет большую часть
    /// нажитого и получает ускорение.
    ///
    /// Способ разменять одно сильное тело на три быстрых — и единственный, в
    /// котором игрок сам выбирает, когда это сделать.
    Lineage,
}

impl Perk {
    pub const ALL: [Perk; 2] = [Perk::Squid, Perk::Lineage];

    pub fn name(self) -> &'static str {
        match self {
            Perk::Squid => "Спрут",
            Perk::Lineage => "Продолжение рода",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Perk::Squid => "облако яда позади, рывок вперёд",
            Perk::Lineage => "делишься на троих, каждый быстрее тебя",
        }
    }

    /// Базовая перезарядка для тела стартовой массы, секунд.
    pub fn base_cooldown(self) -> f32 {
        match self {
            Perk::Squid => 9.0,
            Perk::Lineage => 75.0,
        }
    }

    /// Перезарядка для тела такой массы.
    ///
    /// Растёт от массы: чем крупнее тело, тем реже оно способно на рывок.
    /// Это единственное, чем мелкий лучше крупного, — и потому важное.
    pub fn cooldown(self, mass: f32) -> f32 {
        let bulk = 1.0 + (mass - BASE_MASS).max(0.0) * PERK_MASS_SLOWDOWN;
        self.base_cooldown() * bulk
    }
}

/// Сколько органов каждого семейства несёт тело: двадцать ячеек в порядке
/// [`PartFamily::ALL`].
///
/// Родство считается по семействам, а не по точным частям, значит вся нужная
/// для него информация о теле — эти двадцать чисел. Гистограмма меняется только
/// когда тело отращивает орган, поэтому [`OrganismState`] держит её посчитанной
/// и не собирает заново на каждое сравнение.
///
/// Раньше `genetic_distance` для одной пары делала сорок проходов по частям
/// обоих тел (двадцать семейств × два тела). При семидесяти особях это две
/// тысячи пар за тик — и это была самая дорогая операция на сервере.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FamilyCounts(pub [u8; PartFamily::ALL.len()]);

impl FamilyCounts {
    pub const EMPTY: FamilyCounts = FamilyCounts([0; PartFamily::ALL.len()]);

    /// Считает гистограмму по геному. Один проход по частям вместо двадцати.
    pub fn of(genome: &Genome) -> Self {
        let mut counts = [0u8; PartFamily::ALL.len()];
        for part in &genome.parts {
            let slot = part.kind.family.slot();
            // Насытить, а не переполнить: тело ограничено сотней частей, но
            // предел живёт в конфиге сервера и может быть поднят.
            counts[slot] = counts[slot].saturating_add(1);
        }
        Self(counts)
    }

    pub fn get(&self, family: PartFamily) -> u8 {
        self.0[family.slot()]
    }

    /// Сколько органов разделяет два тела.
    pub fn distance(&self, other: &Self) -> u32 {
        let mut total = 0u32;
        for slot in 0..PartFamily::ALL.len() {
            total += (self.0[slot] as i32 - other.0[slot] as i32).unsigned_abs();
        }
        total
    }
}

impl Default for FamilyCounts {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// How many organs separate two genomes.
///
/// Counted by family, not by exact part: a large flagellum and a small one are
/// still both flagella, and two cells built on the same plan should not become
/// enemies over which variant they grew.
///
/// Тем, кто сравнивает тела в цикле, нужна не эта функция, а
/// [`FamilyCounts::distance`] по уже посчитанным гистограммам: здесь обе
/// собираются заново.
pub fn genetic_distance(a: &Genome, b: &Genome) -> u32 {
    FamilyCounts::of(a).distance(&FamilyCounts::of(b))
}

/// Whether these two will fight on contact.
///
/// Kin are given a much longer leash than strangers, but a lineage is not a
/// permanent truce: a branch that has grown 15 organs away from another is no
/// longer recognisable as family, and the line splits into enemies.
pub fn hostile(a: &Genome, b: &Genome) -> bool {
    hostile_with(a, b, AGGRESSION_THRESHOLD, KIN_SPLIT_THRESHOLD)
}

/// Same rule with the server's own thresholds, which it may have configured.
pub fn hostile_with(a: &Genome, b: &Genome, strangers: u32, kin: u32) -> bool {
    hostile_counts(
        &FamilyCounts::of(a),
        a.lineage,
        &FamilyCounts::of(b),
        b.lineage,
        strangers,
        kin,
    )
}

/// То же решение по уже посчитанным гистограммам — вариант для циклов по парам.
///
/// Ответ обязан совпадать с [`hostile_with`]: это одно правило, записанное так,
/// чтобы его можно было применить, ничего не пересчитывая.
pub fn hostile_counts(
    a: &FamilyCounts,
    a_lineage: u64,
    b: &FamilyCounts,
    b_lineage: u64,
    strangers: u32,
    kin: u32,
) -> bool {
    let distance = a.distance(b);
    if a_lineage == b_lineage {
        distance > kin
    } else {
        distance > strangers
    }
}

/// What the next organ costs in this particular body.
///
/// The base price comes from the part; the multiplier comes from how much has
/// already been grown, so growth gets dearer as the body fills up.
///
/// Цена **не убывает** и заметно растёт на дистанции, но не обязана
/// увеличиваться на каждом отдельном органе. Раньше строгий рост держался
/// плоской надбавкой `+ grown`, и она же незаметно стала главным источником
/// крутизны: к пятидесятому органу из ста пятидесяти пяти очков сорок семь
/// давал множитель и сорок семь — она сама. Игрок упирался в стену ровно
/// тогда, когда начинал понимать, что хочет построить.
///
/// Смысл надбавки не в том, чтобы остановить рост, а в том, чтобы поздние
/// органы стоили ощутимо дороже ранних. Для этого достаточно множителя.
pub fn mutation_price(genome: &Genome, kind: PartKind) -> u16 {
    let base = stats(kind).cost as f32;
    // The three starter organs are free of the surcharge.
    let grown = genome.parts.len().saturating_sub(3) as f32;
    let scale = 1.0 + MUTATION_PRICE_LINEAR * grown + MUTATION_PRICE_QUADRATIC * grown * grown;
    // Плоская надбавка гарантирует, что цена растёт **строго**: без неё
    // округление делает два подряд идущих органа одинаковыми по цене, и
    // «каждая мутация дороже предыдущей» перестаёт быть правдой.
    //
    (base * scale).ceil().max(1.0) as u16
}

/// Deterministic attachment slot on the membrane, spread by the golden angle so
/// that parts never pile up on top of each other however many are added.
pub fn slot_direction(index: usize) -> Vec3 {
    let i = index as f32;
    let golden = 2.399_963_2_f32;
    // Bias the first slots toward the equator; the body reads better than a
    // uniform sphere distribution and the silhouette stays wide.
    let y = ((i * 0.37).sin() * 0.55).clamp(-0.8, 0.8);
    let r = (1.0 - y * y).sqrt();
    let a = i * golden;
    Vec3::new(a.cos() * r, y, a.sin() * r).normalize_or(Vec3::X)
}

/// Where a part sits, given how many parts came before it.
pub fn slot_transform(family: PartFamily, index: usize, body_radius: f32) -> (Vec3, Quat) {
    let dir = slot_facing(family, index);
    let rotation = Quat::from_rotation_arc(Vec3::Y, dir);
    (dir * body_radius * slot_depth(family), rotation)
}

/// Куда смотрит слот — без всякого радиуса.
pub fn slot_facing(family: PartFamily, index: usize) -> Vec3 {
    match family {
        // The flagellum always trails directly behind, that is what it is for.
        PartFamily::Flagellum => Vec3::new(0.0, 0.0, 1.0),
        // The mouth leads.
        PartFamily::Mouth => Vec3::new(0.0, 0.0, -1.0),
        _ => slot_direction(index),
    }
}

/// Насколько глубоко в теле сидит орган, в долях радиуса.
///
/// Доля, а не расстояние: тело растёт, и орган обязан расти вместе с ним.
/// Именно это когда-то и сломалось — позиция части запекалась в геном по
/// прикидочному радиусу, а тело потом раздувалось от массы, и внешние органы
/// оказывались внутри пузыря.
pub fn slot_depth(family: PartFamily) -> f32 {
    if family.is_external() {
        0.86
    } else {
        0.42
    }
}

impl Genome {
    pub fn starter() -> Self {
        Self::starter_of(0)
    }

    pub fn starter_of(lineage: u64) -> Self {
        let mut genome =
            Self { parts: Vec::new(), mutation_points: 0, lineage, generation: 0 };
        for family in [PartFamily::Membrane, PartFamily::Flagellum, PartFamily::Mouth] {
            genome.push_part(PartKind::basic(family));
        }
        genome
    }

    pub fn count(&self, kind: PartKind) -> usize {
        self.parts.iter().filter(|p| p.kind == kind).count()
    }

    /// Organs of this family, whatever variant they are.
    pub fn count_family(&self, family: PartFamily) -> usize {
        self.parts.iter().filter(|p| p.kind.family == family).count()
    }

    /// Adds a part in the next free slot. Layout is derived, never random, so the
    /// same genome always produces the same body.
    pub fn push_part(&mut self, kind: PartKind) {
        let index = self.parts.len();
        // Radius is estimated from the part count: the exact radius depends on mass,
        // which depends on the parts, so a fixed-point pass is not worth it here.
        let radius = 0.85 + (index as f32 * 0.05).min(0.7);
        let (position, rotation) = slot_transform(kind.family, index, radius);
        self.parts.push(BodyPart { kind, position, rotation, level: 1 });
    }
}

/// Builds an offspring genome from its parent.
///
/// The child inherits **every part the parent has** — that is the whole point of
/// a lineage — and then may grow one extra part of its own. Points are not
/// inherited: they are earned, not bequeathed.
pub fn conceive(parent: &Genome, mutates: bool, roll: u64) -> Genome {
    let mut child = parent.clone();
    child.mutation_points = 0;
    child.generation = child.generation.saturating_add(1);
    if mutates && child.parts.len() < MAX_PARTS {
        child.push_part(crate::random_part(roll));
    }
    child
}

impl Default for Genome {
    fn default() -> Self {
        Self::starter()
    }
}

#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrganismState {
    pub genome: Genome,
    /// Гистограмма семейств генома, посчитанная в [`OrganismState::recompute`].
    ///
    /// Кэш, а не отдельное состояние: он обязан совпадать с
    /// `FamilyCounts::of(&self.genome)` в любой момент. Всё, что меняет геном,
    /// проходит через `recompute`.
    pub families: FamilyCounts,
    pub mass: f32,
    pub energy: f32,
    pub health: f32,
    pub age: f32,
    /// Energy eaten in total; converted into mutation points.
    pub absorbed: f32,
    /// Counts down after taking damage; blocks healing while it runs.
    pub combat_timer: f32,
    /// Сколько секунд осталось до готовности каждого перка.
    ///
    /// Хранится в теле, а не отдельно: перезарядка — свойство организма, и при
    /// пересадке в потомка она должна переезжать вместе с ним.
    pub perk_cooldowns: [f32; Perk::ALL.len()],
    pub temperature_tolerance: f32,
    pub salinity_tolerance: f32,
    pub toxin_resistance: f32,
    pub oxygen_affinity: f32,
}

pub const BASE_TEMPERATURE_TOLERANCE: f32 = 0.16;
pub const BASE_SALINITY_TOLERANCE: f32 = 0.16;
pub const BASE_TOXIN_RESISTANCE: f32 = 0.06;
pub const BASE_OXYGEN_AFFINITY: f32 = 0.0;
pub const BASE_MASS: f32 = 6.0;

impl Default for OrganismState {
    fn default() -> Self {
        let mut state = Self {
            genome: Genome::starter(),
            families: FamilyCounts::EMPTY,
            mass: BASE_MASS,
            energy: BASE_ENERGY_CAP,
            health: MAX_HEALTH,
            age: 0.0,
            absorbed: 0.0,
            combat_timer: 0.0,
            perk_cooldowns: [0.0; Perk::ALL.len()],
            temperature_tolerance: BASE_TEMPERATURE_TOLERANCE,
            salinity_tolerance: BASE_SALINITY_TOLERANCE,
            toxin_resistance: BASE_TOXIN_RESISTANCE,
            oxygen_affinity: BASE_OXYGEN_AFFINITY,
        };
        state.recompute();
        state.health = crate::max_health(state.mass);
        state.energy = state.energy_cap();
        state
    }
}

impl OrganismState {
    /// Rebuilds an organism from a genome alone. Only the genome is replicated, so
    /// everything derived from it has to be recomputed rather than carried.
    pub fn from_genome(genome: Genome) -> Self {
        let mut state = Self { genome, ..Default::default() };
        state.recompute();
        // Полное здоровье, а не константа: у крупного тела потолок выше, и
        // рождаться сразу раненым оно не должно.
        state.health = crate::max_health(state.mass);
        state.energy = state.energy_cap();
        state
    }

    /// Tolerances and mass are a pure function of the parts.
    ///
    /// Гистограмма семейств — тоже: она пересчитывается здесь, чтобы родство
    /// сравнивалось по готовым числам, а не по частям тела.
    pub fn recompute(&mut self) {
        self.families = FamilyCounts::of(&self.genome);
        let mut temperature = BASE_TEMPERATURE_TOLERANCE;
        let mut salinity = BASE_SALINITY_TOLERANCE;
        let mut toxin = BASE_TOXIN_RESISTANCE;
        let mut oxygen = BASE_OXYGEN_AFFINITY;
        let mut mass = BASE_MASS;
        for p in &self.genome.parts {
            let s = stats(p.kind);
            temperature += s.temperature;
            salinity += s.salinity;
            toxin += s.toxin;
            oxygen += s.oxygen;
            mass += s.mass;
        }
        self.temperature_tolerance = temperature;
        self.salinity_tolerance = salinity;
        self.toxin_resistance = toxin;
        self.oxygen_affinity = oxygen;
        self.mass = mass;
    }

    pub fn energy_cap(&self) -> f32 {
        // A symbiont eats into the pantry, so the cap has to stay positive.
        (BASE_ENERGY_CAP + self.genome.parts.iter().map(|p| stats(p.kind).storage).sum::<f32>())
            .max(30.0)
    }

    pub fn has_part(&self, kind: PartKind) -> bool {
        self.genome.parts.iter().any(|p| p.kind == kind)
    }

    pub fn count_part(&self, kind: PartKind) -> usize {
        self.genome.count(kind)
    }

    /// Price of this part for this organism, surcharge included.
    pub fn price(&self, kind: PartKind) -> u16 {
        mutation_price(&self.genome, kind)
    }

    /// Base price of a part, before the body's surcharge.
    pub fn mutation_cost(kind: PartKind) -> u16 {
        stats(kind).cost
    }

    /// Why a mutation cannot be applied, or `None` if it can.
    pub fn mutation_error(&self, kind: PartKind) -> Option<&'static str> {
        if self.genome.parts.len() >= MAX_PARTS {
            return Some("тело заполнено");
        }
        // The limit is per family: otherwise ten variants of the same organ are a
        // way around it.
        if self.genome.count_family(kind.family) >= MAX_PARTS_PER_KIND {
            return Some("предел для этого органа");
        }
        if self.genome.mutation_points < self.price(kind) {
            return Some("не хватает очков");
        }
        None
    }

    /// Какой орган этого семейства будет подниматься: самый слабый из имеющихся.
    ///
    /// Именно самый слабый, а не любой: прокачка должна подтягивать отстающее,
    /// иначе игрок будет доводить один орган до совершенства, пока остальные
    /// остаются дешёвыми, — и не заметит, что тело перекошено.
    pub fn weakest_of(&self, family: PartFamily) -> Option<usize> {
        self.genome
            .parts
            .iter()
            .enumerate()
            .filter(|(_, part)| part.kind.family == family)
            .min_by_key(|(_, part)| part.kind.level)
            .map(|(index, _)| index)
    }

    /// Во что обойдётся поднять этот орган на уровень.
    ///
    /// Разница цен между уровнями, а не полная цена нового: вкладываться в уже
    /// отращённое должно быть выгоднее, чем отращивать рядом ещё одно. Иначе
    /// прокачки не будет вовсе — все просто продолжат набирать органы.
    pub fn upgrade_price(&self, family: PartFamily) -> Option<u16> {
        let index = self.weakest_of(family)?;
        let current = self.genome.parts[index].kind;
        let next = current.upgraded()?;
        Some(self.price(next).saturating_sub(self.price(current)).max(1))
    }

    /// Почему орган нельзя поднять, или `None`, если можно.
    pub fn upgrade_error(&self, family: PartFamily) -> Option<&'static str> {
        let Some(index) = self.weakest_of(family) else {
            return Some("такого органа нет");
        };
        if self.genome.parts[index].kind.upgraded().is_none() {
            return Some("уже совершенный");
        }
        match self.upgrade_price(family) {
            Some(price) if self.genome.mutation_points >= price => None,
            _ => Some("не хватает очков"),
        }
    }

    /// Поднимает самый слабый орган этого семейства на уровень.
    pub fn apply_upgrade(&mut self, family: PartFamily) -> bool {
        if self.upgrade_error(family).is_some() {
            return false;
        }
        let (Some(index), Some(price)) = (self.weakest_of(family), self.upgrade_price(family))
        else {
            return false;
        };
        let Some(next) = self.genome.parts[index].kind.upgraded() else { return false };

        let before = crate::max_health(self.mass);
        self.genome.mutation_points -= price;
        self.genome.parts[index].kind = next;
        self.genome.parts[index].level = next.level.step() as u8 + 1;
        self.recompute();
        self.grow_into_new_body(before);
        true
    }

    /// Прибавляет здоровье, которое дала выросшая масса.
    ///
    /// Без этого прокачка ощущалась наказанием: потолок здоровья поднимался, а
    /// сама полоска оставалась на месте, и организм после вложенных очков
    /// выглядел **более раненым**, чем до них. Новая плоть приходит целой.
    fn grow_into_new_body(&mut self, previous_max: f32) {
        let gained = crate::max_health(self.mass) - previous_max;
        if gained > 0.0 {
            self.health += gained;
        }
    }

    pub fn apply_mutation(&mut self, kind: PartKind) -> bool {
        if self.mutation_error(kind).is_some() {
            return false;
        }
        let before = crate::max_health(self.mass);
        self.genome.mutation_points -= self.price(kind);
        self.genome.push_part(kind);
        self.recompute();
        self.grow_into_new_body(before);
        true
    }

    /// Готов ли перк к применению.
    pub fn perk_ready(&self, perk: Perk) -> bool {
        self.perk_cooldowns[perk as usize] <= 0.0
    }

    /// Ставит перк на перезарядку.
    pub fn spend_perk(&mut self, perk: Perk) {
        self.perk_cooldowns[perk as usize] = perk.cooldown(self.mass);
    }

    /// Двигает перезарядки. Зовётся раз в тик.
    pub fn tick_perks(&mut self, dt: f32) {
        for left in &mut self.perk_cooldowns {
            *left = (*left - dt).max(0.0);
        }
    }

    /// Доля готовности, 0..1 — для шкалы в интерфейсе.
    pub fn perk_readiness(&self, perk: Perk) -> f32 {
        let full = perk.cooldown(self.mass).max(0.01);
        1.0 - (self.perk_cooldowns[perk as usize] / full).clamp(0.0, 1.0)
    }

    /// Converts absorbed energy into mutation points and returns how many were earned.
    pub fn claim_points(&mut self) -> u16 {
        self.claim_points_at(ENERGY_PER_MUTATION_POINT)
    }

    /// Same, at the server's configured exchange rate.
    pub fn claim_points_at(&mut self, energy_per_point: f32) -> u16 {
        let rate = energy_per_point.max(1.0);
        let earned = (self.absorbed / rate) as u16;
        if earned > 0 {
            self.absorbed -= earned as f32 * rate;
            self.genome.mutation_points = self.genome.mutation_points.saturating_add(earned);
        }
        earned
    }
}
