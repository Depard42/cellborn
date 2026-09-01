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
    Eye,
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
    Symbiont,
    Chemoreceptor,
    Carapace,
}

/// How that organ turned out. The same organ grown ten different ways: cheaper,
/// heavier, more potent, more wasteful.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PartVariant {
    Basic,
    Small,
    Large,
    Potent,
    Thrifty,
    Fragile,
    Dense,
    Twin,
    Feral,
    Refined,
}

/// A part is a family grown in one particular way: 20 × 10 = **200 mutations**,
/// each with its own cost, mass, upkeep and effect — without 200 hand-written
/// table rows that would inevitably drift apart.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PartKind {
    pub family: PartFamily,
    pub variant: PartVariant,
}

impl PartFamily {
    pub const ALL: [PartFamily; 20] = [
        PartFamily::Flagellum,
        PartFamily::Cilia,
        PartFamily::Pseudopod,
        PartFamily::Mouth,
        PartFamily::Symbiont,
        PartFamily::Eye,
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
        PartFamily::Membrane,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PartFamily::Membrane => "Мембрана",
            PartFamily::Flagellum => "Жгутик",
            PartFamily::Cilia => "Реснички",
            PartFamily::Mouth => "Рот",
            PartFamily::Eye => "Глаз",
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
            PartFamily::Symbiont => "Симбионт",
            PartFamily::Chemoreceptor => "Хеморецептор",
            PartFamily::Carapace => "Панцирь",
        }
    }

    pub fn tradeoff(self) -> &'static str {
        match self {
            PartFamily::Membrane => "+запас энергии, +масса",
            PartFamily::Flagellum => "+скорость, дорогой в содержании",
            PartFamily::Cilia => "+немного скорости, дёшево",
            PartFamily::Mouth => "+радиус захвата пищи",
            PartFamily::Eye => "+дальность обзора",
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
            PartFamily::Symbiont => "даёт энергию, но кормится сам",
            PartFamily::Chemoreceptor => "видит еду издалека, дёшево",
            PartFamily::Carapace => "много защиты, много массы",
        }
    }

    /// Whether the organ sits on the membrane surface or floats inside the cell.
    pub fn is_external(self) -> bool {
        matches!(
            self,
            PartFamily::Flagellum
                | PartFamily::Cilia
                | PartFamily::Mouth
                | PartFamily::Eye
                | PartFamily::Spike
                | PartFamily::MucusCoat
                | PartFamily::ThermalMembrane
                | PartFamily::Pseudopod
                | PartFamily::Nematocyst
                | PartFamily::Carapace
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
            Eye => PartStats { cost: 2, mass: 0.4, upkeep: 0.04, sense: 6.0, ..d },
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
            // A lodger that pays rent in energy and eats some of the pantry.
            Symbiont => PartStats {
                cost: 4, mass: 0.9, upkeep: 0.18, photosynthesis: 0.30, storage: -8.0, ..d
            },
            Chemoreceptor => PartStats { cost: 1, mass: 0.2, upkeep: 0.02, sense: 4.0, ..d },
            Carapace => PartStats {
                cost: 4, mass: 3.0, upkeep: 0.05, defense: 0.28, speed: -0.4, ..d
            },
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

impl PartVariant {
    pub const ALL: [PartVariant; 10] = [
        PartVariant::Basic,
        PartVariant::Small,
        PartVariant::Large,
        PartVariant::Potent,
        PartVariant::Thrifty,
        PartVariant::Fragile,
        PartVariant::Dense,
        PartVariant::Twin,
        PartVariant::Feral,
        PartVariant::Refined,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PartVariant::Basic => "обычный",
            PartVariant::Small => "малый",
            PartVariant::Large => "крупный",
            PartVariant::Potent => "усиленный",
            PartVariant::Thrifty => "экономный",
            PartVariant::Fragile => "хрупкий",
            PartVariant::Dense => "плотный",
            PartVariant::Twin => "двойной",
            PartVariant::Feral => "дикий",
            PartVariant::Refined => "совершенный",
        }
    }

    /// One line explaining the trade this variant makes.
    pub fn hint(self) -> &'static str {
        match self {
            PartVariant::Basic => "как есть",
            PartVariant::Small => "слабее, но лёгкий и дешёвый",
            PartVariant::Large => "сильнее, тяжелее, дороже в содержании",
            PartVariant::Potent => "вдвое эффективнее, прожорливый",
            PartVariant::Thrifty => "почти не ест энергию, чуть слабее",
            PartVariant::Fragile => "эффективный и лёгкий, дёшев в очках",
            PartVariant::Dense => "тяжёлый, но крепкий и недорогой в содержании",
            PartVariant::Twin => "две штуки в одной, во всём вдвое",
            PartVariant::Feral => "мощнее, но ест много",
            PartVariant::Refined => "лучший во всём, кроме цены",
        }
    }

    pub fn mods(self) -> VariantMods {
        let m = |cost, mass, upkeep, effect| VariantMods { cost, mass, upkeep, effect };
        match self {
            PartVariant::Basic => m(1.0, 1.0, 1.0, 1.0),
            PartVariant::Small => m(0.6, 0.5, 0.6, 0.6),
            PartVariant::Large => m(1.6, 1.8, 1.5, 1.6),
            PartVariant::Potent => m(1.8, 1.2, 1.9, 2.0),
            PartVariant::Thrifty => m(1.3, 1.0, 0.4, 0.85),
            PartVariant::Fragile => m(0.5, 0.6, 0.8, 1.35),
            PartVariant::Dense => m(1.4, 2.2, 0.9, 1.25),
            PartVariant::Twin => m(1.9, 1.9, 1.8, 2.0),
            PartVariant::Feral => m(1.1, 1.1, 1.7, 1.5),
            PartVariant::Refined => m(2.6, 0.9, 0.85, 1.8),
        }
    }
}

impl PartKind {
    pub const fn new(family: PartFamily, variant: PartVariant) -> Self {
        Self { family, variant }
    }

    /// The plain version of an organ.
    pub const fn basic(family: PartFamily) -> Self {
        Self::new(family, PartVariant::Basic)
    }

    /// All 200 mutations, family by family.
    pub fn all() -> impl Iterator<Item = PartKind> {
        PartFamily::ALL
            .into_iter()
            .flat_map(|family| PartVariant::ALL.into_iter().map(move |v| PartKind::new(family, v)))
    }

    /// How many mutations exist in total.
    pub const COUNT: usize = PartFamily::ALL.len() * PartVariant::ALL.len();

    pub fn index(self) -> usize {
        let f = PartFamily::ALL.iter().position(|f| *f == self.family).unwrap_or(0);
        let v = PartVariant::ALL.iter().position(|v| *v == self.variant).unwrap_or(0);
        f * PartVariant::ALL.len() + v
    }

    pub fn from_index(index: usize) -> Self {
        let index = index % Self::COUNT;
        let family = PartFamily::ALL[index / PartVariant::ALL.len()];
        let variant = PartVariant::ALL[index % PartVariant::ALL.len()];
        Self::new(family, variant)
    }

    pub fn name(self) -> String {
        match self.variant {
            PartVariant::Basic => self.family.name().to_string(),
            other => format!("{} ({})", self.family.name(), other.name()),
        }
    }

    pub fn tradeoff(self) -> String {
        format!("{} · {}", self.family.tradeoff(), self.variant.hint())
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
    };
}

/// Stats of one of the 200 parts: the family's base, reshaped by the variant.
pub fn stats(kind: PartKind) -> PartStats {
    let base = kind.family.base();
    let m = kind.variant.mods();
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

/// How many organs separate two genomes.
///
/// Counted by family, not by exact part: a large flagellum and a small one are
/// still both flagella, and two cells built on the same plan should not become
/// enemies over which variant they grew.
pub fn genetic_distance(a: &Genome, b: &Genome) -> u32 {
    PartFamily::ALL
        .iter()
        .map(|family| {
            (a.count_family(*family) as i32 - b.count_family(*family) as i32).unsigned_abs()
        })
        .sum()
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
    let distance = genetic_distance(a, b);
    if a.lineage == b.lineage {
        distance > kin
    } else {
        distance > strangers
    }
}

/// What the next organ costs in this particular body.
///
/// The base price comes from the part; the multiplier comes from how much has
/// already been grown, so every mutation makes the following one dearer.
pub fn mutation_price(genome: &Genome, kind: PartKind) -> u16 {
    let base = stats(kind).cost as f32;
    // The three starter organs are free of the surcharge.
    let grown = genome.parts.len().saturating_sub(3) as f32;
    let scale = 1.0 + MUTATION_PRICE_LINEAR * grown + MUTATION_PRICE_QUADRATIC * grown * grown;
    // The flat `+ grown` guarantees the price is *strictly* rising: without it
    // rounding makes two consecutive organs cost the same, and "every mutation
    // is dearer than the last" stops being true.
    ((base * scale).ceil() as u16 + grown as u16).max(1)
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
    let dir = match family {
        // The flagellum always trails directly behind, that is what it is for.
        PartFamily::Flagellum => Vec3::new(0.0, 0.0, 1.0),
        // The mouth leads.
        PartFamily::Mouth => Vec3::new(0.0, 0.0, -1.0),
        _ => slot_direction(index),
    };
    let depth = if family.is_external() { 0.86 } else { 0.42 };
    let rotation = Quat::from_rotation_arc(Vec3::Y, dir);
    (dir * body_radius * depth, rotation)
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
    pub mass: f32,
    pub energy: f32,
    pub health: f32,
    pub age: f32,
    /// Energy eaten in total; converted into mutation points.
    pub absorbed: f32,
    /// Counts down after taking damage; blocks healing while it runs.
    pub combat_timer: f32,
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
            mass: BASE_MASS,
            energy: BASE_ENERGY_CAP,
            health: MAX_HEALTH,
            age: 0.0,
            absorbed: 0.0,
            combat_timer: 0.0,
            temperature_tolerance: BASE_TEMPERATURE_TOLERANCE,
            salinity_tolerance: BASE_SALINITY_TOLERANCE,
            toxin_resistance: BASE_TOXIN_RESISTANCE,
            oxygen_affinity: BASE_OXYGEN_AFFINITY,
        };
        state.recompute();
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
        state
    }

    /// Tolerances and mass are a pure function of the parts.
    pub fn recompute(&mut self) {
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

    pub fn apply_mutation(&mut self, kind: PartKind) -> bool {
        if self.mutation_error(kind).is_some() {
            return false;
        }
        self.genome.mutation_points -= self.price(kind);
        self.genome.push_part(kind);
        self.recompute();
        true
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
