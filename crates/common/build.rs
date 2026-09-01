//! Вшивает в бинарник, из чего он собран.
//!
//! Версия в игре нужна не для красоты: по ней обновлятор решает, есть ли смысл
//! качать новый релиз, а по коммиту и дате понятно, та ли это сборка, о которой
//! говорит игрок в баг-репорте. Номер версии берётся из `Cargo.toml`, остальное
//! — отсюда.
//!
//! Всё необязательно. Собрать проект без гита (из архива, например) должно быть
//! можно — тогда вместо хеша будет прочерк.

use std::process::Command;

fn main() {
    // Пересобираться при смене коммита. `.git/HEAD` меняется при переключении
    // ветки, а файл, на который он ссылается, — при коммите в неё.
    if let Some(git_dir) = find_git_dir() {
        println!("cargo:rerun-if-changed={}/HEAD", git_dir.display());
        if let Ok(head) = std::fs::read_to_string(git_dir.join("HEAD")) {
            if let Some(reference) = head.strip_prefix("ref: ").map(str::trim) {
                println!("cargo:rerun-if-changed={}/{reference}", git_dir.display());
            }
        }
    }

    // В CI гит есть, но сборка может идти из detached HEAD, поэтому короткий
    // хеш берётся у самого гита, а не из имени ветки.
    let commit = run("git", &["rev-parse", "--short=7", "HEAD"]).unwrap_or_else(|| "-".into());
    // Грязное дерево помечаем: сборка из незакоммиченного кода не воспроизводима,
    // и в баг-репорте это важнее, чем аккуратность строки.
    let dirty = run("git", &["status", "--porcelain"]).is_some_and(|out| !out.trim().is_empty());
    let commit = if dirty { format!("{commit}+") } else { commit };

    println!("cargo:rustc-env=CELLBORN_BUILD_DATE={}", build_date());

    println!("cargo:rustc-env=CELLBORN_COMMIT={commit}");
}

/// Сегодняшняя дата в UTC, без внешних программ и без зависимостей.
///
/// Раньше здесь звался `date -u`, но сборка идёт и на Windows-раннере, где его
/// нет, и дата молча превращалась в прочерк — ровно в той сборке, которая
/// уезжает игрокам.
///
/// Уважает `SOURCE_DATE_EPOCH`: с ним сборка становится воспроизводимой.
fn build_date() -> String {
    let seconds = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs())
        });
    let Some(seconds) = seconds else { return "-".into() };

    let (year, month, day) = civil_from_days((seconds / 86_400) as i64);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Дни от эпохи Unix — в календарную дату. Алгоритм Ховарда Хиннанта: он
/// короткий, целочисленный и не врёт на високосных годах.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    // Считаем от 0000-03-01: в такой системе високосный день оказывается в
    // конце года и перестаёт быть особым случаем.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = (z - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

fn run(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Ищет `.git` вверх по дереву: build.rs запускается из каталога крейта, а
/// репозиторий начинается двумя уровнями выше.
fn find_git_dir() -> Option<std::path::PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(".git");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}
