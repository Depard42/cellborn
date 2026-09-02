//! Звук и музыка — синтезированные, а не записанные.
//!
//! У игры нет ассетов: шрифт вшит в бинарник, всё остальное процедурное. Звук
//! живёт по тому же правилу. Это не аскеза ради аскезы — так дистрибутив
//! остаётся парой файлов, обновление качается за секунды, а музыка не
//! повторяется, потому что её каждый раз сочиняют заново.
//!
//! **Про «нераздражающие».** Раздражает в игровых звуках обычно не тембр, а
//! повтор: одинаковый щелчок на сотое нажатие бесит независимо от того, как он
//! звучит. Поэтому у каждого звука здесь плавающая высота, мягкая атака без
//! щелчка на старте и короткий хвост — ухо воспринимает их как разные события
//! одной природы, а не как один и тот же сигнал.

use bevy::audio::Volume;
use bevy::prelude::*;
use cellborn_common::*;
use lightyear::prelude::*;

use crate::settings::Settings;

/// Частота дискретизации. Сорок четыре килогерца избыточны для синусоид,
/// двадцать два — ровно то, что нужно, и вдвое меньше памяти.
const RATE: u32 = 22_050;

/// Пентатоника ля-минора, от неё строится всё.
///
/// Пентатоника выбрана не для красоты слова: в ней нет полутоновых трений, и
/// **любые** её ноты, взятые вместе или подряд, звучат согласованно. Это
/// позволяет сочинять музыку случайным выбором нот и никогда не получить
/// фальшь — что и требовалось: «случайная расслабляющая по гармонии».
const SCALE: [f32; 5] = [220.00, 261.63, 293.66, 329.63, 392.00];

/// Готовые звуки события.
#[derive(Resource)]
pub struct Sounds {
    pub mutation: Handle<AudioSource>,
    pub division: Handle<AudioSource>,
}

/// Отмечает играющую сейчас музыкальную фразу.
#[derive(Component)]
pub struct Music;

/// Отмечает разовый звук, чтобы его можно было убрать после проигрывания.
#[derive(Component)]
pub struct OneShot;

/// Что клиент видел в прошлый кадр — чтобы отличить событие от состояния.
#[derive(Resource, Default)]
struct Seen {
    divisions: u32,
    parts: usize,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Seen>();
    app.add_systems(Startup, load_sounds);
    app.add_systems(Update, (keep_music_playing, watch_events).chain());
}

// ─────────────────────────────────────────────
// Синтез
// ─────────────────────────────────────────────

/// Заворачивает сэмплы в WAV, который умеет читать Bevy.
///
/// Шестнадцать бит, моно. Заголовок короткий и полностью описан здесь, чтобы не
/// тянуть кодировщик ради сорока четырёх байт.
fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = samples.len() as u32 * 2;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // размер блока fmt
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // моно
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes()); // байт в секунду
    out.extend_from_slice(&2u16.to_le_bytes()); // выравнивание кадра
    out.extend_from_slice(&16u16.to_le_bytes()); // бит на сэмпл
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        let clipped = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        out.extend_from_slice(&clipped.to_le_bytes());
    }
    out
}

/// Мягкая огибающая: без неё любой звук начинается щелчком.
///
/// Щелчок — это разрыв в начале волны, и именно он делает звук «резким»
/// независимо от тембра. Здесь плавный вход и длинный выход.
fn envelope(position: f32, attack: f32) -> f32 {
    if position < attack {
        // Косинусный вход вместо линейного: у линейного слышен излом.
        (1.0 - (position / attack * std::f32::consts::PI).cos()) * 0.5
    } else {
        let tail = (position - attack) / (1.0 - attack).max(1e-3);
        (1.0 - tail).powf(1.6)
    }
}

/// Складывает ноту с несколькими обертонами.
fn note(samples: &mut Vec<f32>, freq: f32, seconds: f32, gain: f32, attack: f32) {
    let count = (RATE as f32 * seconds) as usize;
    for i in 0..count {
        let position = i as f32 / count as f32;
        let t = i as f32 / RATE as f32;
        let phase = t * freq * std::f32::consts::TAU;
        // Основной тон плюс тихая октава и квинта: получается мягкий, «водяной»
        // тембр вместо голой синусоиды, которая звучит как сигнал прибора.
        let wave = phase.sin() + (phase * 2.0).sin() * 0.22 + (phase * 3.0).sin() * 0.08;
        let value = wave * envelope(position, attack) * gain;
        if i < samples.len() {
            samples[i] += value;
        } else {
            samples.push(value);
        }
    }
}

/// Звук выросшего органа: одна нота, мягко.
///
/// Была пара нот восходящим шагом — «выросло». Но орган растят часто, и на
/// десятый раз двухнотная фигура начинает звучать как мелодия, которую тебе
/// играют против воли. Одна нота — это отметка о событии, и её ухо перестаёт
/// замечать ровно тогда, когда должно.
fn mutation_sound(seed: u64) -> Vec<u8> {
    let mut samples = vec![0.0; (RATE as f32 * 0.34) as usize];
    let root = SCALE[(seed % SCALE.len() as u64) as usize];
    note(&mut samples, root, 0.34, 0.22, 0.12);
    wav(&samples)
}

/// Деление: короткий пузырь — тон, быстро уходящий вниз.
fn division_sound() -> Vec<u8> {
    let count = (RATE as f32 * 0.34) as usize;
    let mut samples = Vec::with_capacity(count);
    let mut phase = 0.0f32;
    for i in 0..count {
        let position = i as f32 / count as f32;
        // Скользящая вниз частота: так звучит отрывающийся пузырь.
        let freq = 520.0 * (1.0 - position * 0.55);
        phase += freq / RATE as f32 * std::f32::consts::TAU;
        samples.push(phase.sin() * envelope(position, 0.06) * 0.30);
    }
    wav(&samples)
}

/// Одна музыкальная фраза: аккорд из пентатоники, взятый мягко и надолго.
///
/// Никакого ритма и никакой мелодии: это фон, который не должен ни к чему
/// звать. Случайность здесь в выборе нот, а согласованность гарантирована самой
/// пентатоникой — сфальшивить в ней нечем.
fn music_phrase(seed: u64) -> Vec<u8> {
    let seconds = 11.0;
    let mut samples = vec![0.0; (RATE as f32 * seconds) as usize];

    // Три-четыре ноты, взятые вразнобой по времени, — «дышащий» аккорд, а не
    // орган.
    let voices = 3 + (seed % 2) as usize;
    let mut pick = seed;
    for voice in 0..voices {
        pick = pick.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let step = (pick >> 33) as usize % SCALE.len();
        // Нижние голоса на октаву ниже: без баса фон звучит тонко.
        let octave = if voice == 0 { 0.5 } else { 1.0 };
        let start = (RATE as f32 * voice as f32 * 0.7) as usize;
        let length = seconds - voice as f32 * 0.7;

        let mut voice_samples = vec![0.0; (RATE as f32 * length) as usize];
        // Длинная атака — нота вплывает, а не начинается.
        note(&mut voice_samples, SCALE[step] * octave, length, 0.11, 0.45);
        for (i, value) in voice_samples.iter().enumerate() {
            if let Some(slot) = samples.get_mut(start + i) {
                *slot += value;
            }
        }
    }

    // Общий выход в тишину, чтобы фразы сшивались без стыка.
    let fade = (RATE as f32 * 1.6) as usize;
    let total = samples.len();
    for i in 0..fade.min(total) {
        samples[total - 1 - i] *= i as f32 / fade as f32;
    }
    wav(&samples)
}

// ─────────────────────────────────────────────
// Системы
// ─────────────────────────────────────────────

fn load_sounds(mut commands: Commands, mut assets: ResMut<Assets<AudioSource>>) {
    commands.insert_resource(Sounds {
        mutation: assets.add(AudioSource { bytes: mutation_sound(0).into() }),
        division: assets.add(AudioSource { bytes: division_sound().into() }),
    });
}

/// Заводит следующую фразу, когда предыдущая доиграла.
///
/// Фраза сочиняется в момент запуска, поэтому музыка не зациклена: она никогда
/// не повторит саму себя за сеанс.
fn keep_music_playing(
    mut commands: Commands,
    settings: Res<Settings>,
    time: Res<Time>,
    mut assets: ResMut<Assets<AudioSource>>,
    playing: Query<(), With<Music>>,
) {
    let gain = settings.music_gain();
    if gain <= 0.0 || !playing.is_empty() {
        return;
    }
    // Зерно из времени: у каждой фразы свой набор нот.
    let seed = (time.elapsed_secs_f64() * 1000.0) as u64;
    let phrase = assets.add(AudioSource { bytes: music_phrase(seed).into() });
    commands.spawn((
        Music,
        AudioPlayer(phrase),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(gain)),
    ));
}

/// Превращает изменения мира в звуки.
///
/// Клиент не получает «событий»: он видит состояние. Поэтому звук берётся из
/// разницы с прошлым кадром — облаков стало больше, счётчик делений вырос,
/// частей в теле прибавилось.
fn watch_events(
    mut commands: Commands,
    settings: Res<Settings>,
    sounds: Option<Res<Sounds>>,
    mut seen: ResMut<Seen>,
    mine: Query<(&PlayerProgress, &PlayerGenome), With<Controlled>>,
) {
    let Some(sounds) = sounds else { return };
    let gain = settings.effect_gain();

    // Появление облака звука не имеет намеренно. Облака возникают по всей
    // карте и постоянно, в том числе далеко от игрока: любой звук на это
    // превращался бы в непрерывное шипение ни о чём.

    let Ok((progress, genome)) = mine.single() else {
        // Тела нет — сбрасываем память, иначе после возрождения прилетит залп
        // звуков за всё, что случилось, пока мы были мертвы.
        seen.divisions = 0;
        seen.parts = 0;
        return;
    };

    if progress.divisions > seen.divisions && seen.divisions > 0 && gain > 0.0 {
        commands.spawn((
            OneShot,
            AudioPlayer(sounds.division.clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(gain)),
        ));
    }
    seen.divisions = progress.divisions;

    let parts = genome.0.parts.len();
    if parts > seen.parts && seen.parts > 0 && gain > 0.0 {
        commands.spawn((
            OneShot,
            AudioPlayer(sounds.mutation.clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(gain * 0.9)),
        ));
    }
    seen.parts = parts;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WAV должен быть настоящим WAV: иначе Bevy молча не проиграет ничего, и
    /// понять почему будет неоткуда.
    #[test]
    fn generated_wav_has_a_valid_header() {
        for bytes in [mutation_sound(3), division_sound(), music_phrase(7)] {
            assert!(bytes.len() > 44, "пустой звук");
            assert_eq!(&bytes[0..4], b"RIFF");
            assert_eq!(&bytes[8..12], b"WAVE");
            assert_eq!(&bytes[36..40], b"data");
            // Размер в заголовке обязан совпасть с тем, сколько данных реально
            // приложено.
            let declared = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]);
            assert_eq!(declared as usize, bytes.len() - 44, "размер в заголовке врёт");
        }
    }

    /// Звук не должен начинаться щелчком: это первое, что делает звук
    /// раздражающим, независимо от тембра.
    #[test]
    fn sounds_start_softly() {
        let bytes = mutation_sound(1);
        // Первые сэмплы после заголовка обязаны быть близки к нулю.
        for i in 0..8 {
            let at = 44 + i * 2;
            let sample = i16::from_le_bytes([bytes[at], bytes[at + 1]]);
            assert!(sample.abs() < 900, "звук начинается рывком: {sample}");
        }
    }

    /// Музыка строится на пентатонике, и разные зёрна должны давать разные
    /// фразы — иначе «случайная» музыка окажется одной и той же.
    #[test]
    fn music_differs_between_phrases() {
        let a = music_phrase(1);
        let b = music_phrase(999);
        assert_ne!(a, b, "музыка не меняется от фразы к фразе");
        assert_eq!(a.len(), b.len(), "длина фразы обязана быть постоянной");
    }
}
