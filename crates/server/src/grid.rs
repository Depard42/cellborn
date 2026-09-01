//! Пространственная сетка еды.
//!
//! Еда не двигается: она появляется и исчезает. Значит, её раскладку можно
//! построить один раз за тик и дать всем, кому она нужна, вместо того чтобы
//! каждый строил свою.
//!
//! До этого за один тик происходило две вещи. `feeding` собирал `HashMap`
//! клетка → частицы, пользовался им и выбрасывал, чтобы через 15 мс собрать
//! заново — с новой аллокацией `Vec` на каждую занятую клетку. А `bot_movement`
//! копировал позиции всех девятисот частиц в отдельный вектор и каждый бот
//! проходил его целиком, ища ближайшую.
//!
//! Здесь и то и другое: сетка живёт ресурсом между тиками, переиспользует
//! память клеток (`Vec::clear` не отдаёт ёмкость), а поиск ближайшей частицы
//! идёт кольцами от центра и останавливается, как только следующее кольцо уже
//! не может содержать ничего ближе найденного.

use bevy::prelude::*;
use cellborn_common::ARENA_HALF_EXTENT;

/// Сторона клетки. Порядка дальности захвата пищи: слишком мелкая клетка — и
/// поиск обходит много пустых, слишком крупная — и в клетке снова линейный
/// перебор.
pub const CELL: f32 = 4.0;

/// Сколько клеток по одной оси. Арена шире собственных границ на клетку с
/// каждой стороны: тела прижимаются к стенке, и запас избавляет от особого
/// случая на краю.
const SIDE: usize = (2.0 * ARENA_HALF_EXTENT / CELL) as usize + 3;

/// Одна частица еды в сетке.
#[derive(Clone, Copy)]
pub struct FoodEntry {
    pub entity: Entity,
    pub position: Vec3,
    pub energy: f32,
    /// Съедена в этом тике. Флаг вместо поиска по списку съеденного: раньше
    /// `feeding` проверяла `eaten.contains(entity)` линейным проходом.
    pub taken: bool,
}

#[derive(Resource)]
pub struct FoodGrid {
    cells: Vec<Vec<FoodEntry>>,
    /// Сколько частиц лежит в сетке — чтобы не пересчитывать запросом.
    count: usize,
}

impl Default for FoodGrid {
    fn default() -> Self {
        Self { cells: vec![Vec::new(); SIDE * SIDE], count: 0 }
    }
}

impl FoodGrid {
    fn index(position: Vec3) -> usize {
        let x = ((position.x + ARENA_HALF_EXTENT) / CELL) as i32 + 1;
        let z = ((position.z + ARENA_HALF_EXTENT) / CELL) as i32 + 1;
        let x = x.clamp(0, SIDE as i32 - 1) as usize;
        let z = z.clamp(0, SIDE as i32 - 1) as usize;
        z * SIDE + x
    }

    fn cell_of(position: Vec3) -> (i32, i32) {
        let x = ((position.x + ARENA_HALF_EXTENT) / CELL) as i32 + 1;
        let z = ((position.z + ARENA_HALF_EXTENT) / CELL) as i32 + 1;
        (x.clamp(0, SIDE as i32 - 1), z.clamp(0, SIDE as i32 - 1))
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Пересобирает сетку под текущую еду, не отдавая память клеток.
    pub fn rebuild(&mut self, food: impl Iterator<Item = (Entity, Vec3, f32)>) {
        for cell in &mut self.cells {
            cell.clear();
        }
        self.count = 0;
        for (entity, position, energy) in food {
            self.cells[Self::index(position)].push(FoodEntry {
                entity,
                position,
                energy,
                taken: false,
            });
            self.count += 1;
        }
    }

    /// Все ещё не съеденные частицы в радиусе от точки, с возможностью пометить
    /// частицу съеденной прямо в обходе.
    pub fn for_each_near(&mut self, center: Vec3, radius: f32, mut visit: impl FnMut(&mut FoodEntry)) {
        if radius <= 0.0 {
            return;
        }
        let (cx, cz) = Self::cell_of(center);
        let span = (radius / CELL).ceil() as i32;
        let radius_squared = radius * radius;
        for dz in -span..=span {
            let z = cz + dz;
            if z < 0 || z >= SIDE as i32 {
                continue;
            }
            for dx in -span..=span {
                let x = cx + dx;
                if x < 0 || x >= SIDE as i32 {
                    continue;
                }
                for entry in &mut self.cells[z as usize * SIDE + x as usize] {
                    if entry.taken {
                        continue;
                    }
                    if center.distance_squared(entry.position) > radius_squared {
                        continue;
                    }
                    visit(entry);
                }
            }
        }
    }

    /// Ближайшая частица в пределах радиуса, или `None`.
    ///
    /// Идёт кольцами от клетки, в которой стоит организм, и обрывается, как
    /// только найденное ближе, чем ближайший край следующего кольца: обычно это
    /// одно-два кольца вместо прохода по всей арене.
    pub fn nearest(&self, center: Vec3, radius: f32) -> Option<Vec3> {
        let (cx, cz) = Self::cell_of(center);
        let max_ring = (radius / CELL).ceil() as i32;
        let mut best: Option<(f32, Vec3)> = None;

        for ring in 0..=max_ring {
            // Кольцо шириной в клетку не может дать ничего ближе, чем его
            // внутренний край: если найденное уже ближе — дальше смотреть нечего.
            if let Some((distance_squared, _)) = best {
                let inner_edge = (ring - 1).max(0) as f32 * CELL;
                if distance_squared <= inner_edge * inner_edge {
                    break;
                }
            }
            for dz in -ring..=ring {
                for dx in -ring..=ring {
                    // Только внешняя рамка: внутренность уже просмотрена.
                    if ring > 0 && dx.abs() != ring && dz.abs() != ring {
                        continue;
                    }
                    let (x, z) = (cx + dx, cz + dz);
                    if x < 0 || x >= SIDE as i32 || z < 0 || z >= SIDE as i32 {
                        continue;
                    }
                    for entry in &self.cells[z as usize * SIDE + x as usize] {
                        if entry.taken {
                            continue;
                        }
                        let distance_squared = center.distance_squared(entry.position);
                        if distance_squared > radius * radius {
                            continue;
                        }
                        if best.is_none_or(|(best_distance, _)| distance_squared < best_distance) {
                            best = Some((distance_squared, entry.position));
                        }
                    }
                }
            }
        }
        best.map(|(_, position)| position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid_of(points: &[Vec3]) -> FoodGrid {
        let mut grid = FoodGrid::default();
        grid.rebuild(
            points.iter().enumerate().map(|(i, p)| (Entity::from_raw_u32(i as u32 + 1).unwrap(), *p, 9.0)),
        );
        grid
    }

    /// Сетка обязана давать тот же ответ, что и честный перебор, — иначе она
    /// не ускорение, а другая игра.
    #[test]
    fn nearest_matches_a_linear_scan() {
        let mut points = Vec::new();
        let mut seed = 1u64;
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) % 100_000) as f32 / 100_000.0
        };
        for _ in 0..400 {
            let edge = ARENA_HALF_EXTENT;
            points.push(Vec3::new(next() * 2.0 * edge - edge, 0.0, next() * 2.0 * edge - edge));
        }
        let grid = grid_of(&points);

        for _ in 0..200 {
            let edge = ARENA_HALF_EXTENT;
            let here = Vec3::new(next() * 2.0 * edge - edge, 0.0, next() * 2.0 * edge - edge);
            for radius in [2.0, 8.0, 20.0] {
                let honest = points
                    .iter()
                    .map(|p| (here.distance_squared(*p), *p))
                    .filter(|(d, _)| *d <= radius * radius)
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, p)| p);
                let fast = grid.nearest(here, radius);
                match (honest, fast) {
                    (None, None) => {}
                    (Some(a), Some(b)) => assert!(
                        (here.distance(a) - here.distance(b)).abs() < 1e-4,
                        "сетка нашла не ближайшую: {a:?} против {b:?}"
                    ),
                    (a, b) => panic!("сетка разошлась с перебором: {a:?} против {b:?}"),
                }
            }
        }
    }

    /// Частица на самом краю арены должна попадать в сетку, а не теряться.
    #[test]
    fn edges_are_inside_the_grid() {
        let edge = ARENA_HALF_EXTENT;
        let corners = [
            Vec3::new(-edge, 0.0, -edge),
            Vec3::new(edge, 0.0, edge),
            Vec3::new(edge, 1.2, -edge),
            Vec3::new(0.0, -0.6, 0.0),
        ];
        let grid = grid_of(&corners);
        assert_eq!(grid.len(), corners.len());
        for corner in corners {
            assert_eq!(grid.nearest(corner, 0.5), Some(corner), "потеряна частица {corner:?}");
        }
    }

    /// Съеденное в этом тике больше никому не достаётся.
    #[test]
    fn taken_food_is_skipped() {
        let point = Vec3::new(3.0, 0.0, 3.0);
        let mut grid = grid_of(&[point]);
        let mut seen = 0;
        grid.for_each_near(point, 2.0, |entry| {
            entry.taken = true;
            seen += 1;
        });
        assert_eq!(seen, 1);
        grid.for_each_near(point, 2.0, |_| seen += 1);
        assert_eq!(seen, 1, "съеденную частицу выдали второй раз");
        assert_eq!(grid.nearest(point, 5.0), None, "боты всё ещё видят съеденное");
    }
}
