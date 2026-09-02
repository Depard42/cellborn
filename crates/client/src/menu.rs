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
    /// Выбор сервера: найденные в сети и запомненные.
    Servers,
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
    Update,
    Quit,
}

/// Кнопки громкости в меню. Три полосы, у каждой тише и громче.
#[derive(Component, Clone, Copy, PartialEq)]
pub enum VolumeButton {
    Master(i8),
    Music(i8),
    Effects(i8),
}

/// Строка, показывающая текущую громкость.
#[derive(Component)]
pub struct VolumeLine;

/// Корень экрана выбора сервера.
#[derive(Component)]
pub struct ServersRoot;

/// Кнопка одного сервера в списке.
#[derive(Component, Clone)]
pub struct ServerButton {
    pub address: String,
    pub name: String,
}

/// Строка со списком: перерисовывается, пока идёт поиск.
#[derive(Component)]
pub struct ServerList;

/// Поле ручного ввода адреса.
///
/// Своё, а не готовый виджет: в Bevy текстового поля нет, а тащить ради одной
/// строки целую библиотеку — дороже, чем написать сорок строк здесь.
#[derive(Component)]
pub struct AddressField;

/// Что игрок уже набрал.
#[derive(Resource, Default)]
pub struct AddressInput {
    pub text: String,
    /// Почему адрес не принят. Пусто — всё в порядке.
    pub error: &'static str,
}

impl AddressInput {
    /// Разбирает набранное в адрес, дописывая порт по умолчанию.
    ///
    /// Игрок вводит «192.168.1.10», а не «192.168.1.10:5555» — требовать порт
    /// значит требовать помнить его наизусть.
    pub fn parse(&self) -> Option<std::net::SocketAddr> {
        let text = self.text.trim();
        if text.is_empty() {
            return None;
        }
        text.parse()
            .ok()
            .or_else(|| format!("{text}:{}", cellborn_common::SERVER_PORT).parse().ok())
    }
}

/// Подпись под кнопкой обновления: что сейчас происходит.
#[derive(Component)]
pub struct UpdateStatus;

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
                // Текст на этой кнопке переписывается на ходу: она же
                // проверяет, она же ставит, она же просит перезапуск.
                (MenuButton::Update, "ПРОВЕРИТЬ ОБНОВЛЕНИЕ"),
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

            // Громкость: три полосы с кнопками тише и громче. Настройка живёт
            // в файле рядом с игрой и переживает обновление.
            root.spawn((VolumeLine, text(font, 12.0, DIM, "громкость")));
            root.spawn(Node {
                column_gap: Val::Px(4.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                for (button, label) in [
                    (VolumeButton::Master(-1), "общая −"),
                    (VolumeButton::Master(1), "+"),
                    (VolumeButton::Music(-1), "музыка −"),
                    (VolumeButton::Music(1), "+"),
                    (VolumeButton::Effects(-1), "звуки −"),
                    (VolumeButton::Effects(1), "+"),
                ] {
                    row.spawn((
                        button,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.06)),
                        Text::new(label),
                        TextFont {
                            font: FontSource::Handle(font.clone()),
                            font_size: FontSize::Px(11.0),
                            ..default()
                        },
                        TextColor(INK),
                    ));
                }
            });

            // Строка состояния обновления. Пустой она не бывает: пока ничего
            // не происходит, здесь просто написано, какая версия установлена.
            root.spawn((UpdateStatus, text(font, 11.0, DIM, &version::full())));
            root.spawn(text(font, 12.0, DIM, &mutation_count_line()));
            root.spawn(text(
                font,
                12.0,
                DIM,
                "новичку: открой справочник и прочитай «С чего начать» — это две минуты",
            ));
        });
}

/// Держит кнопку обновления и строку под ней в согласии с тем, что делает
/// обновлятор: он работает в своём потоке, а меню просто показывает его
/// состояние.
pub fn update_menu_status(
    updater: Res<crate::update::Updater>,
    mut buttons: Query<(&MenuButton, &mut Text, &mut BackgroundColor), Without<UpdateStatus>>,
    mut status: Query<&mut Text, With<UpdateStatus>>,
) {
    for (button, mut text, mut background) in &mut buttons {
        if !matches!(button, MenuButton::Update) {
            continue;
        }
        let label = updater.as_ref().button_label();
        if text.0 != label {
            text.0 = label;
        }
        // Пока идёт проверка или закачка, кнопка гаснет: нажимать её больше
        // незачем, и это должно быть видно, а не только написано.
        background.0 = if updater.as_ref().busy() {
            Color::srgba(1.0, 1.0, 1.0, 0.03)
        } else {
            Color::srgba(1.0, 1.0, 1.0, 0.07)
        };
    }
    if let Ok(mut text) = status.single_mut() {
        let line = updater.as_ref().status_line();
        if text.0 != line {
            text.0 = line;
        }
    }
}

/// Кнопки громкости двигают настройку на десятую долю за нажатие.
pub fn volume_input(
    buttons: Query<(&Interaction, &VolumeButton), Changed<Interaction>>,
    mut settings: ResMut<crate::settings::Settings>,
    mut line: Query<&mut Text, With<VolumeLine>>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // Шаг в десятую: мельче никто не различает на слух, крупнее — грубо.
        let step = 0.1;
        match button {
            VolumeButton::Master(dir) => {
                settings.volume = (settings.volume + step * *dir as f32).clamp(0.0, 1.0)
            }
            VolumeButton::Music(dir) => {
                settings.music = (settings.music + step * *dir as f32).clamp(0.0, 1.0)
            }
            VolumeButton::Effects(dir) => {
                settings.effects = (settings.effects + step * *dir as f32).clamp(0.0, 1.0)
            }
        }
    }

    if let Ok(mut text) = line.single_mut() {
        let value = format!(
            "громкость: общая {:.0}%   музыка {:.0}%   звуки {:.0}%",
            settings.volume * 100.0,
            settings.music * 100.0,
            settings.effects * 100.0,
        );
        if text.0 != value {
            text.0 = value;
        }
    }
}

/// Экран выбора сервера: найденные в сети сверху, запомненные снизу.
pub fn setup_servers(
    mut commands: Commands,
    font: Res<UiFont>,
    mut discovery: ResMut<crate::discovery::Discovery>,
) {
    // Список начинается с чистого листа: иначе игрок увидит серверы,
    // выключенные полчаса назад.
    discovery.reset();
    let font = &font.0;
    commands
        .spawn((
            ServersRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.05, 0.06, 0.92)),
        ))
        .with_children(|root| {
            root.spawn(text(font, 26.0, ACCENT, "КУДА ПЛЫВЁМ"));
            root.spawn(text(
                font,
                12.0,
                DIM,
                "серверы в твоей сети находятся сами; можно и вписать адрес руками\nEnter — подключиться, Esc — назад",
            ));
            // Поле адреса: набирается с клавиатуры, Enter подключает.
            root.spawn((
                AddressField,
                Node {
                    width: Val::Px(430.0),
                    justify_content: JustifyContent::Center,
                    padding: UiRect::all(Val::Px(9.0)),
                    margin: UiRect::top(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
                BorderColor::all(Color::srgba(0.42, 0.88, 0.72, 0.45)),
                Text::new(""),
                TextFont {
                    font: FontSource::Handle(font.clone()),
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(INK),
            ));

            root.spawn((
                ServerList,
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(5.0),
                    margin: UiRect::top(Val::Px(10.0)),
                    ..default()
                },
            ));
        });
}

/// Перестраивает список серверов: найденные плюс запомненные.
///
/// Кнопки пересоздаются целиком, а не правятся: список короткий, меняется
/// раз в секунду, и любая попытка обновлять его на месте была бы сложнее,
/// чем собрать заново.
#[allow(clippy::too_many_arguments)]
pub fn update_servers(
    mut commands: Commands,
    time: Res<Time>,
    font: Res<UiFont>,
    server: Res<crate::ServerAddress>,
    settings: Res<crate::settings::Settings>,
    mut discovery: ResMut<crate::discovery::Discovery>,
    list: Query<(Entity, Option<&Children>), With<ServerList>>,
    mut last: Local<Vec<String>>,
) {
    crate::discovery::poll(&mut discovery, time.delta_secs());

    // Собираем строки: сперва найденные, потом запомненные, потом адрес из
    // командной строки как запасной вариант.
    let mut rows: Vec<(String, String)> = Vec::new();
    for found in &discovery.found {
        rows.push((found.address.to_string(), format!("в сети:  {}", found.label())));
    }
    for saved in &settings.servers {
        if rows.iter().any(|(address, _)| *address == saved.address) {
            continue;
        }
        let name = if saved.name.is_empty() { "сохранённый" } else { &saved.name };
        rows.push((saved.address.clone(), format!("{name}:  {}", saved.address)));
    }
    let fallback = server.0.to_string();
    if !rows.iter().any(|(address, _)| *address == fallback) {
        rows.push((fallback.clone(), format!("по умолчанию:  {fallback}")));
    }

    // Ничего не изменилось — не трогаем интерфейс.
    let signature: Vec<String> = rows.iter().map(|(_, label)| label.clone()).collect();
    if *last == signature {
        return;
    }
    *last = signature;

    let Ok((entity, children)) = list.single() else { return };
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }

    let font = &font.0;
    commands.entity(entity).with_children(|list| {
        for (address, label) in rows {
            list.spawn((
                ServerButton { address: address.clone(), name: label.clone() },
                Button,
                Node {
                    width: Val::Px(430.0),
                    justify_content: JustifyContent::Center,
                    padding: UiRect::all(Val::Px(9.0)),
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.07)),
                Text::new(label),
                TextFont {
                    font: FontSource::Handle(font.clone()),
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(INK),
            ));
        }
    });
}

/// Набор адреса с клавиатуры.
///
/// Читаем логические клавиши, а не коды: так «точка» остаётся точкой при любой
/// раскладке, и цифры на основном ряду не отличаются от цифр на дополнительном.
pub fn address_input(
    mut typed: MessageReader<bevy::input::keyboard::KeyboardInput>,
    mut input: ResMut<AddressInput>,
    mut field: Query<&mut Text, With<AddressField>>,
) {
    use bevy::input::keyboard::Key;

    for event in typed.read() {
        if !event.state.is_pressed() {
            continue;
        }
        match &event.logical_key {
            Key::Backspace => {
                input.text.pop();
                input.error = "";
            }
            Key::Character(chars) => {
                for c in chars.chars() {
                    // Только то, из чего состоит адрес: буквы имени, цифры,
                    // точки, двоеточие и дефис. Остальное — опечатка.
                    if c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-') {
                        // Длиннее полусотни адрес не бывает, а поле не должно
                        // расползаться на весь экран.
                        if input.text.len() < 48 {
                            input.text.push(c);
                            input.error = "";
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if let Ok(mut text) = field.single_mut() {
        let value = if input.text.is_empty() {
            "впиши адрес: 192.168.1.10 или 192.168.1.10:5555".to_string()
        } else if input.error.is_empty() {
            format!("{}▏", input.text)
        } else {
            format!("{}▏   — {}", input.text, input.error)
        };
        if text.0 != value {
            text.0 = value;
        }
    }
}

/// Выбор сервера: подключаемся и запоминаем.
pub fn servers_input(
    buttons: Query<(&Interaction, &ServerButton), Changed<Interaction>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut typed: ResMut<AddressInput>,
    mut server: ResMut<crate::ServerAddress>,
    mut settings: ResMut<crate::settings::Settings>,
    mut next: ResMut<NextState<Screen>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(Screen::Menu);
        return;
    }

    // Enter подключает к тому, что набрано руками.
    if keys.just_pressed(KeyCode::Enter) && !typed.text.trim().is_empty() {
        match typed.parse() {
            Some(address) => {
                server.0 = address;
                settings.remember(&address.to_string(), "вручную");
                next.set(Screen::Game);
                return;
            }
            None => typed.error = "не похоже на адрес",
        }
    }

    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Ok(address) = button.address.parse() else { continue };
        server.0 = address;
        // Запоминаем именно то, к чему пошли: в следующий раз этот сервер будет
        // первым в списке.
        settings.remember(&button.address, &button.name);
        next.set(Screen::Game);
        return;
    }
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
    mut updater: ResMut<crate::update::Updater>,
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
        // «Играть» ведёт не сразу в море, а на выбор сервера: своего, чужого
        // или найденного в сети.
        Some(MenuButton::Play) => next.set(Screen::Servers),
        Some(MenuButton::Wiki) => next.set(Screen::Wiki),
        Some(MenuButton::Update) => crate::update::act(&mut updater),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Игрок вводит адрес так, как его помнит, — обычно без порта. Требовать
    /// порт значит требовать помнить наизусть то, что почти всегда одно и то же.
    #[test]
    fn an_address_without_a_port_still_works() {
        let mut input = AddressInput::default();

        input.text = "192.168.1.10".into();
        let parsed = input.parse().expect("адрес без порта не принят");
        assert_eq!(parsed.port(), cellborn_common::SERVER_PORT);
        assert_eq!(parsed.ip().to_string(), "192.168.1.10");

        // С портом — берётся указанный, а не подставленный.
        input.text = "192.168.1.10:6000".into();
        assert_eq!(input.parse().expect("адрес с портом не принят").port(), 6000);

        // Пробелы по краям не должны мешать: их легко зацепить при вставке.
        input.text = "  127.0.0.1  ".into();
        assert!(input.parse().is_some(), "пробелы сломали разбор");
    }

    /// Мусор не должен превращаться в подключение неизвестно куда.
    #[test]
    fn rubbish_is_not_an_address() {
        let mut input = AddressInput::default();
        for junk in ["", "   ", "привет", "999.999.999.999", ":::", "1.2.3.4:99999"] {
            input.text = junk.into();
            assert!(input.parse().is_none(), "«{junk}» принято за адрес");
        }
    }
}
