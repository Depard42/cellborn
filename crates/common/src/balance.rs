//! Every gameplay constant in one place, expressed **per second**.
//!
//! Per-tick constants would make `FIXED_TIMESTEP_HZ` a balance parameter: halving
//! the tick rate would halve every drain. Systems multiply these by `delta_secs()`.

/// Energy an organism spends per second before adaptation and body upkeep.
pub const BASE_UPKEEP: f32 = 0.25;
/// How much the adaptation penalty costs, per second, per unit of penalty.
pub const PENALTY_UPKEEP: f32 = 1.0;
/// Health lost per second while starving.
pub const STARVATION_DAMAGE: f32 = 3.0;
/// Health regained per second while well fed (energy above [`WELL_FED_FRACTION`]).
pub const HEALTH_REGEN: f32 = 2.0;
/// Fraction of the energy cap above which the organism heals.
pub const WELL_FED_FRACTION: f32 = 0.5;

/// Base energy capacity, before storage vacuoles.
pub const BASE_ENERGY_CAP: f32 = 100.0;

/// Здоровье тела стартовой массы. Крупные тела крепче — см. [`crate::max_health`].
pub const MAX_HEALTH: f32 = 100.0;

/// Сколько здоровья добавляет каждая единица массы сверх стартовой.
///
/// Масса обязана давать власть, а не только замедлять. До этого рост был
/// сплошным проигрышем: тело становилось медленнее, прожорливее и крупнее как
/// мишень, а взамен не получало ничего, кроме доступа к дорогим органам. Игрок
/// рос и чувствовал себя слабее.
///
/// Теперь крупное тело **труднее убить**: втрое большая масса — вдвое больше
/// здоровья. Это то, что превращает набранный вес в преимущество, которое
/// видно в бою, а не только в таблице.
pub const HEALTH_PER_MASS: f32 = 3.4;

/// Сколько урона добавляет единица массы сверх стартовой.
///
/// Немного: масса не должна заменять оружие, иначе шипы и стрекала теряют
/// смысл. Но таранить собой кого-то мельче обязано работать — это самая
/// понятная форма превосходства, какая бывает.
pub const ATTACK_PER_MASS: f32 = 0.09;

/// Base swimming speed in units per second, before parts and mass.
pub const BASE_SPEED: f32 = 4.0;
/// How strongly mass slows the organism down: `speed / (1 + mass * this)`.
pub const MASS_DRAG: f32 = 0.035;

/// Seconds a dead organism waits before respawning.
pub const RESPAWN_DELAY: f32 = 5.0;
/// Fraction of mutation points kept through death.
pub const DEATH_POINT_RETENTION: f32 = 0.5;

/// Energy that must be absorbed to earn one mutation point.
pub const ENERGY_PER_MUTATION_POINT: f32 = 40.0;
/// Mutation points granted for surviving a season change.
pub const POINTS_PER_SEASON: u16 = 2;

/// Nutrients alive in the arena at once, at `food_density == 1.0`.
///
/// The arena is 140×140 now — four times the old area. 620 particles kept the
/// water from looking empty but quietly made points four times slower to earn,
/// because what matters for feeding is density, not the total count. 900 brings
/// the density back to roughly what it was on the small map.
pub const FOOD_TARGET: usize = 900;
/// Nutrients spawned per second while below the target.
pub const FOOD_SPAWN_RATE: f32 = 45.0;
/// Nutrients a corpse leaves behind.
pub const CORPSE_NUTRIENTS: usize = 6;

/// How much each part already in the body raises the price of the next one.
///
/// Рост цены линейный. Был линейный плюс квадратичный, и это и делало прокачку
/// резкой: к двадцатому органу цена вырастала всемеро, и развитие упиралось в
/// стену ровно тогда, когда игрок начинал понимать, что вообще хочет построить.
///
/// Смысл надбавки не в том, чтобы остановить рост, а в том, чтобы двадцатый
/// орган стоил дороже третьего — иначе поздняя игра сводится к «купи всё».
/// Линейного роста для этого достаточно.
pub const MUTATION_PRICE_LINEAR: f32 = 0.14;
pub const MUTATION_PRICE_QUADRATIC: f32 = 0.0;

/// Minimum seconds between two accepted mutation requests from one client.
pub const MUTATION_COOLDOWN: f32 = 0.25;
/// Default cap on parts in one genome. The server can lower it in its config;
/// this is also the hard ceiling the client draws against.
pub const MAX_PARTS: usize = 100;

/// Default cap for bots. They pay for growth exactly like a player now, so the
/// same ceiling applies; lower it in the config if a server needs the headroom.
pub const WILD_MAX_PARTS: usize = MAX_PARTS;
/// Hard cap on copies of a single part kind.
pub const MAX_PARTS_PER_KIND: usize = 6;

// --- combat -----------------------------------------------------------------

/// Damage per second a cell deals with its bare membrane, before spikes.
pub const BASE_ATTACK: f32 = 1.5;
/// Seconds after taking a hit during which health does not regenerate.
///
/// Without this a fed cell heals at 2/s while bare-membrane contact deals 1.5/s,
/// so two unarmed organisms could gnaw at each other forever. Being in a fight
/// has to stop the healing, or weapons are the only way anything ever dies.
pub const COMBAT_REGEN_BLOCK: f32 = 5.0;
/// Distance beyond the two bodies' radii at which contact damage applies.
pub const ATTACK_MARGIN: f32 = 0.25;
/// Fraction of the victim's mass the killer absorbs as energy.
pub const KILL_ENERGY_YIELD: f32 = 3.5;
/// Mutation points awarded for a kill.
pub const POINTS_PER_KILL: u16 = 3;

// --- reproduction -----------------------------------------------------------

/// Seconds between divisions for a cell with no reproductive parts.
pub const BASE_DIVISION_TIME: f32 = 26.0;
/// Fraction of the energy cap a cell must hold before it may divide.
pub const DIVISION_ENERGY_FRACTION: f32 = 0.7;
/// Fraction of its energy the parent hands to the offspring.
pub const DIVISION_ENERGY_SHARE: f32 = 0.45;
/// Base chance that an offspring is born with a random extra part.
pub const BASE_MUTATION_CHANCE: f32 = 0.25;
/// Cells of one lineage stop dividing past this many members.
pub const MAX_COLONY_SIZE: usize = 35;
/// Hard cap on organisms alive at once, players included.
pub const MAX_ORGANISMS: usize = 70;

// --- bots -------------------------------------------------------------------

/// Wild organisms the server keeps alive in the arena.
pub const WILD_TARGET: usize = 9;
/// Seconds between a bot's attempts to spend the points it has earned.
pub const WILD_MUTATION_INTERVAL: f32 = 6.0;
/// How far a bot looks for food or prey.
pub const BOT_VISION: f32 = 20.0;

// --- toxin clouds -----------------------------------------------------------

/// Насколько глубоко можно провалиться по кислороду, отделываясь одной энергией.
///
/// В пределах этого запаса нехватка кислорода стоит только расхода, как и любое
/// другое несоответствие среде. Глубже — начинается удушье.
pub const SUFFOCATION_SLACK: f32 = 0.20;

/// Урон здоровью в секунду за единицу нехватки кислорода сверх запаса.
///
/// Кислород — единственное давление среды, которое нельзя переесть, и
/// справочник это обещал игроку задолго до того, как это стало правдой. Всё
/// остальное — температура, солёность, фон яда — по-прежнему берёт энергией:
/// от них спасают органы, а не расторопность.
///
/// Числа подобраны так, чтобы шторм был экзаменом, а не казнью. При кислороде
/// 0.45 голое тело теряет около единицы здоровья в секунду — сытый организм
/// это переживает за счёт заживления, голодный нет. **Одной жабры хватает,
/// чтобы удушье исчезло полностью:** орган должен спасать жизнь, а не
/// экономить копейки.
pub const SUFFOCATION_DAMAGE: f32 = 6.0;

/// Урон здоровью в секунду за единицу яда сверх собственной стойкости.
///
/// До этого яд не наносил урона вообще: он поднимал расход энергии, а здоровье
/// падало только при нуле энергии. В мире, где в воде девятьсот частиц еды, это
/// значило «ешь чуть чаще» — сытый организм не замечал отравления.
///
/// Числами это было не починить: штраф адаптации зажат потолком, и в шторм яд
/// уже выедал 94 % этого потолка, так что вторая железа не добавляла почти
/// ничего. Поэтому яд бьёт по здоровью — по ресурсу, который не восполняется
/// подбором планктона.
///
/// Величина выбрана по бою: облако одной железы даёт превышение около 0.24, то
/// есть 1.4 урона в секунду — сравнимо с голой мембраной (1.5). Стоять в чужом
/// облаке становится примерно так же опасно, как стоять в чужих зубах.
pub const TOXIN_DAMAGE: f32 = 6.0;

/// Seconds between toxin releases from a cell that has a gland.
pub const TOXIN_INTERVAL: f32 = 9.0;
/// How long a cloud lingers.
pub const TOXIN_LIFETIME: f32 = 11.0;
pub const TOXIN_RADIUS: f32 = 3.2;
