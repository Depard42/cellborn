//! Из чего собран этот бинарник.
//!
//! Одна версия на всех: и клиент, и сервер берут её отсюда, поэтому они не могут
//! разойтись. Значения вшиваются в `build.rs` при сборке.

/// Версия из `Cargo.toml`. Она же — то, с чем обновлятор сравнивает тег релиза.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Короткий хеш коммита; `+` на конце — сборка из незакоммиченного дерева.
/// Прочерк, если гита при сборке не было.
pub const COMMIT: &str = env!("CELLBORN_COMMIT");

/// Дата сборки, UTC.
pub const BUILD_DATE: &str = env!("CELLBORN_BUILD_DATE");

/// Полная подпись для интерфейса и логов: `0.1.0 (82403ac, 2026-09-01)`.
pub fn full() -> String {
    format!("{VERSION} ({COMMIT}, {BUILD_DATE})")
}

/// Короткая подпись: `v0.1.0`.
pub fn short() -> String {
    format!("v{VERSION}")
}

/// Сравнивает версии по номерам, а не по строке.
///
/// Строковое сравнение здесь врёт: `"0.10.0" < "0.9.0"` как строки, но не как
/// версии, и игрок с 0.10.0 получил бы предложение «обновиться» до 0.9.0.
/// Ведущая `v` в теге релиза отбрасывается, хвост после номера (`-rc1`,
/// `+build`) игнорируется — предрелизы для этой проверки просто не существуют.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    numbers(candidate) > numbers(current)
}

fn numbers(version: &str) -> [u32; 3] {
    let cleaned = version.trim().trim_start_matches(['v', 'V']);
    // Отрезаем всё, что после номера: `1.2.3-rc1` — это 1.2.3.
    let head = cleaned.split(['-', '+']).next().unwrap_or("");
    let mut parts = head.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    [
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_by_number_not_by_string() {
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        // Ровно тот случай, из-за которого сравнение не строковое.
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(!is_newer("0.9.0", "0.10.0"));
        assert!(!is_newer("0.1.0", "0.1.0"), "та же версия — не новее");
        assert!(!is_newer("0.1.0", "0.1.1"), "старее — не новее");
    }

    #[test]
    fn tolerates_the_shapes_tags_actually_come_in() {
        assert!(is_newer("v0.2.0", "0.1.0"), "тег с ведущей v");
        assert!(is_newer("0.2", "0.1.9"), "две цифры вместо трёх");
        assert!(!is_newer("0.1.0-rc1", "0.1.0"), "предрелиз не новее релиза");
        assert!(is_newer("0.2.0-rc1", "0.1.0"), "но номер всё равно читается");
        // Мусор не должен предлагать «обновиться» вниз.
        assert!(!is_newer("не-версия", "0.1.0"));
    }
}
