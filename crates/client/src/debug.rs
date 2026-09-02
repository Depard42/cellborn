//! Режим отладки: F1.
//!
//! Прибор, а не украшение. Показывает две колонки — что происходит здесь и что
//! происходит на сервере, — и главное в нём то, что числа стоят рядом.
//!
//! Клиент считает **то, что до него доехало**. Если у сервера в воде девятьсот
//! частиц, а клиент видит шестьсот, это не расхождение в подсчёте, это
//! отставание репликации, и увидеть его можно только положив два числа рядом.
//! По той же причине у времени тика показан пик, а не только среднее: провал в
//! сорок миллисекунд раз в секунду среднее почти не двигает, а в игре
//! чувствуется.

use bevy::prelude::*;
use cellborn_common::*;
use lightyear::prelude::*;

use crate::menu::Screen;
use crate::ui::UiFont;

/// Окно, по которому усредняются кадры. Достаточно длинное, чтобы число не
/// прыгало, достаточно короткое, чтобы просадка была видна.
const WINDOW: f32 = 0.5;

/// Последняя сводка, присланная сервером, и когда она пришла.
#[derive(Resource, Default)]
pub struct ServerSummary {
    pub stats: Option<ServerStats>,
    pub age: f32,
}

/// Кадры, посчитанные тем же способом, что и тики на сервере.
#[derive(Resource, Default)]
pub struct FrameClock {
    pub fps: f32,
    pub frame_ms: f32,
    pub peak_ms: f32,
    frames: u32,
    sum_ms: f32,
    window_peak_ms: f32,
    elapsed: f32,
}

#[derive(Resource, Default)]
pub struct DebugOverlay {
    pub shown: bool,
}

#[derive(Component)]
struct DebugPanel;

#[derive(Component)]
struct DebugText;

pub fn plugin(app: &mut App) {
    app.init_resource::<ServerSummary>();
    app.init_resource::<FrameClock>();
    app.init_resource::<DebugOverlay>();
    app.add_systems(Startup, setup_overlay);
    // Приём сообщений и счёт кадров идут всегда: к моменту, когда игрок нажмёт
    // F1, числа уже должны быть настоящими, а не наполняться полсекунды.
    app.add_systems(Update, (receive_world, receive_stats, measure_frames).chain());
    // А переключение — только в игре. В меню F1 уже занят справочником, и
    // отнимать у него привычную клавишу ради отладки неправильно.
    app.add_systems(Update, (toggle, update_overlay).chain().run_if(in_state(Screen::Game)));
    app.add_systems(OnExit(Screen::Game), hide_overlay);
}

/// Панель создаётся один раз и живёт всё время работы клиента: ей нечего
/// пересоздавать при каждом входе в игру.
fn setup_overlay(mut commands: Commands, font: Res<UiFont>) {
    commands
        .spawn((
            DebugPanel,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                right: Val::Px(16.0),
                min_width: Val::Px(310.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.04, 0.06, 0.86)),
        ))
        .with_children(|panel| {
            panel.spawn((
                DebugText,
                Text::new(""),
                TextFont {
                    font: FontSource::Handle(font.0.clone()),
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.72, 0.90, 0.86)),
            ));
        });
}

/// Выход в меню прячет панель: поверх меню она только мешает, а F1 там значит
/// другое.
fn hide_overlay(mut overlay: ResMut<DebugOverlay>, mut panel: Query<&mut Node, With<DebugPanel>>) {
    overlay.shown = false;
    for mut node in &mut panel {
        node.display = Display::None;
    }
}

fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut overlay: ResMut<DebugOverlay>,
    mut panel: Query<&mut Node, With<DebugPanel>>,
) {
    if !keys.just_pressed(KeyCode::F1) {
        return;
    }
    overlay.shown = !overlay.shown;
    for mut node in &mut panel {
        node.display = if overlay.shown { Display::Flex } else { Display::None };
    }
}

/// Вода вокруг игрока приходит сообщением, а не компонентом на каждом организме.
fn receive_world(
    mut receivers: Query<&mut MessageReceiver<WorldUpdate>>,
    mut world: ResMut<WorldUpdate>,
) {
    for mut receiver in &mut receivers {
        for update in receiver.receive() {
            *world = update;
        }
    }
}

fn receive_stats(
    time: Res<Time>,
    mut receivers: Query<&mut MessageReceiver<ServerStats>>,
    mut summary: ResMut<ServerSummary>,
) {
    summary.age += time.delta_secs();
    for mut receiver in &mut receivers {
        for stats in receiver.receive() {
            summary.stats = Some(stats);
            summary.age = 0.0;
        }
    }
}

/// Считает кадры всегда, а не только при открытой панели: иначе первое, что
/// увидит игрок, нажав F1, — это ноль, который потом полсекунды наполняется.
fn measure_frames(time: Res<Time<Real>>, mut clock: ResMut<FrameClock>) {
    let ms = time.delta_secs() * 1000.0;
    clock.frames += 1;
    clock.sum_ms += ms;
    clock.window_peak_ms = clock.window_peak_ms.max(ms);
    clock.elapsed += time.delta_secs();
    if clock.elapsed >= WINDOW {
        clock.frame_ms = clock.sum_ms / clock.frames.max(1) as f32;
        clock.fps = clock.frames as f32 / clock.elapsed;
        clock.peak_ms = clock.window_peak_ms;
        clock.frames = 0;
        clock.sum_ms = 0.0;
        clock.window_peak_ms = 0.0;
        clock.elapsed = 0.0;
    }
}

#[allow(clippy::too_many_arguments)]
fn update_overlay(
    overlay: Res<DebugOverlay>,
    clock: Res<FrameClock>,
    summary: Res<ServerSummary>,
    world: Res<WorldUpdate>,
    connection: Query<&PingManager, With<Client>>,
    food: Query<(), With<Nutrient>>,
    organisms: Query<(), With<PlayerPosition>>,
    clouds: Query<(), With<ToxinCloud>>,
    mut text: Query<&mut Text, With<DebugText>>,
) {
    if !overlay.shown {
        return;
    }
    let Ok(mut text) = text.single_mut() else { return };

    let (food_seen, organisms_seen, clouds_seen) =
        (food.iter().count(), organisms.iter().count(), clouds.iter().count());
    let ping = connection
        .single()
        .map(|ping| format!("{:.0} мс", ping.rtt().as_secs_f32() * 1000.0))
        .unwrap_or_else(|_| "нет".to_string());

    let mut lines = format!(
        "F1 — отладка   {}\n\
         \n\
         КЛИЕНТ\n\
         кадров/с      {:.0}   ({:.1} мс, пик {:.1})\n\
         организмов    {organisms_seen}\n\
         еды           {food_seen}\n\
         облаков       {clouds_seen}\n\
         пинг          {ping}\n\
         биом          {}   яд {:.2}\n",
        version::full(),
        clock.fps,
        clock.frame_ms,
        clock.peak_ms,
        world.biome.name(),
        world.toxin,
    );

    match summary.stats {
        // Сводка приходит два раза в секунду; если её нет секунды с лишним —
        // молчит не оверлей, а сервер, и это само по себе сообщение.
        Some(stats) if summary.age < 2.0 => {
            let budget = tick_duration().as_secs_f32() * 1000.0;
            lines.push_str(&format!(
                "\nСЕРВЕР\n\
                 тик           {:.2} мс   сред {:.2}   пик {:.2}\n\
                 бюджет тика   {:.1} мс   занято {:.0}%\n\
                 тиков/с       {:.1}\n\
                 организмов    {}{}\n\
                 еды           {}{}\n\
                 облаков       {}\n\
                 клиентов      {}\n",
                stats.tick_ms,
                stats.tick_ms_avg,
                stats.tick_ms_peak,
                budget,
                stats.tick_ms_avg / budget * 100.0,
                stats.ticks_per_second,
                stats.organisms,
                behind(stats.organisms as i64, organisms_seen as i64),
                stats.food,
                behind(stats.food as i64, food_seen as i64),
                stats.clouds,
                stats.clients,
            ));
        }
        Some(_) => lines.push_str("\nСЕРВЕР\nсводка не приходит\n"),
        None => lines.push_str("\nСЕРВЕР\nждём сводку\n"),
    }

    if text.0 != lines {
        text.0 = lines;
    }
}

/// Насколько клиент отстал от сервера по числу сущностей.
///
/// Ноль не показывается: в норме числа совпадают, и строка не должна шуметь.
fn behind(server: i64, client: i64) -> String {
    let delta = client - server;
    if delta == 0 {
        String::new()
    } else {
        format!("   (у клиента {delta:+})")
    }
}
