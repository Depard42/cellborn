//! Опасности, которые не являются живыми организмами.
//!
//! Колючки стоят на месте и наказывают за размер. Левиафаны проплывают мимо и
//! наказывают за невнимательность. Общее у них одно: это не участники игры, а
//! свойства моря, и вести себя как организмы им не нужно — ни есть, ни
//! размножаться, ни драться по правилам родства.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────
// Колючки
// ─────────────────────────────────────────────

/// Сколько колючек держать в море.
pub const THORN_COUNT: usize = 14;

/// Радиус самой колючки.
pub const THORN_RADIUS: f32 = 2.2;

/// Радиус тела, начиная с которого колючка становится опасной.
///
/// Мелкие проплывают насквозь и прячутся внутри — это и есть смысл механики:
/// у слабых появляется место, куда сильный не может за ними последовать.
/// Порог выбран так, чтобы стартовое тело пролезало с запасом, а выросшее
/// вдвое — уже нет.
pub const THORN_SAFE_RADIUS: f32 = 1.15;

/// Урон в секунду тому, кто слишком велик, чтобы пройти насквозь.
pub const THORN_DAMAGE: f32 = 9.0;

/// Неподвижная колючка: убежище для мелких, стена для крупных.
#[derive(Component, Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Thorn {
    pub position: Vec3,
    pub radius: f32,
}

impl Thorn {
    /// Достаёт ли колючка до тела такого размера в такой точке.
    pub fn touches(&self, point: Vec3, body_radius: f32) -> bool {
        let reach = self.radius + body_radius;
        self.position.distance_squared(point) < reach * reach
    }

    /// Опасна ли колючка телу такого размера.
    ///
    /// Ровно один порог, без промежуточных состояний: игрок должен уметь
    /// посмотреть на себя и сказать «мне туда можно» или «мне туда нельзя».
    pub fn hurts(body_radius: f32) -> bool {
        body_radius > THORN_SAFE_RADIUS
    }
}

// ─────────────────────────────────────────────
// Левиафаны
// ─────────────────────────────────────────────

/// Радиус туши.
pub const LEVIATHAN_RADIUS: f32 = 6.5;

/// Скорость: быстрее любой клетки, но не настолько, чтобы нельзя было уйти
/// с дороги, заметив вовремя.
pub const LEVIATHAN_SPEED: f32 = 7.5;

/// Урон в секунду тому, кого он задел.
pub const LEVIATHAN_DAMAGE: f32 = 45.0;

/// Средний промежуток между заплывами, секунд.
pub const LEVIATHAN_INTERVAL: f32 = 95.0;

/// Огромное существо, проплывающее через море насквозь.
///
/// Приходит извне и уходит вдаль: не житель арены, а событие. С ним нельзя
/// драться и его нельзя съесть — можно только не оказаться на пути.
#[derive(Component, Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Leviathan {
    pub position: Vec3,
    /// Единичный вектор курса. Курс прямой: чудовище не охотится, оно плывёт
    /// мимо, и вся игра с ним — это заметить и уйти в сторону.
    pub heading: Vec3,
    pub radius: f32,
}

impl Leviathan {
    pub fn touches(&self, point: Vec3, body_radius: f32) -> bool {
        let reach = self.radius + body_radius;
        self.position.distance_squared(point) < reach * reach
    }
}

// ─────────────────────────────────────────────
// Лакомые места
// ─────────────────────────────────────────────

/// Сколько лакомых мест держать одновременно.
pub const FEAST_COUNT: usize = 3;

/// Радиус, в котором лакомое место сыплет еду.
pub const FEAST_RADIUS: f32 = 9.0;

/// Сколько живёт одно лакомое место, секунд.
pub const FEAST_LIFETIME: f32 = 55.0;

/// Какая доля еды сыплется в лакомые места, а не по всей арене.
///
/// Не вся: если бы вне пятен еды не было совсем, игра свелась бы к трём точкам
/// на карте, и всё остальное море стало бы мёртвым коридором.
pub const FEAST_SHARE: f32 = 0.62;

/// Место, где сейчас особенно много еды.
///
/// Смысл в том, чтобы у моря была причина куда-то плыть. Равномерно рассыпанная
/// еда никуда не зовёт: где стоишь, там и корм. Пятно — это повод рискнуть,
/// потому что туда же приплывут и остальные.
#[derive(Component, Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Feast {
    pub position: Vec3,
    pub radius: f32,
    /// Сколько ему осталось, в долях от полной жизни: клиент рисует по этому
    /// числу яркость, чтобы затухающее пятно было видно заранее.
    pub strength: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Главное свойство колючки: мелкий прячется, крупный не проходит.
    #[test]
    fn a_thorn_is_shelter_for_the_small_and_a_wall_for_the_big() {
        use crate::{body_radius, BASE_MASS};

        // Стартовое тело обязано пролезать: иначе укрытие бесполезно тем, ради
        // кого оно существует.
        let newborn = body_radius(BASE_MASS);
        assert!(!Thorn::hurts(newborn), "новорождённый не помещается в укрытие");

        // Выросшее вдвое по массе — уже нет.
        let grown = body_radius(BASE_MASS * 4.0);
        assert!(Thorn::hurts(grown), "выросшее тело проходит сквозь колючку");
        assert!(grown > newborn);
    }

    #[test]
    fn hazards_reach_exactly_as_far_as_their_radius() {
        let thorn = Thorn { position: Vec3::ZERO, radius: 2.0 };
        assert!(thorn.touches(Vec3::new(2.5, 0.0, 0.0), 1.0));
        assert!(!thorn.touches(Vec3::new(3.5, 0.0, 0.0), 1.0));

        let beast = Leviathan { position: Vec3::ZERO, heading: Vec3::X, radius: 6.0 };
        assert!(beast.touches(Vec3::new(6.5, 0.0, 0.0), 1.0));
        assert!(!beast.touches(Vec3::new(8.0, 0.0, 0.0), 1.0));
    }
}
