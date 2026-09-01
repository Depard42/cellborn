//! Главное меню и экран вики.
//!
//! Игра начинается не с подключения, а с меню: до нажатия «Играть» клиент не
//! трогает сеть вообще. Так новичок может сначала прочитать справочник, а не
//! разбираться в бою.

use bevy::prelude::*;
use cellborn_common::*;

use crate::atlas::{swatch_color, AtlasSelection, OrganButton, OrganFacts, OrganSwatch, OrganTitle, PreviewImage};
use crate::ui::UiFont;
use crate::wiki::{mutation_count_line, SECTIONS};

/// Индекс раздела, который вместо текста показывает атлас органов.
pub const ATLAS_SECTION: usize = 7;

#[derive(Component)]
pub struct AtlasPane;

#[derive(Component)]
pub struct TextPane;

/// Экран, на котором сейчас находится игрок.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Screen {
    #[default]
    Menu,
    Wiki,
    Game,
}

#[derive(Component)]
pub struct MenuRoot;

#[derive(Component)]
pub struct WikiRoot;

#[derive(Component, Clone, Copy)]
pub enum MenuButton {
    Play,
    Wiki,
    Quit,
}

#[derive(Component)]
pub struct WikiTab {
    pub index: usize,
}

#[derive(Component)]
pub struct WikiBody;

#[derive(Component)]
pub struct WikiScroll;

#[derive(Component)]
pub struct BackButton;

/// Какой раздел вики открыт.
#[derive(Resource, Default)]
pub struct WikiSelection {
    pub section: usize,
}

const INK: Color = Color::srgb(0.88, 0.95, 0.94);
const DIM: Color = Color::srgba(0.75, 0.88, 0.88, 0.75);
const PANEL: Color = Color::srgba(0.02, 0.08, 0.10, 0.92);
const ACCENT: Color = Color::srgb(0.42, 0.88, 0.72);

fn text(font: &Handle<Font>, size: f32, color: Color, value: &str) -> impl Bundle {
    (
        Text::new(value),
        TextFont { font: FontSource::Handle(font.clone()), font_size: FontSize::Px(size), ..default() },
        TextColor(color),
    )
}

pub fn setup_menu(mut commands: Commands, font: Res<UiFont>) {
    let font = &font.0;
    commands
        .spawn((
            MenuRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.05, 0.06, 0.55)),
        ))
        .with_children(|root| {
            root.spawn(text(font, 46.0, ACCENT, "CELLBORN"));
            root.spawn(text(font, 14.0, DIM, "сетевой прототип про клетки, которые растут, дерутся и делятся"));
            root.spawn((
                Node { height: Val::Px(18.0), ..default() },
                Text::new(""),
                TextFont { font: FontSource::Handle(font.clone()), font_size: FontSize::Px(4.0), ..default() },
            ));

            for (button, label) in [
                (MenuButton::Play, "ИГРАТЬ"),
                (MenuButton::Wiki, "СПРАВОЧНИК"),
                (MenuButton::Quit, "ВЫХОД"),
            ] {
                root.spawn((
                    button,
                    Button,
                    Node {
                        width: Val::Px(260.0),
                        justify_content: JustifyContent::Center,
                        padding: UiRect::all(Val::Px(11.0)),
                        border_radius: BorderRadius::all(Val::Px(5.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.07)),
                    Text::new(label),
                    TextFont { font: FontSource::Handle(font.clone()), font_size: FontSize::Px(16.0), ..default() },
                    TextColor(INK),
                ));
            }

            root.spawn(text(font, 12.0, DIM, &mutation_count_line()));
            root.spawn(text(
                font,
                12.0,
                DIM,
                "новичку: открой справочник и прочитай «С чего начать» — это две минуты",
            ));
        });
}

pub fn setup_wiki(mut commands: Commands, font: Res<UiFont>, preview: Res<PreviewImage>) {
    let font = &font.0;
    commands
        .spawn((
            WikiRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(18.0)),
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.05, 0.06, 0.97)),
        ))
        .with_children(|root| {
            root.spawn(text(font, 24.0, ACCENT, "СПРАВОЧНИК"));

            root.spawn(Node {
                flex_grow: 1.0,
                // Без min_height флекс-элемент растягивается под содержимое,
                // переполнения не возникает — и прокручивать становится нечего.
                min_height: Val::Px(0.0),
                column_gap: Val::Px(14.0),
                ..default()
            })
                .with_children(|columns| {
                    // Слева — оглавление.
                    columns
                        .spawn((
                            Node {
                                width: Val::Px(260.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(3.0),
                                padding: UiRect::all(Val::Px(8.0)),
                                border_radius: BorderRadius::all(Val::Px(5.0)),
                                ..default()
                            },
                            BackgroundColor(PANEL),
                        ))
                        .with_children(|list| {
                            for (index, section) in SECTIONS.iter().enumerate() {
                                list.spawn((
                                    WikiTab { index },
                                    Button,
                                    Node {
                                        padding: UiRect::axes(Val::Px(7.0), Val::Px(5.0)),
                                        border_radius: BorderRadius::all(Val::Px(4.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::NONE),
                                    Text::new(section.title),
                                    TextFont {
                                        font: FontSource::Handle(font.clone()),
                                        font_size: FontSize::Px(13.0),
                                        ..default()
                                    },
                                    TextColor(INK),
                                ));
                            }
                        });

                    // Справа — текст раздела, прокручивается колесом.
                    //
                    // ScrollPosition обязателен: без него узел с overflow просто
                    // обрезает содержимое и прокрутить его нечем.
                    columns
                        .spawn((
                            TextPane,
                            WikiScroll,
                            ScrollPosition::default(),
                            Node {
                                flex_grow: 1.0,
                                min_height: Val::Px(0.0),
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::FlexStart,
                                padding: UiRect::all(Val::Px(14.0)),
                                border_radius: BorderRadius::all(Val::Px(5.0)),
                                overflow: Overflow::scroll_y(),
                                ..default()
                            },
                            BackgroundColor(PANEL),
                        ))
                        .with_children(|body| {
                            body.spawn((
                                WikiBody,
                                Text::new(""),
                                TextFont {
                                    font: FontSource::Handle(font.clone()),
                                    font_size: FontSize::Px(14.0),
                                    ..default()
                                },
                                TextColor(INK),
                            ));
                        });

                    // Атлас органов: сетка слева, живая модель и цифры справа.
                    columns
                        .spawn((
                            AtlasPane,
                            Node {
                                flex_grow: 1.0,
                                display: Display::None,
                                column_gap: Val::Px(12.0),
                                padding: UiRect::all(Val::Px(12.0)),
                                border_radius: BorderRadius::all(Val::Px(5.0)),
                                ..default()
                            },
                            BackgroundColor(PANEL),
                        ))
                        .with_children(|atlas| {
                            atlas
                                .spawn(Node {
                                    width: Val::Px(340.0),
                                    flex_wrap: FlexWrap::Wrap,
                                    align_content: AlignContent::FlexStart,
                                    column_gap: Val::Px(4.0),
                                    row_gap: Val::Px(4.0),
                                    ..default()
                                })
                                .with_children(|grid| {
                                    for (index, family) in PartFamily::ALL.iter().enumerate() {
                                        grid.spawn((
                                            OrganButton { index },
                                            Button,
                                            Node {
                                                width: Val::Px(164.0),
                                                align_items: AlignItems::Center,
                                                column_gap: Val::Px(6.0),
                                                padding: UiRect::all(Val::Px(5.0)),
                                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                                ..default()
                                            },
                                            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.05)),
                                        ))
                                        .with_children(|row| {
                                            // Цветная плашка — тот же цвет, каким
                                            // орган виден на теле.
                                            row.spawn((
                                                OrganSwatch { index },
                                                Node {
                                                    width: Val::Px(14.0),
                                                    height: Val::Px(14.0),
                                                    border_radius: BorderRadius::all(Val::Px(7.0)),
                                                    ..default()
                                                },
                                                BackgroundColor(swatch_color(index)),
                                            ));
                                            row.spawn((
                                                Text::new(family.name()),
                                                TextFont {
                                                    font: FontSource::Handle(font.clone()),
                                                    font_size: FontSize::Px(12.0),
                                                    ..default()
                                                },
                                                TextColor(INK),
                                            ));
                                        });
                                    }
                                });

                            atlas
                                .spawn(Node {
                                    flex_grow: 1.0,
                                    flex_direction: FlexDirection::Column,
                                    align_items: AlignItems::Center,
                                    row_gap: Val::Px(8.0),
                                    ..default()
                                })
                                .with_children(|side| {
                                    side.spawn((
                                        OrganTitle,
                                        Text::new(""),
                                        TextFont {
                                            font: FontSource::Handle(font.clone()),
                                            font_size: FontSize::Px(17.0),
                                            ..default()
                                        },
                                        TextColor(ACCENT),
                                    ));
                                    side.spawn((
                                        ImageNode::new(preview.0.clone()),
                                        Node {
                                            width: Val::Px(260.0),
                                            height: Val::Px(260.0),
                                            border_radius: BorderRadius::all(Val::Px(6.0)),
                                            ..default()
                                        },
                                    ));
                                    side.spawn((
                                        OrganFacts,
                                        Text::new(""),
                                        TextFont {
                                            font: FontSource::Handle(font.clone()),
                                            font_size: FontSize::Px(12.0),
                                            ..default()
                                        },
                                        TextColor(INK),
                                    ));
                                });
                        });
                });

            root.spawn((
                BackButton,
                Button,
                Node {
                    width: Val::Px(200.0),
                    justify_content: JustifyContent::Center,
                    padding: UiRect::all(Val::Px(9.0)),
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.07)),
                Text::new("НАЗАД  (Esc)   ↑↓ — раздел, колесо — прокрутка"),
                TextFont { font: FontSource::Handle(font.clone()), font_size: FontSize::Px(14.0), ..default() },
                TextColor(INK),
            ));
        });
}

/// Кнопки меню.
pub fn menu_input(
    buttons: Query<(&Interaction, &MenuButton), Changed<Interaction>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<Screen>>,
    mut exit: MessageWriter<AppExit>,
) {
    let mut chosen = None;
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed {
            chosen = Some(*button);
        }
    }
    if keys.just_pressed(KeyCode::Enter) {
        chosen = Some(MenuButton::Play);
    }
    if keys.just_pressed(KeyCode::F1) {
        chosen = Some(MenuButton::Wiki);
    }

    match chosen {
        Some(MenuButton::Play) => next.set(Screen::Game),
        Some(MenuButton::Wiki) => next.set(Screen::Wiki),
        Some(MenuButton::Quit) => {
            exit.write(AppExit::Success);
        }
        None => {}
    }
}

/// Выбор раздела, прокрутка и выход назад.
pub fn wiki_input(
    keys: Res<ButtonInput<KeyCode>>,
    tabs: Query<(&Interaction, &WikiTab), Changed<Interaction>>,
    back: Query<&Interaction, (Changed<Interaction>, With<BackButton>)>,
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut selection: ResMut<WikiSelection>,
    mut scroll: Query<&mut ScrollPosition, With<WikiScroll>>,
    mut next: ResMut<NextState<Screen>>,
) {
    for (interaction, tab) in &tabs {
        if *interaction == Interaction::Pressed {
            selection.section = tab.index;
            for mut position in &mut scroll {
                position.0.y = 0.0;
            }
        }
    }
    let count = SECTIONS.len();
    if keys.just_pressed(KeyCode::ArrowDown) {
        selection.section = (selection.section + 1) % count;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        selection.section = (selection.section + count - 1) % count;
    }

    // Колесо мыши прокручивает текст.
    let mut delta = 0.0;
    for event in wheel.read() {
        delta += match event.unit {
            bevy::input::mouse::MouseScrollUnit::Line => event.y * 28.0,
            bevy::input::mouse::MouseScrollUnit::Pixel => event.y,
        };
    }
    if delta != 0.0 {
        for mut position in &mut scroll {
            position.0.y = (position.0.y - delta).max(0.0);
        }
    }

    let pressed_back = back.iter().any(|i| *i == Interaction::Pressed);
    if pressed_back || keys.just_pressed(KeyCode::Escape) {
        next.set(Screen::Menu);
    }
}

/// Перерисовывает текст выбранного раздела и подсвечивает оглавление.
pub fn update_wiki(
    selection: Res<WikiSelection>,
    mut body: Query<&mut Text, With<WikiBody>>,
    mut tabs: Query<(&WikiTab, &mut BackgroundColor, &mut TextColor)>,
) {
    let index = selection.section.min(SECTIONS.len() - 1);
    if let Ok(mut text) = body.single_mut() {
        let section = &SECTIONS[index];
        let value = format!("{}\n\n{}", section.title.to_uppercase(), section.text());
        if text.0 != value {
            text.0 = value;
        }
    }
    for (tab, mut background, mut color) in &mut tabs {
        let active = tab.index == index;
        background.0 = if active { Color::srgba(0.35, 0.85, 0.65, 0.22) } else { Color::NONE };
        color.0 = if active { ACCENT } else { INK };
    }
}

/// Раздел «Все органы» показывает атлас вместо текста.
#[allow(clippy::too_many_arguments)]
pub fn update_atlas(
    selection: Res<WikiSelection>,
    mut atlas: ResMut<AtlasSelection>,
    buttons: Query<(&Interaction, &OrganButton), Changed<Interaction>>,
    mut panes: ParamSet<(
        Query<&mut Node, With<TextPane>>,
        Query<&mut Node, With<AtlasPane>>,
    )>,
    mut title: Query<&mut Text, (With<OrganTitle>, Without<OrganFacts>)>,
    mut facts: Query<&mut Text, (With<OrganFacts>, Without<OrganTitle>)>,
    mut swatches: Query<(&OrganSwatch, &mut BackgroundColor)>,
) {
    let showing_atlas = selection.section == ATLAS_SECTION;
    for mut node in &mut panes.p0() {
        node.display = if showing_atlas { Display::None } else { Display::Flex };
    }
    for mut node in &mut panes.p1() {
        node.display = if showing_atlas { Display::Flex } else { Display::None };
    }
    if !showing_atlas {
        return;
    }

    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed {
            atlas.family = button.index;
        }
    }

    let index = atlas.family % PartFamily::ALL.len();
    let family = PartFamily::ALL[index];
    let kind = PartKind::basic(family);
    let stats = cellborn_common::stats(kind);

    if let Ok(mut text) = title.single_mut() {
        let value = family.name().to_uppercase();
        if text.0 != value {
            text.0 = value;
        }
    }
    if let Ok(mut text) = facts.single_mut() {
        let value = format!(
            "{}\n\nцена {}   масса {:.1}   содержание {:.2}/с\n{}\n\nдаёт: {}\n\n{}",
            family.tradeoff(),
            stats.cost,
            stats.mass,
            stats.upkeep,
            if family.is_external() { "растёт снаружи" } else { "органелла внутри" },
            effect_summary(kind),
            crate::wiki::mechanic_of(family),
        );
        if text.0 != value {
            text.0 = value;
        }
    }
    for (swatch, mut background) in &mut swatches {
        let color = swatch_color(swatch.index);
        background.0 = if swatch.index == index { color } else { color.with_alpha(0.45) };
    }
}

/// Esc в игре возвращает в меню.
pub fn game_escape(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<Screen>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(Screen::Menu);
    }
}

/// Убирает всё, что принадлежало экрану.
pub fn despawn<T: Component>(mut commands: Commands, query: Query<Entity, With<T>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}
