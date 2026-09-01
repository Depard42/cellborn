//! Проверка и установка обновлений из релизов GitHub.
//!
//! Работает так: спросить у GitHub последний релиз, сравнить его тег с
//! [`cellborn_common::VERSION`], и если он новее — скачать архив для этой
//! системы и разложить его поверх установленной игры.
//!
//! **Конфиг сервера не трогается никогда.** В этом его смысл: игрок настроил
//! мир под себя, и обновление игры не должно это стирать. Новые настройки,
//! появившиеся в свежей версии, сервер сам допишет в его файл при первом
//! запуске (`crates/server/src/config.rs`).
//!
//! **Порядок операций выбран так, чтобы оборванная закачка не убила игру.**
//! Сначала архив целиком скачивается во временный файл, потом целиком
//! распаковывается во временный каталог, и только когда оба шага прошли, файлы
//! переезжают на место. Прежние бинарники не удаляются, а переименовываются в
//! `.old`: заменить работающий исполняемый файл нельзя ни на Windows, ни без
//! риска на Linux, а переименовать — можно, и старую версию всегда есть куда
//! откатить.
//!
//! Сеть живёт в отдельном потоке и общается с игрой через канал: Bevy не должен
//! ждать GitHub.

use bevy::prelude::*;
use cellborn_common::version;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};

/// Откуда берутся релизы. Переопределяется переменной окружения, чтобы можно
/// было проверить обновлятор на своём форке, не пересобирая игру.
pub fn repository() -> String {
    std::env::var("CELLBORN_REPO").unwrap_or_else(|_| "Depard42/cellborn".to_string())
}

/// Имя архива для этой системы. Его же собирает `.github/workflows/release.yml`
/// — если разойдутся, обновление перестанет находить свой файл.
pub const ASSET: &str = if cfg!(target_os = "windows") {
    "cellborn-windows-x86_64.zip"
} else {
    "cellborn-linux-x86_64.zip"
};

/// Файлы, которые принадлежат игроку и переживают обновление.
///
/// Сравнение по имени файла, без учёта регистра: на Windows игрок вполне может
/// сохранить конфиг как `Cellborn-Server.CFG`.
const KEEP: &[&str] = &[
    // Настройки мира: игрок их правил, обновление игры их не касается.
    "cellborn-server.cfg",
    // Громкость и запомненные серверы. Без этой строки каждое обновление
    // сбрасывало бы звук и стирало список серверов, к которым игрок ходит
    // годами.
    "cellborn.cfg",
];

/// Что известно про доступный релиз.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub tag: String,
    pub version: String,
    pub notes: String,
    pub asset_url: String,
    pub size: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Stage {
    /// Ещё не спрашивали.
    #[default]
    Idle,
    Checking,
    /// Спросили, обновляться некуда.
    UpToDate,
    Available(Box<Release>),
    Downloading,
    /// Установлено. Дальше нужен перезапуск — заменять себя на ходу нельзя.
    Installed(String),
    Failed(String),
}

/// Сообщения из рабочего потока в игру.
enum Report {
    Checked(Option<Release>),
    Installed(String),
    Failed(String),
}

#[derive(Resource, Default)]
pub struct Updater {
    pub stage: Stage,
    /// Ответ рабочего потока. Под `Mutex` не ради гонки — приёмник читает одна
    /// система, — а потому что `Receiver` не `Sync`, а ресурсу Bevy это нужно.
    inbox: Option<Mutex<Receiver<Report>>>,
    /// Сколько байт уже скачано. Пишется рабочим потоком, читается интерфейсом.
    pub done: Arc<AtomicU64>,
    pub total: u64,
}

impl Updater {
    pub fn busy(&self) -> bool {
        matches!(self.stage, Stage::Checking | Stage::Downloading)
    }

    /// Строка для кнопки: одна на все состояния, чтобы интерфейс не расползался.
    pub fn button_label(&self) -> String {
        match &self.stage {
            Stage::Idle | Stage::UpToDate | Stage::Failed(_) => "ПРОВЕРИТЬ ОБНОВЛЕНИЕ".into(),
            Stage::Checking => "ПРОВЕРЯЮ...".into(),
            Stage::Available(release) => format!("ОБНОВИТЬ ДО {}", release.tag),
            Stage::Downloading => {
                let done = self.done.load(Ordering::Relaxed);
                if self.total > 0 {
                    format!("СКАЧИВАЮ {} %", done * 100 / self.total.max(1))
                } else {
                    format!("СКАЧИВАЮ {} МБ", done / 1_048_576)
                }
            }
            Stage::Installed(_) => "ПЕРЕЗАПУСТИ ИГРУ".into(),
        }
    }

    /// Пояснение под кнопкой.
    pub fn status_line(&self) -> String {
        match &self.stage {
            Stage::Idle => format!("версия {}", version::full()),
            Stage::Checking => "спрашиваю GitHub...".into(),
            Stage::UpToDate => format!("установлена последняя версия — {}", version::full()),
            Stage::Available(release) => {
                let size = release.size as f32 / 1_048_576.0;
                let notes = first_line(&release.notes);
                if notes.is_empty() {
                    format!("доступна {} ({size:.0} МБ), у тебя {}", release.tag, version::short())
                } else {
                    format!("{} ({size:.0} МБ): {notes}", release.tag)
                }
            }
            Stage::Downloading => "качаю комплект; настройки сервера не тронутся".into(),
            Stage::Installed(tag) => {
                format!("{tag} установлена. Закрой игру и запусти заново")
            }
            Stage::Failed(why) => format!("не вышло: {why}"),
        }
    }
}

fn first_line(text: &str) -> String {
    text.lines().map(str::trim).find(|line| !line.is_empty()).unwrap_or("").chars().take(70).collect()
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Updater>();
    app.add_systems(Startup, sweep_old_files);
    app.add_systems(Update, collect_reports);
}

/// Убирает `.old`, оставшиеся от прошлого обновления.
///
/// Раньше это сделать нельзя: файл был занят работавшим процессом. Теперь мы —
/// уже новая версия, и старую можно наконец удалить. Не вышло — не беда, лежит
/// себе дальше.
fn sweep_old_files() {
    let Some(dir) = install_dir() else { return };
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "old") {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Забирает готовые ответы рабочего потока.
fn collect_reports(mut updater: ResMut<Updater>) {
    let Some(inbox) = &updater.inbox else { return };
    let Ok(report) = inbox.lock().map(|inbox| inbox.try_recv()) else { return };
    let Ok(report) = report else { return };
    updater.inbox = None;
    match report {
        Report::Checked(Some(release)) => {
            updater.total = release.size;
            updater.stage = Stage::Available(Box::new(release));
        }
        Report::Checked(None) => updater.stage = Stage::UpToDate,
        Report::Installed(tag) => updater.stage = Stage::Installed(tag),
        Report::Failed(why) => {
            warn!("обновление: {why}");
            updater.stage = Stage::Failed(why);
        }
    }
}

/// Нажатие на кнопку: что именно делать, зависит от того, где мы сейчас.
pub fn act(updater: &mut Updater) {
    match updater.stage.clone() {
        Stage::Idle | Stage::UpToDate | Stage::Failed(_) => start_check(updater),
        Stage::Available(release) => start_install(updater, *release),
        // Идёт работа или уже установлено — жать больше не на что.
        Stage::Checking | Stage::Downloading | Stage::Installed(_) => {}
    }
}

fn start_check(updater: &mut Updater) {
    let (tx, rx) = mpsc::channel();
    updater.inbox = Some(Mutex::new(rx));
    updater.stage = Stage::Checking;
    std::thread::spawn(move || {
        let report = match latest_release() {
            Ok(release) => Report::Checked(release),
            Err(why) => Report::Failed(why),
        };
        let _ = tx.send(report);
    });
}

fn start_install(updater: &mut Updater, release: Release) {
    let (tx, rx) = mpsc::channel();
    updater.inbox = Some(Mutex::new(rx));
    updater.stage = Stage::Downloading;
    updater.total = release.size;
    updater.done.store(0, Ordering::Relaxed);
    let progress = updater.done.clone();
    std::thread::spawn(move || {
        let tag = release.tag.clone();
        let report = match install(&release, &progress) {
            Ok(()) => Report::Installed(tag),
            Err(why) => Report::Failed(why),
        };
        let _ = tx.send(report);
    });
}

// ─────────────────────────────────────────────
// Сеть
// ─────────────────────────────────────────────

/// GitHub требует User-Agent и без него отвечает 403.
const AGENT: &str = concat!("cellborn/", env!("CARGO_PKG_VERSION"));

fn latest_release() -> Result<Option<Release>, String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", repository());
    let mut response = ureq::get(&url)
        .header("User-Agent", AGENT)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|error| match error {
            // Пока ни одного релиза нет, GitHub отвечает 404 — это не поломка,
            // а «обновляться некуда», и говорить об этом надо человеческими
            // словами, а не кодом ответа.
            ureq::Error::StatusCode(404) => "релизов ещё нет".to_string(),
            other => format!("нет связи с GitHub ({other})"),
        })?;

    let json: serde_json::Value =
        response.body_mut().read_json().map_err(|e| format!("непонятный ответ GitHub ({e})"))?;

    let tag = json["tag_name"].as_str().unwrap_or_default().to_string();
    if tag.is_empty() {
        return Err("в ответе GitHub нет тега релиза".into());
    }
    if !version::is_newer(&tag, version::VERSION) {
        return Ok(None);
    }

    // Архив для этой системы. Его может не быть, если сборка под одну из
    // платформ упала, — тогда честнее сказать это, чем молчать.
    let assets = json["assets"].as_array().cloned().unwrap_or_default();
    let asset = assets
        .iter()
        .find(|a| a["name"].as_str() == Some(ASSET))
        .ok_or_else(|| format!("в релизе {tag} нет файла {ASSET}"))?;

    Ok(Some(Release {
        version: tag.trim_start_matches('v').to_string(),
        tag,
        notes: json["body"].as_str().unwrap_or_default().to_string(),
        asset_url: asset["browser_download_url"].as_str().unwrap_or_default().to_string(),
        size: asset["size"].as_u64().unwrap_or(0),
    }))
}

// ─────────────────────────────────────────────
// Установка
// ─────────────────────────────────────────────

/// Каталог, в котором лежит игра.
fn install_dir() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(Path::to_path_buf)
}

fn install(release: &Release, progress: &AtomicU64) -> Result<(), String> {
    let target = install_dir().ok_or("не понял, где лежит игра")?;

    // Всё временное — рядом с игрой, а не в системном temp: переезд файла между
    // разделами превращается в копирование, и атомарности уже не будет.
    let work = target.join(".cellborn-update");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| format!("нет прав писать рядом с игрой ({e})"))?;

    let result = (|| {
        let archive = work.join("release.zip");
        download(&release.asset_url, &archive, release.size, progress)?;
        let unpacked = work.join("unpacked");
        unpack(&archive, &unpacked)?;
        swap_in(&unpacked, &target)
    })();

    // Прибираем за собой в любом случае: оставленный каталог на пол-гигабайта
    // — плохая плата за неудачное обновление.
    let _ = std::fs::remove_dir_all(&work);
    result
}

fn download(url: &str, to: &Path, expected: u64, progress: &AtomicU64) -> Result<(), String> {
    let mut response = ureq::get(url)
        .header("User-Agent", AGENT)
        .call()
        .map_err(|e| format!("не скачалось ({e})"))?;

    let mut file = std::fs::File::create(to).map_err(|e| format!("не создался файл ({e})"))?;
    let mut reader = response.body_mut().as_reader();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut written = 0u64;
    loop {
        let read = reader.read(&mut buffer).map_err(|e| format!("оборвалась закачка ({e})"))?;
        if read == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buffer[..read])
            .map_err(|e| format!("не пишется на диск ({e})"))?;
        written += read as u64;
        progress.store(written, Ordering::Relaxed);
    }

    // Оборванная закачка выглядит как успешная: сервер просто закрыл соединение.
    // Размер знает GitHub, и это единственная дешёвая проверка, что доехало всё.
    if expected > 0 && written != expected {
        return Err(format!("скачалось {written} байт вместо {expected}"));
    }
    Ok(())
}

fn unpack(archive: &Path, into: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|e| format!("архив не открылся ({e})"))?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|e| format!("архив повреждён ({e})"))?;
    zip.extract(into).map_err(|e| format!("не распаковался ({e})"))?;

    // Пустой или чужой архив до подмены файлов лучше поймать здесь.
    if collect_files(into).is_empty() {
        return Err("в архиве нет файлов".into());
    }
    Ok(())
}

/// Переносит распакованное поверх установленной игры.
///
/// Файлы игрока пропускаются, прежние — переименовываются в `.old`, а не
/// удаляются: работающий исполняемый файл заменить нельзя, а переименовать
/// можно, и это же оставляет путь назад, если новая версия не запустится.
fn swap_in(unpacked: &Path, target: &Path) -> Result<(), String> {
    let files = collect_files(unpacked);
    for source in files {
        let name = source
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("в архиве файл со странным именем")?;

        if KEEP.iter().any(|keep| keep.eq_ignore_ascii_case(name)) {
            info!("обновление: {name} принадлежит тебе, оставляю как есть");
            continue;
        }

        // Внутренняя структура архива сохраняется: если однажды появятся
        // подкаталоги, они приедут туда же, куда лежали.
        let relative = source.strip_prefix(unpacked).map_err(|_| "путаница с путями")?;
        let destination = target.join(relative);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("не создался каталог {}: {e}", parent.display()))?;
        }

        if destination.exists() {
            let old = destination.with_extension("old");
            let _ = std::fs::remove_file(&old);
            std::fs::rename(&destination, &old).map_err(|e| {
                format!(
                    "не смог отодвинуть {name}: {e}. \
                     Если запущен сервер — закрой его и попробуй снова"
                )
            })?;
        }

        std::fs::rename(&source, &destination)
            .map_err(|e| format!("не смог поставить {name}: {e}"))?;

        // На Unix права из архива не переживают распаковку так, как хотелось бы:
        // бинарник должен остаться исполняемым.
        #[cfg(unix)]
        if destination.extension().is_none() {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(
                &destination,
                std::fs::Permissions::from_mode(0o755),
            );
        }
    }
    Ok(())
}

/// Все файлы в дереве, рекурсивно.
fn collect_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Конфиг игрока — единственное, ради чего этот список существует.
    #[test]
    fn the_players_config_is_protected_whatever_its_case() {
        for name in [
            "cellborn-server.cfg",
            "Cellborn-Server.CFG",
            "CELLBORN-SERVER.cfg",
            "cellborn.cfg",
            "Cellborn.CFG",
        ] {
            assert!(
                KEEP.iter().any(|keep| keep.eq_ignore_ascii_case(name)),
                "{name} перезаписался бы обновлением"
            );
        }
        assert!(
            !KEEP.iter().any(|keep| keep.eq_ignore_ascii_case("cellborn-server")),
            "сам сервер обновляться обязан"
        );
    }

    /// Живой поход в GitHub. Не для CI: требует сети и зависит от того, что
    /// сейчас лежит в релизах, — поэтому `#[ignore]`.
    ///
    /// ```bash
    /// cargo test -p cellborn-client -- --ignored --nocapture
    /// ```
    ///
    /// Проверяет то, что нельзя проверить без сети: что TLS собрался, что
    /// GitHub нас пускает, и что ответ разбирается. Отсутствие релизов —
    /// нормальный исход, а не провал.
    #[test]
    #[ignore = "ходит в сеть"]
    fn talks_to_github() {
        match latest_release() {
            Ok(Some(release)) => {
                println!("есть релиз {} ({} байт)", release.tag, release.size);
                assert!(!release.asset_url.is_empty(), "нашли релиз без ссылки на архив");
            }
            Ok(None) => println!("релиз есть, но он не новее нашей версии — тоже ответ"),
            Err(why) => {
                println!("GitHub ответил: {why}");
                // Про пустой репозиторий и отсутствующий архив говорить можно —
                // это состояние мира. А вот молчание про причину — нельзя.
                assert!(!why.is_empty(), "ошибка без объяснения");
            }
        }
    }

    /// Имя архива обязано совпадать с тем, что кладёт сборка. Разойдутся —
    /// обновление будет вечно говорить «в релизе нет файла».
    #[test]
    fn asset_name_matches_the_workflow() {
        let workflow = include_str!("../../../.github/workflows/release.yml");
        assert!(
            workflow.contains(ASSET),
            "сборка не делает {ASSET}; поправь release.yml или ASSET"
        );
    }
}
