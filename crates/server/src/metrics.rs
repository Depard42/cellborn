//! Сколько времени сервер тратит на один тик.
//!
//! Мерить приходится самому: `MinimalPlugins` не приносит диагностику Bevy, а
//! среднее по кадру всё равно не тот ответ. Важна не средняя загрузка, а запас
//! до края. Симуляция живёт в `FixedUpdate`, и как только тик перестаёт
//! укладываться в свои 15.6 мс, накопитель фиксированного времени отдаёт
//! следующему кадру два шага, они тоже не укладываются, следующему достаётся
//! ещё больше. Деградация здесь не плавная, поэтому в оверлей едет **пик**, а
//! не только среднее: провал в 40 мс раз в секунду среднее почти не двигает, а
//! в игре чувствуется.

use bevy::prelude::*;
use cellborn_common::ServerStats;
use std::time::Instant;

/// Как часто пересчитывается окно наблюдения (среднее, пик, темп тиков).
const WINDOW: f32 = 0.5;

#[derive(Resource)]
pub struct TickClock {
    started: Option<Instant>,
    /// Последний измеренный тик.
    pub last_ms: f32,
    /// Готовые числа за прошедшее окно — то, что видит игрок.
    pub avg_ms: f32,
    pub peak_ms: f32,
    pub ticks_per_second: f32,
    /// Накопители текущего окна.
    sum_ms: f32,
    window_peak_ms: f32,
    ticks: u32,
    elapsed: f32,
}

impl Default for TickClock {
    fn default() -> Self {
        Self {
            started: None,
            last_ms: 0.0,
            avg_ms: 0.0,
            peak_ms: 0.0,
            ticks_per_second: 0.0,
            sum_ms: 0.0,
            window_peak_ms: 0.0,
            ticks: 0,
            elapsed: 0.0,
        }
    }
}

/// Первая система тика: засекает время.
pub fn tick_begin(mut clock: ResMut<TickClock>) {
    clock.started = Some(Instant::now());
}

/// Последняя система тика: закрывает замер и, раз в окно, пересчитывает сводку.
pub fn tick_end(time: Res<Time>, mut clock: ResMut<TickClock>) {
    let Some(started) = clock.started.take() else { return };
    let ms = started.elapsed().as_secs_f32() * 1000.0;
    clock.last_ms = ms;
    clock.sum_ms += ms;
    clock.window_peak_ms = clock.window_peak_ms.max(ms);
    clock.ticks += 1;
    clock.elapsed += time.delta_secs();

    if clock.elapsed >= WINDOW {
        clock.avg_ms = clock.sum_ms / clock.ticks.max(1) as f32;
        clock.peak_ms = clock.window_peak_ms;
        clock.ticks_per_second = clock.ticks as f32 / clock.elapsed;
        clock.sum_ms = 0.0;
        clock.window_peak_ms = 0.0;
        clock.ticks = 0;
        clock.elapsed = 0.0;
    }
}

/// Снимок для отправки клиенту.
pub fn snapshot(
    clock: &TickClock,
    organisms: usize,
    food: usize,
    clouds: usize,
    clients: usize,
) -> ServerStats {
    ServerStats {
        tick_ms: clock.last_ms,
        tick_ms_avg: clock.avg_ms,
        tick_ms_peak: clock.peak_ms,
        ticks_per_second: clock.ticks_per_second,
        organisms: organisms.min(u16::MAX as usize) as u16,
        food: food.min(u16::MAX as usize) as u16,
        clouds: clouds.min(u16::MAX as usize) as u16,
        clients: clients.min(u16::MAX as usize) as u16,
    }
}
