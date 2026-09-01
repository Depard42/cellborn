//! Настройки игрока: громкость и запомненные серверы.
//!
//! Живут в файле рядом с игрой и **переживают обновление**: обновлятор их не
//! трогает так же, как не трогает конфиг сервера. Иначе каждое обновление
//! сбрасывало бы громкость и стирало список серверов, к которым игрок ходит
//! годами.
//!
//! Формат тот же `ключ = значение`, что и у сервера, и по той же причине: он
//! читается человеком, правится блокнотом и не тянет ни одной зависимости.
//! Незнакомые ключи сохраняются как есть — файл, записанный более новой
//! версией, не должен терять данные при запуске старой.

use bevy::prelude::*;
use std::path::{Path, PathBuf};

pub const SETTINGS_NAME: &str = "cellborn.cfg";

/// Запомненный сервер.
#[derive(Debug, Clone, PartialEq)]
pub struct Server {
    pub address: String,
    /// Как игрок его назвал, или как он представился при обнаружении.
    pub name: String,
}

#[derive(Resource, Debug, Clone)]
pub struct Settings {
    /// Общая громкость, 0..1.
    pub volume: f32,
    /// Громкость музыки относительно общей.
    pub music: f32,
    /// Громкость звуков относительно общей.
    pub effects: f32,
    /// Серверы, к которым игрок подключался. Первый — последний удачный.
    pub servers: Vec<Server>,
    /// Строки файла, которых мы не поняли: чужие или из будущей версии.
    unknown: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 0.7,
            music: 0.5,
            effects: 0.9,
            servers: Vec::new(),
            unknown: Vec::new(),
        }
    }
}

impl Settings {
    /// Итоговая громкость музыки и звуков с учётом общей.
    pub fn music_gain(&self) -> f32 {
        (self.volume * self.music).clamp(0.0, 1.0)
    }

    pub fn effect_gain(&self) -> f32 {
        (self.volume * self.effects).clamp(0.0, 1.0)
    }

    /// Поднимает сервер наверх списка, добавляя его, если он новый.
    ///
    /// Наверху — последний удачный: именно к нему игрок захочет вернуться, и
    /// именно он должен подставляться в поле адреса при следующем запуске.
    pub fn remember(&mut self, address: &str, name: &str) {
        self.servers.retain(|s| s.address != address);
        self.servers.insert(
            0,
            Server { address: address.to_string(), name: name.to_string() },
        );
        // Больше восьми — это уже не память, а свалка.
        self.servers.truncate(8);
    }


    fn to_file_text(&self) -> String {
        let mut text = String::from(
            "# Настройки игры Cellborn.\n\
             # Правятся прямо здесь; обновление игры этот файл не трогает.\n\
             \n\
             # Громкость, от 0 до 1.\n",
        );
        text.push_str(&format!("volume = {:.2}\n", self.volume));
        text.push_str(&format!("music = {:.2}\n", self.music));
        text.push_str(&format!("effects = {:.2}\n", self.effects));
        text.push_str("\n# Запомненные серверы: адрес и имя через пробел.\n");
        for server in &self.servers {
            text.push_str(&format!("server = {} {}\n", server.address, server.name));
        }
        if !self.unknown.is_empty() {
            text.push_str(
                "\n# Строки, которых эта версия игры не знает. Скорее всего их\n\
                 # записала более новая версия — они сохранены нетронутыми.\n",
            );
            for line in &self.unknown {
                text.push_str(line);
                text.push('\n');
            }
        }
        text
    }

    fn parse(text: &str) -> Self {
        let mut settings = Settings { servers: Vec::new(), ..Default::default() };
        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else { continue };
            let (key, value) = (key.trim(), value.trim());
            match key {
                "volume" => settings.volume = parse_gain(value, settings.volume),
                "music" => settings.music = parse_gain(value, settings.music),
                "effects" => settings.effects = parse_gain(value, settings.effects),
                "server" => {
                    let (address, name) = value.split_once(' ').unwrap_or((value, ""));
                    if !address.is_empty() {
                        settings.servers.push(Server {
                            address: address.to_string(),
                            name: name.trim().to_string(),
                        });
                    }
                }
                // Не выбрасываем: файл мог быть записан более новой версией, и
                // терять её настройки при запуске старой нельзя.
                _ => settings.unknown.push(raw.trim().to_string()),
            }
        }
        settings
    }
}

fn parse_gain(value: &str, fallback: f32) -> f32 {
    value.parse::<f32>().map(|v| v.clamp(0.0, 1.0)).unwrap_or(fallback)
}

/// Где лежит файл: рядом с игрой, как и конфиг сервера.
pub fn settings_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .map(|dir| dir.join(SETTINGS_NAME))
        .unwrap_or_else(|| PathBuf::from(SETTINGS_NAME))
}

pub fn load() -> Settings {
    match std::fs::read_to_string(settings_path()) {
        Ok(text) => Settings::parse(&text),
        // Файла нет — первый запуск. Значения по умолчанию, файл появится при
        // первом же изменении.
        Err(_) => Settings::default(),
    }
}

pub fn save(settings: &Settings) {
    let path = settings_path();
    if let Err(error) = std::fs::write(&path, settings.to_file_text()) {
        warn!("не смог сохранить настройки в {}: {error}", path.display());
    }
}

/// Сохраняет настройки, когда они изменились, и не чаще.
pub fn save_when_changed(settings: Res<Settings>) {
    if settings.is_changed() && !settings.is_added() {
        save(&settings);
    }
}

pub fn plugin(app: &mut App) {
    app.insert_resource(load());
    app.add_systems(Update, save_when_changed);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Файл должен переживать круг «записали — прочитали» без потерь: иначе
    /// обновление игры незаметно съест настройки.
    #[test]
    fn settings_survive_a_round_trip() {
        let mut original = Settings { volume: 0.33, music: 0.25, effects: 0.8, ..Default::default() };
        original.remember("192.168.1.10:5555", "Дома");
        original.remember("127.0.0.1:5555", "Локальный");

        let parsed = Settings::parse(&original.to_file_text());
        assert!((parsed.volume - 0.33).abs() < 0.01);
        assert!((parsed.music - 0.25).abs() < 0.01);
        assert_eq!(parsed.servers.len(), 2);
        // Последний запомненный — первый в списке.
        assert_eq!(parsed.servers[0].address, "127.0.0.1:5555");
        assert_eq!(parsed.servers[0].name, "Локальный");
    }

    /// Настройки более новой версии не должны пропадать при запуске старой.
    #[test]
    fn unknown_keys_are_kept() {
        let text = "volume = 0.5\nтембр_музыки = звонкий\n";
        let parsed = Settings::parse(text);
        assert!((parsed.volume - 0.5).abs() < 0.01);
        let written = parsed.to_file_text();
        assert!(written.contains("тембр_музыки = звонкий"), "чужая настройка потеряна");
    }

    /// Повторное подключение не должно плодить дубликаты.
    #[test]
    fn remembering_the_same_server_moves_it_up_instead_of_duplicating() {
        let mut settings = Settings::default();
        settings.remember("a:1", "А");
        settings.remember("b:2", "Б");
        settings.remember("a:1", "А заново");
        assert_eq!(settings.servers.len(), 2);
        assert_eq!(settings.servers[0].address, "a:1");
        assert_eq!(settings.servers[0].name, "А заново");
    }

    /// Мусор в файле не должен обнулять громкость.
    #[test]
    fn garbage_does_not_reset_the_volume() {
        let parsed = Settings::parse("volume = очень громко\nmusic = 2.5\n");
        assert!((parsed.volume - Settings::default().volume).abs() < 0.01);
        assert!(parsed.music <= 1.0, "громкость не зажата");
    }
}
