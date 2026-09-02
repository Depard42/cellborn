//! HUD, mutation panel and death overlay.

use bevy::prelude::*;
use cellborn_common::*;
use lightyear::prelude::client::*;
use lightyear::prelude::*;

use crate::ServerAddress;

/// The font bundled with Bevy has no Cyrillic glyphs, so the UI ships its own.
///
/// It is compiled into the binary rather than loaded from `assets/`: Bevy resolves
/// the asset root relative to the executable, which differs between `cargo run` and
/// running the binary directly, and a UI with no text is a bad failure mode.
#[derive(Resource)]
pub struct UiFont(pub Handle<Font>);

/// Иконки интерфейса.
///
/// Обычные файлы в `assets/ui`, а не вшитые в бинарник: их рисует
/// `scripts/make-icons.py`, и перерисовать иконку должно быть можно без
/// пересборки игры.
///
/// Шрифт при этом остаётся вшитым, и это не непоследовательность: пропавшая
/// иконка — это интерфейс без картинки, а пропавший шрифт — интерфейс без
/// единой буквы, причём молча.
#[derive(Resource)]
pub struct UiIcons {
    pub energy: Handle<Image>,
    pub health: Handle<Image>,
    pub mass: Handle<Image>,
    pub points: Handle<Image>,
    pub mutation: Handle<Image>,
    pub danger: Handle<Image>,
    pub attack: Handle<Image>,
    pub speed: Handle<Image>,
    pub defense: Handle<Image>,
    pub resist: Handle<Image>,
}

pub fn load_icons(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(UiIcons {
        energy: assets.load("ui/energy.png"),
        health: assets.load("ui/health.png"),
        mass: assets.load("ui/mass.png"),
        points: assets.load("ui/points.png"),
        mutation: assets.load("ui/mutation.png"),
        danger: assets.load("ui/danger.png"),
        attack: assets.load("ui/attack.png"),
        speed: assets.load("ui/speed.png"),
        defense: assets.load("ui/defense.png"),
        resist: assets.load("ui/resist.png"),
    });
}

/// Иконка перед строкой: маленькая, ровно в высоту текста.
fn icon(image: &Handle<Image>, size: f32) -> impl Bundle {
    (
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            margin: UiRect::right(Val::Px(6.0)),
            flex_shrink: 0.0,
            ..default()
        },
        ImageNode::new(image.clone()),
    )
}

const FONT_BYTES: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSans.ttf");

/// Ставит шрифт до запуска систем.
///
/// Не системой в `Startup`: первый переход состояний происходит раньше него, и
/// `OnEnter(Menu)` не нашёл бы ресурс.
pub fn install_font(app: &mut App) {
    let handle = app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .add(Font::from_bytes(FONT_BYTES.to_vec()));
    app.insert_resource(UiFont(handle));
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum HudLine {
    Status,
    Vitals,
    Environment,
}

/// Боевые и ходовые показатели тела — то, что игрок сравнивает с чужими.
///
/// Считаются на клиенте из генома: сервер их не присылает, потому что и не
/// должен — это чистая функция от тела, и обе стороны считают её одинаково.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum HudStat {
    Mass,
    Attack,
    Speed,
    Defense,
    /// Стойкости к среде одной строкой: их четыре, и по отдельности они
    /// занимают больше места, чем значат.
    Resist,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum HudBar {
    Energy,
    Health,
}

/// Всё, что принадлежит игровому интерфейсу и исчезает при выходе в меню.
#[derive(Component)]
pub struct GameUi;

#[derive(Component)]
pub struct MutationPanel;


/// Which organ family the mutation panel is showing.
///
/// 200 mutations do not fit on screen as a flat grid, and a scrolling wall of
/// cards is unreadable anyway. The panel shows one family at a time: 20 tabs,
/// 10 variants each.
#[derive(Resource, Default)]
pub struct MutationSelection {
    pub family: usize,
}

#[derive(Component)]
pub struct FamilyTab {
    pub index: usize,
}

/// Description of the organ the panel is currently showing.
#[derive(Component)]
pub struct FamilyHeader;

#[derive(Component)]
pub struct VariantCard {
    pub variant: usize,
}

#[derive(Component)]
pub struct DeathOverlay;

const PANEL: Color = Color::srgba(0.02, 0.07, 0.09, 0.80);
/// Цифры показателей ярче подписей: на них смотрят чаще всего.
const INK_STAT: Color = Color::srgb(0.93, 0.97, 0.96);
const LABEL: Color = Color::srgb(0.74, 0.88, 0.90);

pub fn setup_hud(mut commands: Commands, font: Res<UiFont>, icons: Res<UiIcons>) {
    let font = &font.0;
    // Vitals panel, top left.
    commands
        .spawn((
            GameUi,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                left: Val::Px(16.0),
                width: Val::Px(350.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(12.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("Connecting..."),
                TextFont { font: FontSource::Handle(font.clone()), font_size: FontSize::Px(13.0), ..default() },
                TextColor(Color::srgb(0.95, 0.75, 0.35)),
                HudLine::Status,
            ));
            bar(panel, HudBar::Energy, Color::srgb(0.35, 0.85, 0.65), &icons.energy);
            bar(panel, HudBar::Health, Color::srgb(0.85, 0.35, 0.40), &icons.health);
            panel.spawn((
                Text::new("-"),
                TextFont { font: FontSource::Handle(font.clone()), font_size: FontSize::Px(13.0), ..default() },
                TextColor(LABEL),
                HudLine::Vitals,
            ));

            // Показатели тела: значок и число. Ряды по два, чтобы панель не
            // вытягивалась в столбец на пол-экрана.
            for row in [
                [(HudStat::Mass, &icons.mass), (HudStat::Attack, &icons.attack)],
                [(HudStat::Speed, &icons.speed), (HudStat::Defense, &icons.defense)],
            ] {
                panel
                    .spawn(Node {
                        column_gap: Val::Px(18.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|line| {
                        for (stat, image) in row {
                            line.spawn(Node { align_items: AlignItems::Center, ..default() })
                                .with_children(|cell| {
                                    cell.spawn(icon(image, 14.0));
                                    cell.spawn((
                                        stat,
                                        Text::new("-"),
                                        TextFont {
                                            font: FontSource::Handle(font.clone()),
                                            font_size: FontSize::Px(13.0),
                                            ..default()
                                        },
                                        TextColor(INK_STAT),
                                    ));
                                });
                        }
                    });
            }

            // Стойкости отдельной строкой: их четыре, и рядом с ними важно
            // видеть, что творится в воде прямо сейчас.
            panel
                .spawn(Node { align_items: AlignItems::Center, ..default() })
                .with_children(|line| {
                    line.spawn(icon(&icons.resist, 14.0));
                    line.spawn((
                        HudStat::Resist,
                        Text::new("-"),
                        TextFont {
                            font: FontSource::Handle(font.clone()),
                            font_size: FontSize::Px(12.0),
                            ..default()
                        },
                        TextColor(LABEL),
                    ));
                });
            panel.spawn((
                Text::new("-"),
                TextFont { font: FontSource::Handle(font.clone()), font_size: FontSize::Px(13.0), ..default() },
                TextColor(LABEL),
                HudLine::Environment,
            ));
        });

    commands.spawn((
        GameUi,
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(14.0),
            left: Val::Px(16.0),
            ..default()
        },
        Text::new(
            "WASD - плыть   Tab - мутации   Q/E - орган   1-0 - вырастить   F1 - отладка",
        ),
        TextFont { font: FontSource::Handle(font.clone()), font_size: FontSize::Px(12.0), ..default() },
        TextColor(Color::srgba(0.74, 0.88, 0.90, 0.55)),
    ));

    // Death overlay, hidden until it is needed.
    commands.spawn((
        GameUi,
        DeathOverlay,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(42.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            display: Display::None,
            ..default()
        },
        Text::new(""),
        TextFont { font: FontSource::Handle(font.clone()), font_size: FontSize::Px(26.0), ..default() },
        TextColor(Color::srgb(0.95, 0.45, 0.45)),
    ));

    // Mutation panel, hidden until Tab.
    commands
        .spawn((
            GameUi,
            MutationPanel,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(40.0),
                left: Val::Px(16.0),
                right: Val::Px(16.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(10.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .with_children(|panel| {
            // Family tabs: Q / E cycle them, click picks one.
            panel
                .spawn(Node {
                    flex_wrap: FlexWrap::Wrap,
                    align_items: AlignItems::FlexStart,
                    column_gap: Val::Px(4.0),
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|tabs| {
                    for (index, family) in PartFamily::ALL.iter().enumerate() {
                        tabs.spawn((
                            FamilyTab { index },
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.06)),
                            Text::new(family.name()),
                            TextFont { font: FontSource::Handle(font.clone()), font_size: FontSize::Px(11.0), ..default() },
                            TextColor(Color::srgb(0.88, 0.94, 0.94)),
                        ));
                    }
                });

            // What this organ is for, before the ten ways of growing it.
            panel.spawn((
                FamilyHeader,
                Text::new(""),
                TextFont { font: FontSource::Handle(font.clone()), font_size: FontSize::Px(12.0), ..default() },
                TextColor(Color::srgb(0.62, 0.88, 0.82)),
            ));

            // Variants of the selected family. Text is rewritten on every switch.
            panel
                .spawn(Node {
                    flex_wrap: FlexWrap::Wrap,
                    // Without this the cards stretch to the tallest one and the
                    // panel swallows the screen.
                    align_items: AlignItems::FlexStart,
                    column_gap: Val::Px(5.0),
                    row_gap: Val::Px(5.0),
                    ..default()
                })
                .with_children(|cards| {
                    // Четыре уровня плюс карточка прокачки: последняя не растит
                    // новый орган, а поднимает уже отращённый.
                    for variant in 0..PartLevel::ALL.len() + 1 {
                        cards.spawn((
                            VariantCard { variant },
                            Button,
                            Node {
                                width: Val::Px(232.0),
                                min_height: Val::Px(66.0),
                                padding: UiRect::all(Val::Px(6.0)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.06)),
                            Text::new(""),
                            TextFont { font: FontSource::Handle(font.clone()), font_size: FontSize::Px(11.0), ..default() },
                            TextColor(Color::srgb(0.92, 0.96, 0.96)),
                        ));
                    }
                });
        });
}

/// Полоса с иконкой слева.
///
/// Иконка вместо подписи: «капля» и «сердце» читаются мгновенно и на любом
/// языке, а слово «Энергия» надо прочесть. Игровой интерфейс не должен
/// заставлять читать то, что можно узнать.
fn bar(panel: &mut ChildSpawnerCommands, kind: HudBar, color: Color, image: &Handle<Image>) {
    panel
        .spawn(Node {
            width: Val::Percent(100.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn(icon(image, 15.0));
            row.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(11.0),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    padding: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
                // Рамка делает полосу предметом, а не заливкой: без неё пустая
                // шкала сливается с панелью и кажется, что её нет.
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.16)),
            ))
                .with_children(|track| {
                    track.spawn((
                        Node {
                            width: Val::Percent(0.0),
                            height: Val::Percent(100.0),
                            border_radius: BorderRadius::all(Val::Px(5.0)),
                            ..default()
                        },
                        BackgroundColor(color),
                        kind,
                    ));
                });
        });
}

/// Своё тело глазами интерфейса. Энергия отдельным компонентом: её носят только
/// организмы игроков, поэтому здесь она `Option` — в первый тик после появления
/// тела она может ещё не доехать.
type PlayerView<'a> =
    (&'a PlayerVitals, Option<&'a PlayerEnergy>, &'a PlayerProgress, &'a PlayerGenome);

#[allow(clippy::too_many_arguments)]
pub fn update_hud(
    server: Res<ServerAddress>,
    water: Res<WorldUpdate>,
    connection: Query<(), (With<Client>, With<Connected>)>,
    player: Query<PlayerView, With<Controlled>>,
    everyone: Query<&PlayerGenome>,
    mut lines: Query<(&mut Text, &mut TextColor, &HudLine)>,
    mut stats: Query<(&mut Text, &HudStat), Without<HudLine>>,
    mut bars: Query<(&mut Node, &HudBar)>,
) {
    let connected = !connection.is_empty();
    let view = player.single().ok();
    // Kin are everyone sharing our lineage: the colony we can fall back into.
    let kin_count = view
        .map(|v| everyone.iter().filter(|g| g.0.lineage == v.3.0.lineage).count())
        .unwrap_or(0);

    for (mut text, mut color, line) in &mut lines {
        let value = match line {
            HudLine::Status => match (connected, view.is_some()) {
                (false, _) => format!("Подключение к {}...", server.0),
                (true, false) => format!("Подключено к {} - ждём организм", server.0),
                (true, true) => {
                    let (progress, genome) = (view.unwrap().2, view.unwrap().3);
                    format!(
                        "Поколение {}   очки: {}   частей: {}/{}   убийств: {}   родня: {}",
                        genome.0.generation,
                        progress.points,
                        genome.0.parts.len(),
                        progress.max_parts,
                        progress.kills,
                        kin_count.saturating_sub(1)
                    )
                }
            },
            HudLine::Vitals => match view {
                Some((v, energy, _, _)) => {
                    let (energy, cap) =
                        energy.map(|e| (e.energy, e.cap)).unwrap_or((0.0, 0.0));
                    format!(
                        "Энергия {energy:.0}/{cap:.0}   Здоровье {:.0}/{:.0}   Масса {:.1}",
                        v.health,
                        max_health(v.mass),
                        v.mass
                    )
                }
                None => "-".to_string(),
            },
            // Вода — общая для всех и приходит сообщением, а не компонентом на
            // каждом организме: рисуем её независимо от того, есть ли уже тело.
            // Вода как она есть. Рядом со строкой стойкостей это читается
            // парой: «вот что снаружи, вот что я терплю».
            HudLine::Environment => format!(
                "ВОДА: {}\nT {:.2}   соль {:.2}   O2 {:.2}   яд {:.2}",
                water.biome.name(),
                water.temperature,
                water.salinity,
                water.oxygen,
                water.toxin
            ),
        };
        if text.0 != value {
            text.0 = value;
        }
        if *line == HudLine::Status {
            color.0 = if connected && view.is_some() {
                Color::srgb(0.45, 0.85, 0.65)
            } else {
                Color::srgb(0.95, 0.75, 0.35)
            };
        }
    }

    // Показатели тела. Считаются здесь же из генома: это чистая функция от
    // тела, и просить их у сервера значило бы гонять по сети то, что клиент
    // умеет вычислить сам.
    let body = view.map(|v| OrganismState::from_genome(v.3.0.clone()));
    for (mut text, stat) in &mut stats {
        let value = match (&body, stat) {
            (Some(body), HudStat::Mass) => format!("{:.1}", body.mass),
            (Some(body), HudStat::Attack) => format!("{:.1}/с", attack_power(body)),
            (Some(body), HudStat::Speed) => format!("{:.1}", movement_speed(body)),
            // Защита — доля поглощаемого урона, и в процентах она понятнее.
            (Some(body), HudStat::Defense) => format!("{:.0}%", defense(body) * 100.0),
            (Some(body), HudStat::Resist) => format!(
                "T {:.2}   соль {:.2}   O2 {:+.2}   яд {:.2}",
                body.temperature_tolerance,
                body.salinity_tolerance,
                body.oxygen_affinity,
                body.toxin_resistance,
            ),
            (None, _) => "-".to_string(),
        };
        if text.0 != value {
            text.0 = value;
        }
    }

    for (mut node, kind) in &mut bars {
        let fill = match (view, kind) {
            (Some((_, Some(energy), _, _)), HudBar::Energy) => {
                (energy.energy / energy.cap.max(1.0) * 100.0).clamp(0.0, 100.0)
            }
            (Some((v, _, _, _)), HudBar::Health) => {
                (v.health / max_health(v.mass).max(1.0) * 100.0).clamp(0.0, 100.0)
            }
            _ => 0.0,
        };
        node.width = Val::Percent(fill);
    }
}

pub fn update_death_overlay(
    player: Query<&PlayerProgress, With<Controlled>>,
    mut overlay: Query<(&mut Node, &mut Text), With<DeathOverlay>>,
) {
    let Ok((mut node, mut text)) = overlay.single_mut() else { return; };
    match player.single() {
        Ok(progress) if progress.dead => {
            node.display = Display::Flex;
            let value = format!("Организм погиб. Возрождение через {:.0}", progress.respawn_in.max(0.0));
            if text.0 != value {
                text.0 = value;
            }
        }
        _ => node.display = Display::None,
    }
}

pub fn toggle_mutation_panel(
    keys: Res<ButtonInput<KeyCode>>,
    mut panel: Query<&mut Node, With<MutationPanel>>,
) {
    if !keys.just_pressed(KeyCode::Tab) {
        return;
    }
    for mut node in &mut panel {
        node.display = if node.display == Display::None { Display::Flex } else { Display::None };
    }
}

/// Rewrites the panel for the selected family and marks what is affordable.
pub fn update_mutation_panel(
    selection: Res<MutationSelection>,
    player: Query<&PlayerGenome, With<Controlled>>,
    limit: Query<&PlayerProgress, With<Controlled>>,
    mut cards: Query<(&VariantCard, &mut Text, &mut BackgroundColor), Without<FamilyTab>>,
    mut tabs: Query<(&FamilyTab, &mut BackgroundColor), Without<VariantCard>>,
    mut header: Query<&mut Text, (With<FamilyHeader>, Without<VariantCard>, Without<FamilyTab>)>,
) {
    let organism = player.single().ok().map(|g| OrganismState::from_genome(g.0.clone()));
    let family = PartFamily::ALL[selection.family % PartFamily::ALL.len()];
    // The server's limit, not the one this build was compiled with.
    let limit = limit.single().map(|p| p.max_parts as usize).unwrap_or(MAX_PARTS);
    let full = organism.as_ref().is_some_and(|o| o.genome.parts.len() >= limit);

    if let Ok(mut header) = header.single_mut() {
        let owned = organism.as_ref().map(|o| o.genome.count_family(family)).unwrap_or(0);
        let base = cellborn_common::stats(PartKind::basic(family));
        let value = format!(
            "{} — {}   ({}, базовая цена {}, в теле: {}/{})",
            family.name().to_uppercase(),
            family.tradeoff(),
            if family.is_external() { "растёт снаружи" } else { "органелла внутри" },
            base.cost,
            owned,
            MAX_PARTS_PER_KIND,
        );
        if header.0 != value {
            header.0 = value;
        }
    }

    for (tab, mut background) in &mut tabs {
        let owned = organism
            .as_ref()
            .map(|o| o.genome.count_family(PartFamily::ALL[tab.index]))
            .unwrap_or(0);
        background.0 = if tab.index == selection.family % PartFamily::ALL.len() {
            Color::srgba(0.35, 0.85, 0.65, 0.30)
        } else if owned > 0 {
            Color::srgba(0.85, 0.80, 0.35, 0.16)
        } else {
            Color::srgba(1.0, 1.0, 1.0, 0.06)
        };
    }

    for (card, mut text, mut background) in &mut cards {
        // Последняя карточка — прокачка уже отращённого органа.
        if card.variant == PartLevel::ALL.len() {
            let price = organism.as_ref().and_then(|o| o.upgrade_price(family));
            let error = organism
                .as_ref()
                .and_then(|o| o.upgrade_error(family))
                .or(if organism.is_some() { None } else { Some("нет тела") });
            let current = organism
                .as_ref()
                .and_then(|o| o.weakest_of(family))
                .and_then(|i| organism.as_ref().map(|o| o.genome.parts[i].kind));

            let value = format!(
                "{}  ПОДНЯТЬ УРОВЕНЬ  [{} очк.]\n{}\n{}",
                card.variant + 1,
                price.map(|p| p.to_string()).unwrap_or_else(|| "-".into()),
                match current {
                    Some(kind) => match kind.upgraded() {
                        Some(next) => format!("{} → {}", kind.level.name(), next.level.name()),
                        None => "уже совершенный".to_string(),
                    },
                    None => "сначала отрасти орган".to_string(),
                },
                error.unwrap_or("дешевле, чем растить второй такой же"),
            );
            if text.0 != value {
                text.0 = value;
            }
            background.0 = if error.is_none() {
                Color::srgba(0.85, 0.70, 0.35, 0.26)
            } else {
                Color::srgba(1.0, 1.0, 1.0, 0.05)
            };
            continue;
        }

        let kind = PartKind::new(family, PartLevel::ALL[card.variant]);
        let stats = cellborn_common::stats(kind);
        // The price shown is what this body pays now, surcharge included — not
        // the base price, which nobody past their third organ actually pays.
        let price = organism
            .as_ref()
            .map(|o| o.price(kind))
            .unwrap_or_else(|| stats.cost);
        let owned = organism.as_ref().map(|o| o.genome.count(kind)).unwrap_or(0);
        let error = organism.as_ref().and_then(|o| o.mutation_error(kind));

        // Effects come from the numbers themselves, so a card can never promise
        // something the simulation does not do.
        let value = format!(
            "{}  {}  [{} очк.]{}\n{}\nмасса {:.1}   содержание {:.2}/с\n{}",
            card.variant + 1,
            PartLevel::ALL[card.variant].name(),
            price,
            if owned > 0 { format!("   уже {owned}") } else { String::new() },
            effect_summary(kind),
            stats.mass,
            stats.upkeep,
            match error {
                Some(reason) => reason,
                None => PartLevel::ALL[card.variant].hint(),
            },
        );
        if text.0 != value {
            text.0 = value;
        }
        background.0 = if error.is_none() && !full {
            Color::srgba(0.35, 0.85, 0.65, 0.22)
        } else {
            Color::srgba(1.0, 1.0, 1.0, 0.05)
        };
    }
}

/// Q / E switch families, digits pick a variant, clicks do both.
pub fn mutation_navigation(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<MutationSelection>,
    tabs: Query<(&Interaction, &FamilyTab), Changed<Interaction>>,
) {
    let count = PartFamily::ALL.len();
    if keys.just_pressed(KeyCode::KeyQ) {
        selection.family = (selection.family + count - 1) % count;
    }
    if keys.just_pressed(KeyCode::KeyE) {
        selection.family = (selection.family + 1) % count;
    }
    for (interaction, tab) in &tabs {
        if *interaction == Interaction::Pressed {
            selection.family = tab.index;
        }
    }
}

/// Sends a mutation request. The client asks; the server decides.
pub fn mutation_input(
    keys: Res<ButtonInput<KeyCode>>,
    selection: Res<MutationSelection>,
    cards: Query<(&Interaction, &VariantCard), Changed<Interaction>>,
    mut sender: Query<&mut MessageSender<MutationRequest>, With<Client>>,
) {
    // Пять клавиш: четыре уровня и прокачка. Больше не нужно — карточек ровно
    // столько.
    const DIGITS: [KeyCode; 5] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
    ];

    let mut wanted: Option<usize> = None;
    for (index, key) in DIGITS.iter().enumerate() {
        if keys.just_pressed(*key) {
            wanted = Some(index);
        }
    }
    for (interaction, card) in &cards {
        if *interaction == Interaction::Pressed {
            wanted = Some(card.variant);
        }
    }

    let Some(slot) = wanted else { return; };
    let Ok(mut sender) = sender.single_mut() else { return; };
    let family = PartFamily::ALL[selection.family % PartFamily::ALL.len()];
    let request = match PartLevel::ALL.get(slot) {
        Some(level) => MutationRequest::Grow(PartKind::new(family, *level)),
        // Слот за последним уровнем — прокачка уже отращённого.
        None => MutationRequest::Upgrade(family),
    };
    sender.send::<GameplayChannel>(request);
}

#[cfg(test)]
mod tests {
    /// Иконки лежат файлами, и это значит, что их легко потерять: не
    /// переименовать в скрипте, не положить в релиз, не докопировать при
    /// обновлении. Пропажу видно только на экране, и то не сразу.
    ///
    /// Поэтому список нужных файлов проверяется сборкой. Путь строится от
    /// каталога крейта, а не от рабочего: тесты запускают из разных мест.
    #[test]
    fn every_icon_the_ui_asks_for_exists() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/ui");
        for name in ["energy", "health", "mass", "points", "mutation", "danger"] {
            let path = root.join(format!("{name}.png"));
            let bytes = std::fs::read(&path)
                .unwrap_or_else(|e| panic!("нет иконки {}: {e}", path.display()));
            // Заголовок PNG: файл должен быть картинкой, а не пустышкой или
            // текстом, случайно сохранённым под этим именем.
            assert_eq!(&bytes[..4], b"\x89PNG", "{name}.png не PNG");
            assert!(bytes.len() > 100, "{name}.png подозрительно пуст");
        }
    }
}
