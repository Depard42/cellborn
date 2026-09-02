//! Гербы родов и таблица родов.
//!
//! Род — главная единица этой игры: он наследуется, он определяет, кто с кем
//! дерётся, в него возвращается погибший игрок. Но до сих пор его нельзя было
//! ни увидеть, ни сосчитать: все чужие клетки выглядели одинаково, а
//! принадлежность к роду читалась только по цвету «свой-чужой» относительно
//! себя.
//!
//! Герб чинит первое, таблица — второе.
//!
//! **Герб выводится из числа рода, а не хранится.** Род — это `u64`, одинаковый
//! у сервера и у всех клиентов, поэтому одна и та же клетка получает один и тот
//! же герб у всех, и пересылать по сети ничего не нужно.

use bevy::prelude::*;
use cellborn_common::*;
use lightyear::prelude::*;

use crate::menu::Screen;
use crate::ui::{GameUi, UiFont};

/// Сколько цветов в палитре гербов.
///
/// Достаточно, чтобы соседние роды почти никогда не совпали, и мало, чтобы
/// каждый цвет оставался различимым в мутной воде.
const CREST_COLORS: usize = 12;

/// Формы гербов. Символ, а не текстура: он должен читаться размером в
/// несколько пикселей над телом.
const CREST_SHAPES: [&str; 8] = ["◆", "●", "▲", "■", "✦", "✚", "▼", "◗"];

/// Герб рода: цвет и знак, выведенные из его номера.
#[derive(Clone, Copy, PartialEq)]
pub struct Crest {
    pub color: Color,
    pub shape: &'static str,
}

impl Crest {
    pub fn of(lineage: u64) -> Self {
        // Перемешиваем биты, иначе соседние номера родов дают соседние цвета, а
        // роды нумеруются подряд у ботов, рождённых одной пачкой.
        let mixed = lineage
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .rotate_left(31)
            .wrapping_mul(0xBF58_476D_1CE4_E5B9);

        let hue = (mixed % CREST_COLORS as u64) as f32 / CREST_COLORS as f32 * 360.0;
        let shape = CREST_SHAPES[((mixed >> 20) % CREST_SHAPES.len() as u64) as usize];
        Self {
            // Насыщенный и светлый: герб должен быть виден сквозь муть и на
            // фоне собственного тела.
            color: Color::hsl(hue, 0.72, 0.62),
            shape,
        }
    }

    /// Порядковый номер знака: по нему выбирается и силуэт метки в мире.
    pub fn shape_index(&self) -> usize {
        CREST_SHAPES.iter().position(|s| *s == self.shape).unwrap_or(0)
    }

    /// Знак вместе с номером рода — для таблицы, где важно ещё и различать
    /// роды с одинаковым знаком.
    pub fn label(&self, lineage: u64) -> String {
        format!("{} {:04X}", self.shape, (lineage >> 48) as u16)
    }
}

/// Строка таблицы родов.
pub struct Line {
    pub lineage: u64,
    pub count: usize,
    /// Самый развитый представитель: по нему видно, куда род идёт.
    pub best_parts: usize,
    pub best_generation: u16,
    pub total_mass: f32,
    pub is_mine: bool,
}

/// Собирает таблицу родов из того, что клиент и так видит.
///
/// Считается на клиенте намеренно: все геномы и так реплицированы, и просить у
/// сервера то, что лежит под рукой, значило бы гонять по сети данные ради
/// экрана, который открыт несколько секунд в час.
pub fn tally(organisms: &[(&PlayerGenome, &PlayerVitals)], mine: Option<u64>) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    for (genome, vitals) in organisms {
        let lineage = genome.0.lineage;
        match lines.iter_mut().find(|line| line.lineage == lineage) {
            Some(line) => {
                line.count += 1;
                line.best_parts = line.best_parts.max(genome.0.parts.len());
                line.best_generation = line.best_generation.max(genome.0.generation);
                line.total_mass += vitals.mass;
            }
            None => lines.push(Line {
                lineage,
                count: 1,
                best_parts: genome.0.parts.len(),
                best_generation: genome.0.generation,
                total_mass: vitals.mass,
                is_mine: Some(lineage) == mine,
            }),
        }
    }
    // Сильнейшие сверху: сначала по числу тел, при равенстве — по развитию.
    lines.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(b.best_parts.cmp(&a.best_parts))
            .then(b.total_mass.total_cmp(&a.total_mass))
    });
    lines
}

// ─────────────────────────────────────────────
// Герб над телом
// ─────────────────────────────────────────────

/// Метка герба, висящая над клеткой рядом с полоской здоровья.
#[derive(Component)]
pub struct CrestMark;

/// Форма герба в виде меша: знак из [`CREST_SHAPES`] нарисовать над телом
/// нечем, но различать роды надо и в бою, а не только в таблице.
///
/// Поэтому в мире род опознаётся парой «цвет + силуэт»: восемь форм на
/// двенадцать цветов дают почти сотню различимых сочетаний, чего с запасом
/// хватает на любое море.
pub fn crest_mesh(lineage: u64, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
    let index = (Crest::of(lineage).shape_index()) % 4;
    match index {
        0 => meshes.add(Sphere::new(0.5).mesh().uv(10, 7)),
        1 => meshes.add(Cuboid::new(0.8, 0.8, 0.8)),
        2 => meshes.add(Cone { radius: 0.5, height: 0.9 }),
        _ => meshes.add(Capsule3d::new(0.32, 0.6).mesh().latitudes(4).longitudes(8)),
    }
}

// ─────────────────────────────────────────────
// Таблица родов
// ─────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct Leaderboard {
    pub shown: bool,
}

#[derive(Component)]
struct LeaderboardPanel;

#[derive(Component)]
struct LeaderboardText;

/// Постоянная табличка вверху справа: три сильнейших рода.
#[derive(Component)]
struct TopBoard;

pub fn plugin(app: &mut App) {
    app.init_resource::<Leaderboard>();
    app.add_systems(OnEnter(Screen::Game), (setup_leaderboard, setup_top_board));
    app.add_systems(
        Update,
        (toggle_leaderboard, update_leaderboard, update_top_board)
            .chain()
            .run_if(in_state(Screen::Game)),
    );
}

/// Табличка лидеров всегда на экране.
///
/// Как в агарио: три верхние строки видно постоянно, без нажатий. Смысл в том,
/// чтобы игрок в любой момент знал, догоняет он или отстаёт, — цель «стать
/// сильнейшим» существует, только если видно, кто сейчас сильнейший.
fn setup_top_board(mut commands: Commands, font: Res<UiFont>) {
    commands
        .spawn((
            GameUi,
            TopBoard,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                right: Val::Px(16.0),
                min_width: Val::Px(210.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(9.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.08, 0.10, 0.72)),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new(""),
                TextFont {
                    font: FontSource::Handle(font.0.clone()),
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.88, 0.95, 0.93)),
            ));
        });
}

fn update_top_board(
    organisms: Query<(&PlayerGenome, &PlayerVitals)>,
    mine: Query<&PlayerGenome, With<Controlled>>,
    board: Query<&Children, With<TopBoard>>,
    mut text: Query<&mut Text>,
) {
    let Ok(children) = board.single() else { return };
    let all: Vec<(&PlayerGenome, &PlayerVitals)> = organisms.iter().collect();
    if all.is_empty() {
        return;
    }
    let my_lineage = mine.single().ok().map(|g| g.0.lineage);
    let lines = tally(&all, my_lineage);

    let mut out = String::from("СИЛЬНЕЙШИЕ РОДА       L — все
");
    for (place, line) in lines.iter().take(3).enumerate() {
        let crest = Crest::of(line.lineage);
        out.push_str(&format!(
            "{}. {}  ×{}  до {} частей{}
",
            place + 1,
            crest.label(line.lineage),
            line.count,
            line.best_parts,
            if line.is_mine { "  ←" } else { "" }
        ));
    }
    // Своё место, если оно не в тройке: без этого табличка показывает чужой
    // успех и молчит о твоём.
    if let Some(mine) = my_lineage {
        if let Some(place) = lines.iter().position(|l| l.lineage == mine) {
            if place >= 3 {
                let line = &lines[place];
                out.push_str(&format!(
                    "…
{}. {}  ×{}  до {} частей  ←
",
                    place + 1,
                    Crest::of(mine).label(mine),
                    line.count,
                    line.best_parts
                ));
            }
        }
    }

    for child in children.iter() {
        let Ok(mut text) = text.get_mut(child) else { continue };
        if text.0 != out {
            text.0 = out.clone();
        }
    }
}

fn setup_leaderboard(mut commands: Commands, font: Res<UiFont>) {
    commands
        .spawn((
            GameUi,
            LeaderboardPanel,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(120.0),
                left: Val::Px(16.0),
                min_width: Val::Px(330.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.08, 0.10, 0.90)),
        ))
        .with_children(|panel| {
            panel.spawn((
                LeaderboardText,
                Text::new(""),
                TextFont {
                    font: FontSource::Handle(font.0.clone()),
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.86, 0.94, 0.92)),
            ));
        });
}

fn toggle_leaderboard(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut board: ResMut<Leaderboard>,
    mut panel: Query<&mut Node, With<LeaderboardPanel>>,
) {
    let pad = pads.iter().any(|pad| pad.just_pressed(GamepadButton::Start));
    if !keys.just_pressed(KeyCode::KeyL) && !pad {
        return;
    }
    board.shown = !board.shown;
    for mut node in &mut panel {
        node.display = if board.shown { Display::Flex } else { Display::None };
    }
}

fn update_leaderboard(
    board: Res<Leaderboard>,
    organisms: Query<(&PlayerGenome, &PlayerVitals)>,
    mine: Query<&PlayerGenome, With<Controlled>>,
    mut text: Query<&mut Text, With<LeaderboardText>>,
) {
    if !board.shown {
        return;
    }
    let Ok(mut text) = text.single_mut() else { return };

    let all: Vec<(&PlayerGenome, &PlayerVitals)> = organisms.iter().collect();
    let mine = mine.single().ok().map(|g| g.0.lineage);
    let lines = tally(&all, mine);

    let mut out = format!("РОДА В МОРЕ — {} (L закрывает)\n", lines.len());

    // Не сводка, а разбор: у каждого рода показан его сильнейший представитель
    // — из чего он собран и чем опасен. Одних чисел мало, чтобы понять, с кем
    // имеешь дело.
    for line in lines.iter().take(8) {
        let crest = Crest::of(line.lineage);
        out.push_str(&format!(
            "\n─── {} ───{}\n  тел {}   поколение до {}   общая масса {:.0}\n",
            crest.label(line.lineage),
            if line.is_mine { "  ← твой род" } else { "" },
            line.count,
            line.best_generation,
            line.total_mass,
        ));

        // Сильнейший представитель: тот, у кого больше всего органов.
        let champion = all
            .iter()
            .filter(|(genome, _)| genome.0.lineage == line.lineage)
            .max_by_key(|(genome, _)| genome.0.parts.len());
        if let Some((genome, vitals)) = champion {
            let body = OrganismState::from_genome(genome.0.clone());
            out.push_str(&format!(
                "  сильнейший: {} частей, масса {:.0}, урон {:.1}/с, защита {:.0}%\n",
                genome.0.parts.len(),
                vitals.mass,
                attack_power(&body),
                defense(&body) * 100.0,
            ));

            // Из чего он собран: самые многочисленные органы, как в справочнике.
            let mut organs: Vec<(PartFamily, u8)> = PartFamily::ALL
                .into_iter()
                .map(|family| (family, body.families.get(family)))
                .filter(|(_, count)| *count > 0)
                .collect();
            organs.sort_by(|a, b| b.1.cmp(&a.1));
            let build = organs
                .iter()
                .take(4)
                .map(|(family, count)| format!("{} ×{count}", family.name()))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("  строение: {build}\n"));
        }
    }
    if lines.len() > 8 {
        out.push_str(&format!("\n…и ещё {} родов помельче", lines.len() - 8));
    }

    if text.0 != out {
        text.0 = out;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Герб обязан быть одинаковым везде и разным у разных родов, иначе он не
    /// опознавательный знак, а украшение.
    #[test]
    fn a_crest_is_stable_and_spread_out() {
        assert_eq!(Crest::of(42).shape, Crest::of(42).shape);
        assert_eq!(Crest::of(42).color, Crest::of(42).color);

        // Роды у ботов рождаются подряд, и именно на подряд идущих номерах
        // наивный герб давал бы почти одинаковые цвета.
        let spread: std::collections::HashSet<&str> =
            (0..40u64).map(|n| Crest::of(n).shape).collect();
        assert!(spread.len() >= 6, "знаки почти не различаются: {}", spread.len());

        let colors: std::collections::HashSet<String> =
            (0..40u64).map(|n| format!("{:?}", Crest::of(n).color)).collect();
        assert!(colors.len() >= 8, "цвета почти не различаются: {}", colors.len());
    }

    /// Своё место должно быть видно, даже когда оно не в тройке: иначе
    /// табличка показывает чужой успех и молчит о твоём.
    #[test]
    fn your_own_lineage_is_findable_even_when_it_is_losing() {
        let vitals = PlayerVitals { mass: 10.0, health: 100.0 };
        let mut genomes = Vec::new();
        // Четыре чужих рода по нескольку тел и свой — одним.
        for lineage in 1..=4u64 {
            for _ in 0..(6 - lineage) {
                genomes.push(PlayerGenome(Genome::starter_of(lineage)));
            }
        }
        genomes.push(PlayerGenome(Genome::starter_of(99)));

        let rows: Vec<(&PlayerGenome, &PlayerVitals)> =
            genomes.iter().map(|g| (g, &vitals)).collect();
        let lines = tally(&rows, Some(99));

        let place = lines.iter().position(|l| l.is_mine).expect("свой род потерялся");
        assert!(place >= 3, "предпосылка теста неверна: свой род и так в тройке");
        assert_eq!(lines[place].count, 1);
    }

    /// Таблица должна складывать роды, а не тела, и ставить сильнейших сверху.
    #[test]
    fn the_board_groups_by_lineage_and_ranks_the_strongest_first() {
        let mut small = PlayerGenome(Genome::starter_of(1));
        small.0.generation = 2;
        let mut big = PlayerGenome(Genome::starter_of(7));
        big.0.push_part(PartKind::basic(PartFamily::Spike));

        let vitals = PlayerVitals { mass: 10.0, health: 100.0 };
        let rows: Vec<(&PlayerGenome, &PlayerVitals)> =
            vec![(&small, &vitals), (&big, &vitals), (&big, &vitals)];

        let lines = tally(&rows, Some(1));
        assert_eq!(lines.len(), 2, "роды не сгруппированы");
        assert_eq!(lines[0].lineage, 7, "многочисленный род не первый");
        assert_eq!(lines[0].count, 2);
        assert_eq!(lines[0].best_parts, 4);
        assert!(lines[1].is_mine, "свой род не отмечен");
    }
}
