//! Загрязнение воды.
//!
//! Организмы гадят там, где живут. Чем больше тело и чем оно активнее, тем
//! быстрее портит воду вокруг себя; вода при этом сама себя очищает, но
//! медленнее, чем её пачкает толпа.
//!
//! **Зачем.** Без этого центр арены — лучшее место в мире: еда сыплется
//! равномерно, а в середине ещё и все соседи, которых можно съесть. Скопление
//! ничем не наказывалось, и игра сводилась к куче-мале в центре. Теперь у
//! скопления есть цена, и она растёт быстрее, чем выгода: место, где стоят
//! тридцать организмов, становится опасным даже для того, кто сильнее всех.
//!
//! **Почему сетка, а не облака.** Отдельная сущность-облако на каждого
//! организма — это семьдесят реплицируемых сущностей, меняющихся каждый тик,
//! ровно то, от чего мы избавлялись в `docs/PERFORMANCE.md`. Сетка — это один
//! компонент на одной сущности: несколько сотен байт, которые едут пару раз в
//! секунду.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::ARENA_HALF_EXTENT;

/// Сторона клетки загрязнения.
///
/// Крупнее, чем сетка еды: грязь — это не частица, у неё нет точного места, и
/// мельчить здесь значит гонять по сети вчетверо больше чисел ради разрешения,
/// которого никто не увидит.
pub const POLLUTION_CELL: f32 = 7.0;

/// Клеток по одной оси.
pub const POLLUTION_SIDE: usize = (2.0 * ARENA_HALF_EXTENT / POLLUTION_CELL) as usize + 1;

/// Во что превращается полностью забитая клетка, в единицах яда.
///
/// Примерно как одно облако от железы: плотная толпа портит воду не хуже, чем
/// организм, специально отрастивший для этого орган.
///
/// Стояло 0.40, и это оказалось слишком: при полном мире грязь давала около
/// двух единиц урона в секунду, тела умирали раньше, чем успевали отрастить
/// хоть что-нибудь, и вместо «частей до 27» перепись показывала «до 9».
/// Скопление должно мешать жить, а не обнулять развитие: при 0.25 забитая
/// клетка снимает чуть больше единицы здоровья в секунду — уйти нужно, но
/// время на это есть.
pub const POLLUTION_MAX_TOXIN: f32 = 0.30;

/// Сколько грязи в секунду даёт единица содержания тела.
///
/// Считается от расхода на органы, а не от массы: гадит обмен веществ. Крупное
/// тело с дешёвыми органами пачкает воду меньше, чем мелкое, но прожорливое.
///
/// Величина не подобрана, а решена из цели. Клетка теряет
/// `POLLUTION_DECAY + POLLUTION_SPREAD` своего уровня в секунду, поэтому
/// равновесный уровень при `N` телах равен `упор × K × N / потери`. Нужно было:
///
/// * одиночка — ниже базовой стойкости, то есть ровно ноль урона;
/// * десяток — заметно, но переживаемо;
/// * толпа — быстрее, чем успевает заживать.
///
/// Отсюда `K = 0.18`: втроём стоять можно сколько угодно, вдесятером клетка
/// набирает около 0.7 и начинает есть по единице здоровья в секунду, два
/// десятка забивают её до предела.
///
/// Первая рабочая версия стояла на 0.10 при потолке 0.25, и этого оказалось
/// мало: скопление было заметно в числах, но не в игре — толпа продолжала
/// стоять, потому что теряла меньше, чем выигрывала на еде.
///
/// Первая версия стояла на 0.55, и это был не баланс, а мор: одно тело травило
/// само себя, три — убивали друг друга, за десять минут население падало вдвое.
/// Проверять такое надо счётом, а не на глаз, — см. `examples/balance.rs`.
pub const POLLUTION_PER_UPKEEP: f32 = 0.18;

/// Какая доля загрязнения уходит за секунду. Вода очищается сама, но не быстро:
/// иначе толпа успевала бы разойтись и вернуться на чистое место.
pub const POLLUTION_DECAY: f32 = 0.11;

/// Какая доля клетки за секунду растекается по соседям.
///
/// Без растекания толпа рисует на карте резкий квадрат, а край клетки
/// становится безопасной линией, вдоль которой можно стоять.
pub const POLLUTION_SPREAD: f32 = 0.35;

/// Загрязнение воды по клеткам арены, в долях от [`POLLUTION_MAX_TOXIN`].
///
/// Байт на клетку: разрешение в 1/255 от максимума — мельче, чем игрок способен
/// заметить, а по сети это несколько сотен байт вместо нескольких килобайт.
#[derive(Component, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pollution {
    pub cells: Vec<u8>,
}

impl Default for Pollution {
    fn default() -> Self {
        Self { cells: vec![0; POLLUTION_SIDE * POLLUTION_SIDE] }
    }
}

impl Pollution {
    /// Клетка, в которой находится точка. Края арены прижимаются внутрь.
    pub fn cell_of(position: Vec3) -> (usize, usize) {
        let x = ((position.x + ARENA_HALF_EXTENT) / POLLUTION_CELL) as i32;
        let z = ((position.z + ARENA_HALF_EXTENT) / POLLUTION_CELL) as i32;
        (
            x.clamp(0, POLLUTION_SIDE as i32 - 1) as usize,
            z.clamp(0, POLLUTION_SIDE as i32 - 1) as usize,
        )
    }

    pub fn index(position: Vec3) -> usize {
        let (x, z) = Self::cell_of(position);
        z * POLLUTION_SIDE + x
    }

    /// Сколько яда добавляет вода в этой точке.
    pub fn toxin_at(&self, position: Vec3) -> f32 {
        let level = self.cells.get(Self::index(position)).copied().unwrap_or(0);
        level as f32 / 255.0 * POLLUTION_MAX_TOXIN
    }

    /// Доля заполнения клетки, 0..1 — для отрисовки.
    pub fn level_at(&self, position: Vec3) -> f32 {
        self.cells.get(Self::index(position)).copied().unwrap_or(0) as f32 / 255.0
    }

    /// Самая грязная клетка — для переписи и оверлея.
    pub fn worst(&self) -> f32 {
        self.cells.iter().copied().max().unwrap_or(0) as f32 / 255.0
    }
}

/// Серверная сторона: те же клетки, но в дробных числах.
///
/// Байты годятся для пересылки, но не для накопления: прибавка за тик меньше
/// одной двухсотпятидесятипятой, и в целых числах она бы просто терялась —
/// загрязнение не росло бы вообще.
#[derive(Resource, Debug, Clone)]
pub struct PollutionField {
    pub levels: Vec<f32>,
    /// Буфер для растекания, чтобы не выделять память каждый тик.
    scratch: Vec<f32>,
}

impl Default for PollutionField {
    fn default() -> Self {
        let size = POLLUTION_SIDE * POLLUTION_SIDE;
        Self { levels: vec![0.0; size], scratch: vec![0.0; size] }
    }
}

impl PollutionField {
    /// Добавляет грязь в точке.
    pub fn add(&mut self, position: Vec3, amount: f32) {
        let index = Pollution::index(position);
        self.levels[index] = (self.levels[index] + amount).min(1.0);
    }

    pub fn at(&self, position: Vec3) -> f32 {
        self.levels[Pollution::index(position)]
    }

    /// Самая грязная клетка поля, 0..1 — для переписи и отладки.
    pub fn worst(&self) -> f32 {
        self.levels.iter().copied().fold(0.0, f32::max)
    }

    /// Очищение и растекание за прошедшее время.
    pub fn settle(&mut self, dt: f32) {
        let keep = (1.0 - POLLUTION_DECAY * dt).clamp(0.0, 1.0);
        let spread = (POLLUTION_SPREAD * dt).clamp(0.0, 0.9);

        // Растекание считается по снимку: иначе клетка отдавала бы соседу то,
        // что только что получила от другого, и грязь ползла бы по карте в одну
        // сторону — в порядке обхода.
        self.scratch.copy_from_slice(&self.levels);
        for z in 0..POLLUTION_SIDE {
            for x in 0..POLLUTION_SIDE {
                let index = z * POLLUTION_SIDE + x;
                let here = self.scratch[index];
                if here <= 0.0 {
                    continue;
                }
                let mut neighbours = [usize::MAX; 4];
                let mut count = 0;
                if x > 0 {
                    neighbours[count] = index - 1;
                    count += 1;
                }
                if x + 1 < POLLUTION_SIDE {
                    neighbours[count] = index + 1;
                    count += 1;
                }
                if z > 0 {
                    neighbours[count] = index - POLLUTION_SIDE;
                    count += 1;
                }
                if z + 1 < POLLUTION_SIDE {
                    neighbours[count] = index + POLLUTION_SIDE;
                    count += 1;
                }
                if count == 0 {
                    continue;
                }
                let given = here * spread;
                self.levels[index] -= given;
                let share = given / count as f32;
                for neighbour in &neighbours[..count] {
                    self.levels[*neighbour] += share;
                }
            }
        }

        for level in &mut self.levels {
            *level = (*level * keep).clamp(0.0, 1.0);
        }
    }

    /// Упаковывает поле в то, что уезжает клиентам.
    pub fn quantise(&self, into: &mut Pollution) -> bool {
        let mut changed = false;
        for (cell, level) in into.cells.iter_mut().zip(&self.levels) {
            let byte = (level * 255.0).round().clamp(0.0, 255.0) as u8;
            // Порог в одно деление: без него ползающая в последнем разряде
            // грязь помечала бы компонент изменённым каждый тик.
            if cell.abs_diff(byte) > 0 {
                *cell = byte;
                changed = true;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Сетка обязана покрывать арену целиком, включая углы.
    #[test]
    fn every_corner_lands_inside_the_grid() {
        let edge = ARENA_HALF_EXTENT;
        for corner in [
            Vec3::new(-edge, 0.0, -edge),
            Vec3::new(edge, 0.0, edge),
            Vec3::new(edge, 0.0, -edge),
            Vec3::new(-edge, 0.0, edge),
            // За пределами арены тоже: тела прижимаются к стенке, и округление
            // может вынести точку за край.
            Vec3::new(-edge - 5.0, 0.0, edge + 5.0),
        ] {
            let index = Pollution::index(corner);
            assert!(index < POLLUTION_SIDE * POLLUTION_SIDE, "{corner:?} вне сетки");
        }
    }

    /// Грязь копится там, где стоят, и рассасывается, когда ушли.
    #[test]
    fn dirt_accumulates_and_then_clears() {
        let mut field = PollutionField::default();
        let spot = Vec3::new(10.0, 0.0, -20.0);

        for _ in 0..200 {
            field.add(spot, 0.01);
            field.settle(1.0 / 64.0);
        }
        let dirty = field.at(spot);
        assert!(dirty > 0.05, "толпа не запачкала воду: {dirty}");

        // Ушли — вода очищается.
        for _ in 0..(64 * 60) {
            field.settle(1.0 / 64.0);
        }
        assert!(field.at(spot) < dirty * 0.1, "вода не очистилась за минуту");
    }

    /// Растекание не должно создавать грязь из ничего и не должно её терять
    /// быстрее, чем задумано очищением.
    #[test]
    fn spreading_conserves_dirt() {
        let mut field = PollutionField::default();
        field.add(Vec3::ZERO, 1.0);
        let before: f32 = field.levels.iter().sum();

        // Растекание без очищения: keep = 1 при нулевом распаде.
        let mut clean = PollutionField { levels: field.levels.clone(), scratch: field.scratch.clone() };
        let keep_all = 1.0 - POLLUTION_DECAY * 0.0;
        assert_eq!(keep_all, 1.0);
        clean.settle(0.0);
        let after: f32 = clean.levels.iter().sum();
        assert!((before - after).abs() < 1e-4, "растекание изменило количество грязи");
    }

    /// Байты для сети не должны терять сам факт загрязнения.
    #[test]
    fn quantising_keeps_what_matters() {
        let mut field = PollutionField::default();
        let spot = Vec3::new(-30.0, 0.0, 30.0);
        field.add(spot, 0.5);

        let mut packed = Pollution::default();
        assert!(field.quantise(&mut packed), "изменение не замечено");
        assert!((packed.level_at(spot) - 0.5).abs() < 0.01, "уровень потерялся при упаковке");
        assert!(packed.toxin_at(spot) > 0.0);
        // Второй раз без изменений — писать нечего.
        assert!(!field.quantise(&mut packed), "поле не менялось, а упаковка сообщила об изменении");
    }
}
