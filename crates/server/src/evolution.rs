//! Наследственная память сервера: боты учатся между сессиями.
//!
//! Дикие боты рождались случайными и умирали случайными — каждый заход мир
//! начинался с нуля. Отбор в нём происходил, но никуда не записывался, и всё
//! найденное пропадало с последним организмом.
//!
//! Теперь сервер помнит, какие тела жили дольше. Когда рождается новый дикий,
//! он с большой вероятностью собирается из удачного предка с одной случайной
//! правкой — обычный генетический алгоритм, где отбор делает сама игра, а не
//! функция приспособленности: выживших определяет то, кто действительно выжил.
//!
//! **Память переживает перезапуск.** Файл рядом с сервером, тот же формат
//! «ключ = значение», что у конфига, и та же причина: он читается человеком и
//! не тянет зависимостей. Испорченный или чужой файл просто игнорируется —
//! мир начнётся с чистого листа, а не упадёт.

use bevy::prelude::*;
use cellborn_common::*;
use std::path::{Path, PathBuf};

pub const MEMORY_NAME: &str = "cellborn-evolution.txt";

/// Сколько удачных тел хранить.
///
/// Немного: смысл в том, чтобы держать пул разнообразных решений, а не архив.
/// Слишком длинный список делает отбор незаметным, слишком короткий — сводит
/// всё море к одному чертежу.
const KEEP_BEST: usize = 24;

/// Насколько прожитое время должно превзойти худшего в списке, чтобы вытеснить
/// его. Без запаса список бы дёргался от случайных долгожителей.
const IMPROVEMENT: f32 = 1.05;

/// Одно запомненное тело.
#[derive(Debug, Clone)]
pub struct Ancestor {
    /// Сколько секунд оно прожило — единственная мера успеха, какая тут
    /// возможна и нужна.
    pub lifetime: f32,
    /// Органы: семейство и уровень, по одному на часть.
    pub parts: Vec<PartKind>,
}

#[derive(Resource, Default)]
pub struct Heredity {
    pub best: Vec<Ancestor>,
    /// Изменилась ли память с прошлого сохранения.
    dirty: bool,
    since_save: f32,
}

impl Heredity {
    /// Предлагает геном для нового дикого бота.
    ///
    /// Иногда чистая случайность, чаще — потомок удачного предка с одной
    /// правкой. Случайность оставлена намеренно: без неё пул схлопывается в
    /// один чертёж и эволюция останавливается, найдя первый локальный максимум.
    pub fn propose(&self, lineage: u64, roll: u64) -> Genome {
        let mut genome = Genome::starter_of(lineage);
        if self.best.is_empty() || roll % 100 < 25 {
            for i in 0..(roll % 3) {
                genome.push_part(random_part(roll.wrapping_add(i * 7919)));
            }
            return genome;
        }

        // Выбор предка смещён к лучшим: чем выше в списке, тем чаще берут.
        let span = self.best.len();
        let a = (roll >> 8) as usize % span;
        let b = (roll >> 24) as usize % span;
        let ancestor = &self.best[a.min(b)];

        for kind in &ancestor.parts {
            if genome.parts.len() >= MAX_PARTS {
                break;
            }
            genome.push_part(*kind);
        }

        // Одна правка на потомка: мутация должна быть шагом, а не прыжком.
        match (roll >> 40) % 3 {
            // Прибавить орган.
            0 => genome.push_part(random_part(roll)),
            // Поднять уровень случайного органа — то же, что делает игрок.
            1 => {
                if !genome.parts.is_empty() {
                    let index = (roll >> 48) as usize % genome.parts.len();
                    if let Some(better) = genome.parts[index].kind.upgraded() {
                        genome.parts[index].kind = better;
                    }
                }
            }
            // Отбросить орган: без обратного хода отбор умеет только раздувать.
            _ => {
                if genome.parts.len() > 3 {
                    let index = 3 + (roll >> 52) as usize % (genome.parts.len() - 3);
                    genome.parts.remove(index);
                }
            }
        }
        genome
    }

    /// Запоминает тело, если оно прожило дольше худшего из запомненных.
    pub fn remember(&mut self, lifetime: f32, genome: &Genome) {
        // Совсем короткие жизни ничему не учат: это шум отбора.
        if lifetime < 20.0 {
            return;
        }
        let parts: Vec<PartKind> = genome.parts.iter().map(|p| p.kind).collect();

        if self.best.len() < KEEP_BEST {
            self.best.push(Ancestor { lifetime, parts });
        } else {
            let worst = self
                .best
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.lifetime.total_cmp(&b.1.lifetime))
                .map(|(index, entry)| (index, entry.lifetime));
            let Some((index, worst_lifetime)) = worst else { return };
            if lifetime < worst_lifetime * IMPROVEMENT {
                return;
            }
            self.best[index] = Ancestor { lifetime, parts };
        }

        self.best.sort_by(|a, b| b.lifetime.total_cmp(&a.lifetime));
        self.dirty = true;
    }

    fn to_file_text(&self) -> String {
        let mut text = String::from(
            "# Наследственная память сервера.\n\
             # Тела, прожившие дольше прочих: из них собираются новые дикие боты.\n\
             # Формат: lifetime = <секунды> | <орган>:<уровень>, ...\n\
             # Файл можно удалить — мир начнётся с чистого листа.\n",
        );
        for ancestor in &self.best {
            let parts = ancestor
                .parts
                .iter()
                .map(|kind| format!("{}:{}", kind.family.slot(), kind.level.step()))
                .collect::<Vec<_>>()
                .join(",");
            text.push_str(&format!("lifetime = {:.1} | {parts}\n", ancestor.lifetime));
        }
        text
    }

    fn parse(text: &str) -> Self {
        let mut best = Vec::new();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some(rest) = line.strip_prefix("lifetime") else { continue };
            let Some(rest) = rest.trim().strip_prefix('=') else { continue };
            let Some((lifetime, parts)) = rest.split_once('|') else { continue };
            let Ok(lifetime) = lifetime.trim().parse::<f32>() else { continue };

            let parts: Vec<PartKind> = parts
                .split(',')
                .filter_map(|entry| {
                    let (family, level) = entry.trim().split_once(':')?;
                    let family = PartFamily::ALL.get(family.trim().parse::<usize>().ok()?)?;
                    let level = PartLevel::ALL.get(level.trim().parse::<usize>().ok()?)?;
                    Some(PartKind::new(*family, *level))
                })
                .collect();
            if parts.is_empty() {
                continue;
            }
            best.push(Ancestor { lifetime, parts });
        }
        best.sort_by(|a, b| b.lifetime.total_cmp(&a.lifetime));
        best.truncate(KEEP_BEST);
        Self { best, dirty: false, since_save: 0.0 }
    }
}

fn memory_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .map(|dir| dir.join(MEMORY_NAME))
        .unwrap_or_else(|| PathBuf::from(MEMORY_NAME))
}

pub fn load() -> Heredity {
    match std::fs::read_to_string(memory_path()) {
        Ok(text) => {
            let memory = Heredity::parse(&text);
            info!("наследственная память: {} тел", memory.best.len());
            memory
        }
        // Файла нет — первый запуск этого сервера. Не ошибка.
        Err(_) => Heredity::default(),
    }
}

/// Сохраняет память, когда есть что сохранять, и не чаще раза в минуту.
///
/// Чаще незачем: список меняется редко, а писать файл на каждую смерть значило
/// бы дёргать диск ради строки.
pub fn save_periodically(time: Res<Time>, mut memory: ResMut<Heredity>) {
    memory.since_save += time.delta_secs();
    if !memory.dirty || memory.since_save < 60.0 {
        return;
    }
    memory.since_save = 0.0;
    memory.dirty = false;

    let path = memory_path();
    if let Err(error) = std::fs::write(&path, memory.to_file_text()) {
        warn!("не смог сохранить наследственную память в {}: {error}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(parts: &[PartKind]) -> Genome {
        let mut genome = Genome::starter_of(1);
        for kind in parts {
            genome.push_part(*kind);
        }
        genome
    }

    /// Память должна хранить лучших, а не последних: иначе отбора нет.
    #[test]
    fn only_the_long_lived_are_kept() {
        let mut memory = Heredity::default();
        for seconds in 1..=(KEEP_BEST as u32 + 10) {
            memory.remember(
                seconds as f32 * 10.0,
                &body(&[PartKind::basic(PartFamily::Spike)]),
            );
        }
        assert_eq!(memory.best.len(), KEEP_BEST, "список разросся");
        // Сверху самый живучий, снизу — уже не новичок.
        assert!(memory.best[0].lifetime > memory.best[KEEP_BEST - 1].lifetime);
        assert!(memory.best[KEEP_BEST - 1].lifetime > 100.0, "короткие жизни вытеснили длинные");

        // Мимолётная жизнь ничему не учит.
        let before = memory.best.len();
        memory.remember(3.0, &body(&[]));
        assert_eq!(memory.best.len(), before);
    }

    /// Файл должен переживать круг «записали — прочитали»: ради этого он и есть.
    #[test]
    fn memory_survives_a_restart() {
        let mut memory = Heredity::default();
        memory.remember(
            180.0,
            &body(&[
                PartKind::new(PartFamily::Gill, PartLevel::Perfect),
                PartKind::new(PartFamily::Spike, PartLevel::Fine),
            ]),
        );
        let restored = Heredity::parse(&memory.to_file_text());
        assert_eq!(restored.best.len(), 1);
        assert!((restored.best[0].lifetime - 180.0).abs() < 0.2);
        // Органы должны вернуться теми же, вместе с уровнями.
        let parts = &restored.best[0].parts;
        assert!(parts.iter().any(|k| k.family == PartFamily::Gill && k.level == PartLevel::Perfect));
        assert!(parts.iter().any(|k| k.family == PartFamily::Spike && k.level == PartLevel::Fine));
    }

    /// Испорченный файл не должен ронять сервер: мир просто начнётся заново.
    #[test]
    fn a_broken_file_is_ignored_quietly() {
        for junk in [
            "мусор",
            "lifetime = не-число | 1:1",
            "lifetime = 50 |",
            "lifetime = 50 | 999:999",
            "",
        ] {
            let memory = Heredity::parse(junk);
            assert!(memory.best.is_empty(), "мусор «{junk}» принят за память");
        }
    }

    /// Потомок удачного предка должен наследовать его, а не рождаться заново.
    #[test]
    fn offspring_inherit_from_the_remembered() {
        let mut memory = Heredity::default();
        let ancestor: Vec<PartKind> = (0..6)
            .map(|_| PartKind::new(PartFamily::Carapace, PartLevel::Fine))
            .collect();
        memory.remember(300.0, &body(&ancestor));

        // Перебираем зёрна: часть даёт чистую случайность, и это нормально, но
        // наследование обязано случаться.
        let inherited = (0..200u64)
            .map(|roll| memory.propose(1, roll * 2_654_435_761))
            .filter(|genome| genome.count_family(PartFamily::Carapace) >= 5)
            .count();
        assert!(inherited > 50, "предок почти не наследуется: {inherited} из 200");
    }
}
