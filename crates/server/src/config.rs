//! Server settings that can be changed without recompiling.
//!
//! A plain `ключ = значение` file next to the binary. No new dependency, no
//! syntax to learn, and it works the same on Windows where the person running
//! the server has a folder with two .exe files and no toolchain.
//!
//! Lookup order: `--config <путь>` on the command line, then the file next to
//! the executable, then the working directory. Environment variables override
//! whatever the file said. If no file is found, one is written with the
//! defaults, so the first run leaves you something to edit.
//!
//! **What is not here, and why.** Anything the client also computes stays a
//! compile-time constant: swimming speed, mass drag, arena size, tick rate and
//! the mutation price curve. The client predicts its own movement and draws its
//! own panel with those numbers, so a server that quietly used different ones
//! would fight the prediction on every tick and lie in the UI. Only values the
//! server alone decides are configurable.

use bevy::prelude::*;
use cellborn_common::*;
use std::path::{Path, PathBuf};

pub const CONFIG_NAME: &str = "cellborn-server.cfg";

/// One table generates the struct, the defaults, the parser and the file that
/// gets written — so a setting cannot exist in one of them and be missing from
/// another.
macro_rules! settings {
    ($( $section:literal { $( $key:ident : $ty:ty = $default:expr , $doc:literal ;)* } )*) => {
        #[derive(Resource, Debug, Clone)]
        pub struct ServerConfig {
            $($( pub $key: $ty, )*)*
        }

        impl Default for ServerConfig {
            fn default() -> Self {
                Self { $($( $key: $default, )*)* }
            }
        }

        impl ServerConfig {
            /// The file as it would be written out, comments included.
            pub fn to_file_text(&self) -> String {
                let mut text = String::from(
                    "# Настройки сервера Cellborn.\n\
                     # Меняются без пересборки: правь файл и перезапусти сервер.\n\
                     # Любое значение перекрывается переменной окружения:\n\
                     #   CELLBORN_MAX_ORGANISMS=120 ./cellborn-server\n\
                     #\n\
                     # Здесь только то, что решает сервер. Скорость, размер арены,\n\
                     # тикрейт и цены мутаций остаются в коде: их считает и клиент,\n\
                     # и расхождение сломало бы предсказание движения.\n",
                );
                $(
                    text.push_str(&format!("\n# --- {} ---\n", $section));
                    $(
                        text.push_str(&format!("\n# {}\n{} = {}\n", $doc, stringify!($key), self.$key));
                    )*
                )*
                text
            }
        }

        /// Настройки, которых нет в файле, — текстом, готовым к дописыванию.
        ///
        /// Файл принадлежит пользователю: он его правил, расставлял свои
        /// комментарии, менял порядок. Перезаписывать его целиком нельзя, а
        /// молча не давать доступа к новым настройкам — тоже: человек о них
        /// просто не узнает. Поэтому дописываем в конец только недостающее,
        /// с тем же описанием, что и в сгенерированном файле.
        fn missing_settings(present: &[String]) -> Option<String> {
            let defaults = ServerConfig::default();
            let mut text = String::new();
            $(
                // Сначала раздел целиком, потом заголовок — иначе в файл
                // попадали бы заголовки разделов, под которыми ничего нет.
                let mut section = String::new();
                $(
                    if !present.iter().any(|key| key == stringify!($key)) {
                        section.push_str(&format!(
                            "\n# {}\n{} = {}\n",
                            $doc,
                            stringify!($key),
                            defaults.$key
                        ));
                    }
                )*
                if !section.is_empty() {
                    text.push_str(&format!("\n# --- {} ---\n", $section));
                    text.push_str(&section);
                }
            )*
            (!text.is_empty()).then_some(text)
        }

        fn apply(key: &str, value: &str, config: &mut ServerConfig) {
            match key {
                $($(
                    stringify!($key) => match value.parse::<$ty>() {
                        Ok(parsed) => config.$key = parsed,
                        Err(_) => warn!("не смог прочитать {key} = {value}, оставляю прежнее"),
                    },
                )*)*
                other => warn!("незнакомая настройка в конфиге: {other}"),
            }
        }

        /// Every настройка, for the environment-variable pass.
        const KEYS: &[&str] = &[ $($( stringify!($key), )*)* ];
    };
}

settings! {
    "сеть" {
        port: u16 = SERVER_PORT, "UDP-порт, который слушает сервер";
    }

    "размер мира" {
        max_organisms: usize = MAX_ORGANISMS,
            "Сколько всего организмов может жить одновременно (игроки и боты)";
        max_colony_size: usize = MAX_COLONY_SIZE,
            "Сколько особей может быть в одном роду: предел размножения колонии";
        max_parts: usize = MAX_PARTS,
            "Сколько частей может отрастить одно тело (потолок 100: по нему рисует клиент)";
        season_length: f32 = 180.0, "Длительность одного сезона в секундах";
    }

    "еда" {
        food_target: usize = FOOD_TARGET,
            "Сколько частиц еды держать в воде (умножается на плотность сезона)";
        food_spawn_rate: f32 = FOOD_SPAWN_RATE, "Сколько частиц появляется в секунду";
        corpse_nutrients: usize = CORPSE_NUTRIENTS,
            "Сколько детрита оставляет труп (плюс надбавка за массу)";
    }

    "выживание" {
        base_upkeep: f32 = BASE_UPKEEP, "Базовый расход энергии в секунду";
        penalty_upkeep: f32 = PENALTY_UPKEEP,
            "Во что обходится единица штрафа адаптации, энергии в секунду";
        starvation_damage: f32 = STARVATION_DAMAGE,
            "Сколько здоровья теряется в секунду при нулевой энергии";
        health_regen: f32 = HEALTH_REGEN, "Сколько здоровья восстанавливается в секунду";
        well_fed_fraction: f32 = WELL_FED_FRACTION,
            "Доля запаса энергии, выше которой организм лечится (0.5 = половина)";
    }

    "бой" {
        base_attack: f32 = BASE_ATTACK, "Урон в секунду голой мембраной, без оружия";
        attack_margin: f32 = ATTACK_MARGIN, "Насколько далеко от тел засчитывается контакт";
        combat_regen_block: f32 = COMBAT_REGEN_BLOCK,
            "Сколько секунд после удара не восстанавливается здоровье";
        kill_energy_yield: f32 = KILL_ENERGY_YIELD,
            "Сколько энергии даёт единица массы съеденной жертвы";
        points_per_kill: u16 = POINTS_PER_KILL, "Очков мутаций за убийство";
        aggression_threshold: u32 = AGGRESSION_THRESHOLD,
            "На сколько органов должны разойтись чужаки, чтобы драться";
        kin_split_threshold: u32 = KIN_SPLIT_THRESHOLD,
            "То же для родни: после этого род раскалывается (клиент красит по своим значениям)";
    }

    "размножение" {
        base_division_time: f32 = BASE_DIVISION_TIME, "Секунд между делениями без делителей";
        division_energy_fraction: f32 = DIVISION_ENERGY_FRACTION,
            "Какую долю запаса надо накопить, чтобы делиться (0.7 = 70%)";
        division_energy_share: f32 = DIVISION_ENERGY_SHARE,
            "Какую долю своей энергии родитель отдаёт потомку";
        base_mutation_chance: f32 = BASE_MUTATION_CHANCE,
            "Шанс, что потомок родится с лишней случайной частью";
    }

    "смерть" {
        respawn_delay: f32 = RESPAWN_DELAY, "Через сколько секунд игрок возвращается в игру";
        death_point_retention: f32 = DEATH_POINT_RETENTION,
            "Какая доля очков мутаций переживает смерть";
    }

    "очки мутаций" {
        energy_per_mutation_point: f32 = ENERGY_PER_MUTATION_POINT,
            "Сколько энергии надо съесть на одно очко";
        points_per_season: u16 = POINTS_PER_SEASON, "Очков за переживание смены сезона";
        mutation_cooldown: f32 = MUTATION_COOLDOWN,
            "Минимум секунд между принятыми запросами мутации от одного клиента";
    }

    "опасности и приманки" {
        thorn_count: usize = THORN_COUNT, "Сколько колючек расставить в море (0 — выключить)";
        thorn_damage: f32 = THORN_DAMAGE,
            "Урон в секунду тому, кто слишком велик, чтобы пройти сквозь колючку";
        leviathan_interval: f32 = LEVIATHAN_INTERVAL,
            "Средний промежуток между заплывами чудовища, секунд (0 — выключить)";
        leviathan_speed: f32 = LEVIATHAN_SPEED,
            "Во сколько раз гигант быстрее, чем позволяет его масса";
        feast_count: usize = FEAST_COUNT,
            "Сколько лакомых мест держать одновременно (0 — еда сыплется ровно)";
    }

    "боты" {
        wild_target: usize = WILD_TARGET, "Сколько диких ботов поддерживать в мире";
        wild_max_parts: usize = WILD_MAX_PARTS,
            "Предел частей для ботов (по умолчанию такой же, как у игроков)";
        wild_mutation_interval: f32 = WILD_MUTATION_INTERVAL,
            "Раз во сколько секунд бот решает потратить накопленные очки";
        bot_vision: f32 = BOT_VISION, "На каком расстоянии бот видит еду и врагов";
    }

    "загрязнение воды" {
        pollution_per_upkeep: f32 = POLLUTION_PER_UPKEEP,
            "Сколько грязи в секунду даёт единица содержания тела (0 — выключить)";
    }

    "ядовитые облака" {
        suffocation_damage: f32 = SUFFOCATION_DAMAGE,
            "Урон здоровью в секунду за нехватку кислорода сверх запаса (0 — выключить)";
        toxin_damage: f32 = TOXIN_DAMAGE,
            "Урон здоровью в секунду за единицу яда сверх стойкости организма";
        toxin_interval: f32 = TOXIN_INTERVAL, "Раз во сколько секунд железа оставляет облако";
        toxin_lifetime: f32 = TOXIN_LIFETIME, "Сколько секунд живёт облако";
        toxin_radius: f32 = TOXIN_RADIUS, "Радиус облака";
    }
}

fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let args: Vec<String> = std::env::args().collect();
    if let Some(index) = args.iter().position(|a| a == "--config") {
        if let Some(path) = args.get(index + 1) {
            paths.push(PathBuf::from(path));
        }
    }
    // Next to the executable: this is where a Windows player's file will live.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            paths.push(dir.join(CONFIG_NAME));
        }
    }
    paths.push(PathBuf::from(CONFIG_NAME));
    paths
}

fn parse(text: &str, config: &mut ServerConfig) {
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        apply(key.trim(), value.trim(), config);
    }
}

/// Какие ключи в файле реально заданы — с точки зрения того же разбора, что и
/// в [`parse`], иначе настройка, закомментированная пользователем, сошла бы за
/// присутствующую и он никогда бы её не увидел.
fn keys_in(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let line = line.split('#').next().unwrap_or("").trim();
            line.split_once('=').map(|(key, _)| key.trim().to_string())
        })
        .collect()
}

/// Дописывает в пользовательский файл настройки, появившиеся в новой версии.
///
/// Конфиг принадлежит игроку и переживает обновление игры — в этом его смысл.
/// Но новая версия приносит новые настройки, и если о них не сказать, человек
/// просто не узнает, что они есть. Поэтому файл не перезаписывается, а
/// дополняется в конец: чужие значения, комментарии и порядок остаются как были.
fn append_new_settings(path: &Path, text: &str) {
    let Some(addition) = missing_settings(&keys_in(text)) else { return };
    let names: Vec<&str> = addition
        .lines()
        .filter_map(|line| line.split_once(" = ").map(|(key, _)| key))
        .filter(|key| !key.starts_with('#'))
        .collect();

    let mut updated = text.to_string();
    if !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(
        "\n# --- Добавлено при обновлении игры ---\n         # Ниже настройки, которых не было в твоей версии. Значения по умолчанию;\n         # правь как любые другие. Твои прежние значения не тронуты.\n",
    );
    updated.push_str(&addition);

    match std::fs::write(path, updated) {
        Ok(()) => info!(
            "в конфиг дописаны новые настройки ({}): {}",
            names.len(),
            names.join(", ")
        ),
        // Не беда: сервер и так запустится на значениях по умолчанию. Права
        // на файл могут быть чужими, каталог — только для чтения.
        Err(error) => warn!(
            "не смог дописать новые настройки в {}: {error}. \
             Сервер работает, но в файле их не будет: {}",
            path.display(),
            names.join(", ")
        ),
    }
}

/// Keeps a hand-edited file from producing an unplayable world.
fn sanitize(config: &mut ServerConfig) {
    config.max_organisms = config.max_organisms.max(1);
    config.max_colony_size = config.max_colony_size.max(1);
    // The client draws the body against the compiled ceiling, so the server may
    // lower the limit but never raise it.
    config.max_parts = config.max_parts.clamp(3, MAX_PARTS);
    config.wild_max_parts = config.wild_max_parts.clamp(3, config.max_parts);
    config.season_length = config.season_length.max(5.0);
    config.food_spawn_rate = config.food_spawn_rate.max(0.0);
    config.well_fed_fraction = config.well_fed_fraction.clamp(0.0, 1.0);
    config.division_energy_fraction = config.division_energy_fraction.clamp(0.05, 1.0);
    config.division_energy_share = config.division_energy_share.clamp(0.05, 0.9);
    config.base_mutation_chance = config.base_mutation_chance.clamp(0.0, 1.0);
    config.death_point_retention = config.death_point_retention.clamp(0.0, 1.0);
    config.base_division_time = config.base_division_time.max(1.0);
    config.energy_per_mutation_point = config.energy_per_mutation_point.max(1.0);
    config.mutation_cooldown = config.mutation_cooldown.max(0.0);
    config.toxin_lifetime = config.toxin_lifetime.max(0.5);
    config.toxin_radius = config.toxin_radius.max(0.5);
    config.toxin_damage = config.toxin_damage.max(0.0);
    config.suffocation_damage = config.suffocation_damage.max(0.0);
    config.pollution_per_upkeep = config.pollution_per_upkeep.max(0.0);
    config.bot_vision = config.bot_vision.max(1.0);
    config.thorn_damage = config.thorn_damage.max(0.0);
    config.leviathan_interval = config.leviathan_interval.max(0.0);
    config.leviathan_speed = config.leviathan_speed.max(0.0);
}

/// Loads the config, writing a default file if there is none to read.
pub fn load() -> ServerConfig {
    let mut config = ServerConfig::default();
    let mut loaded_from: Option<PathBuf> = None;

    for path in candidate_paths() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            parse(&text, &mut config);
            // Файл старой версии не знает о новых настройках — допишем их,
            // не трогая того, что игрок уже настроил.
            append_new_settings(&path, &text);
            loaded_from = Some(path);
            break;
        }
    }

    // Environment always wins: that is what makes the Docker case bearable.
    for key in KEYS {
        let variable = format!("CELLBORN_{}", key.to_uppercase());
        if let Ok(value) = std::env::var(&variable) {
            apply(key, value.trim(), &mut config);
        }
    }

    sanitize(&mut config);

    match &loaded_from {
        Some(path) => info!("конфиг прочитан: {}", path.display()),
        None => {
            let path = default_write_path();
            match std::fs::write(&path, config.to_file_text()) {
                Ok(()) => {
                    info!("конфига не было, записал со значениями по умолчанию: {}", path.display())
                }
                Err(error) => warn!("не смог записать конфиг {}: {error}", path.display()),
            }
        }
    }

    info!(
        "настройки: порт {}, особей до {}, в роду до {}, частей до {} (дикие {}), \
         диких {}, еды {}, сезон {:.0}с",
        config.port,
        config.max_organisms,
        config.max_colony_size,
        config.max_parts,
        config.wild_max_parts,
        config.wild_target,
        config.food_target,
        config.season_length
    );
    config
}

fn default_write_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .map(|dir| dir.join(CONFIG_NAME))
        .unwrap_or_else(|| PathBuf::from(CONFIG_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_values_and_ignores_comments() {
        let mut config = ServerConfig::default();
        parse(
            "# комментарий\nmax_organisms = 120\n\nmax_colony_size=7  # и тут\nport = 6000\n\
             base_attack = 4.5\n",
            &mut config,
        );
        assert_eq!(config.max_organisms, 120);
        assert_eq!(config.max_colony_size, 7);
        assert_eq!(config.port, 6000);
        assert_eq!(config.base_attack, 4.5);
    }

    /// A broken line must not silently reset the world.
    #[test]
    fn keeps_previous_value_when_a_line_is_garbage() {
        let mut config = ServerConfig::default();
        parse("max_organisms = много\nbase_upkeep = чуть-чуть\n", &mut config);
        assert_eq!(config.max_organisms, MAX_ORGANISMS);
        assert_eq!(config.base_upkeep, BASE_UPKEEP);
    }

    /// The config may lower the part limit but never raise it past the ceiling
    /// the client is drawing against.
    #[test]
    fn part_limit_is_clamped_to_the_genome_ceiling() {
        let mut config = ServerConfig::default();
        parse("max_parts = 5000\n", &mut config);
        sanitize(&mut config);
        assert_eq!(config.max_parts, MAX_PARTS);

        parse("max_parts = 30\nwild_max_parts = 90\n", &mut config);
        sanitize(&mut config);
        assert_eq!(config.max_parts, 30);
        assert_eq!(config.wild_max_parts, 30, "дикие не могут превысить общий предел");
    }

    /// Nonsense in the file must not produce an unplayable world.
    #[test]
    fn absurd_values_are_clamped() {
        let mut config = ServerConfig::default();
        parse("division_energy_share = 5\nwell_fed_fraction = -2\nseason_length = 0\n", &mut config);
        sanitize(&mut config);
        assert!(config.division_energy_share <= 0.9);
        assert_eq!(config.well_fed_fraction, 0.0);
        assert!(config.season_length >= 5.0);
    }

    /// Every setting must survive a write/read round trip, or the file the server
    /// generates is not a valid file for the server to read.
    #[test]
    fn written_file_round_trips() {
        let original = ServerConfig {
            port: 7777,
            max_organisms: 99,
            base_attack: 3.25,
            toxin_radius: 4.5,
            points_per_kill: 9,
            ..Default::default()
        };
        let mut parsed = ServerConfig::default();
        parse(&original.to_file_text(), &mut parsed);
        assert_eq!(parsed.port, 7777);
        assert_eq!(parsed.max_organisms, 99);
        assert_eq!(parsed.base_attack, 3.25);
        assert_eq!(parsed.toxin_radius, 4.5);
        assert_eq!(parsed.points_per_kill, 9);
        assert_eq!(parsed.season_length, original.season_length);
    }

    /// Главное свойство дописывания: чужой файл остаётся чужим файлом.
    #[test]
    fn appending_keeps_everything_the_player_wrote() {
        let original = "# мои настройки, не трогать\n\
                        max_organisms = 120   # специально много\n\
                        \n\
                        port=6000\n";
        let addition = missing_settings(&keys_in(original)).expect("новые настройки есть");

        // Заданное игроком в дописку не попадает.
        assert!(!addition.contains("\nmax_organisms = "), "перезаписали заданную настройку");
        assert!(!addition.contains("\nport = "), "перезаписали заданную настройку");
        // А то, чего у него не было, — попадает.
        assert!(addition.contains("\nbase_attack = "));
        assert!(addition.contains("\nseason_length = "));

        // Склеенный файл читается и сохраняет всё, что игрок настроил.
        let combined = format!("{original}{addition}");
        let mut config = ServerConfig::default();
        parse(&combined, &mut config);
        assert_eq!(config.max_organisms, 120, "потеряно значение игрока");
        assert_eq!(config.port, 6000, "потеряно значение игрока");
        assert_eq!(config.base_attack, BASE_ATTACK, "новая настройка не по умолчанию");
    }

    /// Полный файл дописывать нечем — второй запуск не должен его раздувать.
    #[test]
    fn appending_is_idempotent() {
        let full = ServerConfig::default().to_file_text();
        assert!(missing_settings(&keys_in(&full)).is_none(), "дописал в полный файл");
    }

    /// Закомментированная настройка — это её отсутствие, а не присутствие.
    /// Иначе игрок, спрятавший строку под `#`, о новой версии ключа не узнает.
    #[test]
    fn commented_out_settings_count_as_missing() {
        let text = "# base_attack = 99\nport = 5555\n";
        let keys = keys_in(text);
        assert!(keys.iter().any(|k| k == "port"));
        assert!(!keys.iter().any(|k| k == "base_attack"));
        let addition = missing_settings(&keys).expect("есть что дописать");
        assert!(addition.contains("\nbase_attack = "));
    }

    /// Заголовок раздела без настроек под ним — мусор в файле игрока.
    #[test]
    fn empty_sections_are_not_written() {
        // Файл, где не хватает ровно одной настройки из раздела «сеть».
        let mut text = ServerConfig::default().to_file_text();
        text = text.replace("port = 5555", "# port убран");
        let addition = missing_settings(&keys_in(&text)).expect("port отсутствует");
        assert!(addition.contains("--- сеть ---"));
        assert!(!addition.contains("--- бой ---"), "заголовок раздела без содержимого");
    }

    /// The generated file has to document every setting it contains.
    #[test]
    fn every_setting_is_written_with_a_comment() {
        let text = ServerConfig::default().to_file_text();
        for key in KEYS {
            assert!(text.contains(&format!("{key} = ")), "в файле нет настройки {key}");
        }
        let documented = text.lines().filter(|l| l.starts_with("# ") && l.len() > 4).count();
        assert!(documented >= KEYS.len(), "не у каждой настройки есть описание");
    }
}
