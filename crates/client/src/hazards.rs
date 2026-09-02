//! Отрисовка того, что не является организмом: колючек, левиафанов и лакомых
//! мест.
//!
//! Главное правило здесь — **цвет отвечает на вопрос про тебя**. Колючка
//! красится по тому, пролезешь ли в неё ты сейчас, а не по какому-то своему
//! свойству: то же самое сделано для облаков яда. Игрок должен уметь
//! посмотреть на предмет и понять, что тот с ним сделает, не открывая
//! справочник.

use bevy::prelude::*;
use cellborn_common::*;
use lightyear::prelude::*;

use crate::fx::NotShadowCaster;

/// Уже нарисованная колючка.
#[derive(Component)]
pub struct ThornVisual;

/// Иглы колючки: их подкрашивают в зависимости от того, опасна ли она тебе.
#[derive(Component)]
pub struct ThornSpine;

#[derive(Component)]
pub struct LeviathanVisual;

#[derive(Component)]
pub struct FeastVisual;

/// Цвет колючки, которая тебя пропустит: спокойная зелень, «сюда можно».
const SHELTER: Color = Color::srgb(0.36, 0.78, 0.52);
/// Цвет колючки, которая тебя порежет.
const HAZARD: Color = Color::srgb(0.95, 0.42, 0.30);

/// Сколько пятен мути показывать одновременно.
///
/// Пул фиксированного размера: клеток загрязнения четыре сотни, и рисовать
/// каждую значило бы четыреста прозрачных объектов ради того, что читается и
/// десятком. Показываем самые грязные, остальные и так ниже порога заметности.
const MURK_BLOBS: usize = 40;

/// Ниже этого уровня грязь не рисуется: чуть запачканная вода выглядит чистой,
/// и это правда — она не наносит урона.
const MURK_FLOOR: f32 = 0.18;

/// Одно пятно мути из пула.
#[derive(Component)]
pub struct Murk;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_murk_pool);
    app.add_systems(
        Update,
        (
            spawn_thorn_visuals,
            recolor_thorns,
            spawn_leviathan_visuals,
            animate_leviathans,
            spawn_feast_visuals,
            animate_feasts,
            show_pollution,
        )
            .chain(),
    );
}

/// Пул пятен создаётся один раз и потом только переставляется.
///
/// Грязь меняется каждые полсекунды по всей карте; создавать и уничтожать под
/// это сущности значило бы дёргать мир сотнями команд в секунду.
fn spawn_murk_pool(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Sphere::new(1.0).mesh().uv(12, 8));
    for _ in 0..MURK_BLOBS {
        commands.spawn((
            Murk,
            Transform::from_scale(Vec3::ZERO),
            Visibility::Hidden,
            Mesh3d(mesh.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                // Болотная муть: бурая взвесь, а не зелёный туман. Она должна
                // читаться как «вода испорчена», а не как ещё одна подсветка.
                base_color: Color::srgba(0.26, 0.30, 0.16, 0.16),
                emissive: LinearRgba::new(0.04, 0.05, 0.01, 1.0),
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 1.0,
                ..default()
            })),
            NotShadowCaster,
        ));
    }
}

/// Переставляет пятна мути на самые грязные клетки.
///
/// Без этого загрязнение было чистой абстракцией: игрок терял здоровье в
/// толпе и не понимал, за что. Теперь вода вокруг скопления буреет, и причина
/// видна раньше, чем начинает падать полоска.
fn show_pollution(
    pollution: Query<&Pollution>,
    time: Res<Time>,
    mut blobs: Query<(&mut Transform, &mut Visibility), With<Murk>>,
) {
    let Ok(pollution) = pollution.single() else { return };

    // Самые грязные клетки: частичная сортировка по уровню.
    let mut worst: Vec<(u8, usize)> = pollution
        .cells
        .iter()
        .enumerate()
        .filter(|(_, level)| **level as f32 / 255.0 >= MURK_FLOOR)
        .map(|(index, level)| (*level, index))
        .collect();
    worst.sort_unstable_by(|a, b| b.0.cmp(&a.0));

    let breathe = 1.0 + (time.elapsed_secs() * 0.35).sin() * 0.06;
    for (slot, (mut transform, mut visibility)) in blobs.iter_mut().enumerate() {
        match worst.get(slot) {
            Some((level, index)) => {
                let (x, z) = (index % POLLUTION_SIDE, index / POLLUTION_SIDE);
                // Центр клетки в мировых координатах.
                let position = Vec3::new(
                    (x as f32 + 0.5) * POLLUTION_CELL - ARENA_HALF_EXTENT,
                    // Чуть выше дна: муть висит в воде, а не лежит на грунте.
                    0.4,
                    (z as f32 + 0.5) * POLLUTION_CELL - ARENA_HALF_EXTENT,
                );
                let strength = *level as f32 / 255.0;
                transform.translation = position;
                // Грязнее — крупнее и заметнее. Размер чуть больше клетки,
                // чтобы соседние пятна сливались в одно облако, а не в шахматы.
                transform.scale =
                    Vec3::splat(POLLUTION_CELL * (0.5 + strength * 0.45) * breathe);
                *visibility = Visibility::Inherited;
            }
            None => *visibility = Visibility::Hidden,
        }
    }
}

fn spawn_thorn_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    thorns: Query<(Entity, &Thorn), Without<ThornVisual>>,
) {
    for (entity, thorn) in &thorns {
        commands.entity(entity).insert((
            ThornVisual,
            Transform::from_translation(thorn.position),
            Visibility::default(),
        ));

        // Сердцевина маленькая: куст — это в основном иглы и пустота между
        // ними, иначе внутри негде прятаться.
        let core = meshes.add(Sphere::new(thorn.radius * 0.22).mesh().uv(16, 12));
        // Три длины игл вместо одной: ровные иглы одинаковой длины читаются как
        // морской ёж, а нужен куст — неровный, с просветами.
        let spines: Vec<Handle<Mesh>> = [0.55f32, 0.78, 1.0]
            .iter()
            .map(|length| {
                meshes.add(Cone {
                    radius: thorn.radius * 0.055,
                    height: thorn.radius * 0.78 * length,
                })
            })
            .collect();

        commands.entity(entity).with_children(|parent| {
            parent.spawn((
                Mesh3d(core),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.16, 0.34, 0.28),
                    perceptual_roughness: 0.8,
                    ..default()
                })),
            ));

            // Иглы по золотому углу, как и органы на теле: так они не сбиваются
            // в пучки при любом их количестве.
            for i in 0..64 {
                let dir = slot_direction(i);
                // Длина и вылет чередуются, чтобы куст был неровным.
                let variant = i % spines.len();
                let reach = 0.34 + variant as f32 * 0.14;
                parent.spawn((
                    ThornSpine,
                    Transform::from_translation(dir * thorn.radius * reach)
                        .with_rotation(Quat::from_rotation_arc(Vec3::Y, dir)),
                    Mesh3d(spines[variant].clone()),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: SHELTER,
                        emissive: LinearRgba::from(SHELTER) * 0.4,
                        perceptual_roughness: 0.5,
                        ..default()
                    })),
                    NotShadowCaster,
                ));
            }
        });
    }
}

/// Красит иглы по тому, опасна ли колючка **тебе сейчас**.
///
/// Вырос — и все колючки на карте разом побагровели: это единственное
/// уведомление о том, что убежища кончились, и оно приходит ровно в тот момент,
/// когда перестаёт быть правдой прежнее.
fn recolor_thorns(
    mut materials: ResMut<Assets<StandardMaterial>>,
    player: Query<&PlayerVitals, With<Controlled>>,
    spines: Query<&MeshMaterial3d<StandardMaterial>, With<ThornSpine>>,
) {
    let Ok(vitals) = player.single() else { return };
    let dangerous = Thorn::hurts(body_radius(vitals.mass));
    let wanted = if dangerous { HAZARD } else { SHELTER };

    for material in &spines {
        let Some(mut material) = materials.get_mut(material.id()) else { continue };
        if material.base_color != wanted {
            material.base_color = wanted;
            material.emissive = LinearRgba::from(wanted) * 0.4;
        }
    }
}

fn spawn_leviathan_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    beasts: Query<(Entity, &Leviathan), Without<LeviathanVisual>>,
) {
    for (entity, beast) in &beasts {
        commands.entity(entity).insert((
            LeviathanVisual,
            Transform::from_translation(beast.position),
            Visibility::default(),
        ));

        // Тёмный силуэт без деталей: чудовище должно читаться как тень, идущая
        // из мглы, а не как ещё одна клетка, только больше.
        let skin = materials.add(StandardMaterial {
            base_color: Color::srgb(0.06, 0.10, 0.14),
            emissive: LinearRgba::new(0.02, 0.05, 0.08, 1.0),
            perceptual_roughness: 0.9,
            ..default()
        });
        let body = meshes.add(Sphere::new(beast.radius).mesh().uv(24, 16));
        let fin = meshes.add(Sphere::new(beast.radius * 0.55).mesh().uv(12, 8));

        commands.entity(entity).with_children(|parent| {
            parent.spawn((
                Transform::from_scale(Vec3::new(0.62, 0.45, 1.7)),
                Mesh3d(body),
                MeshMaterial3d(skin.clone()),
            ));
            for side in [-1.0f32, 1.0] {
                parent.spawn((
                    Transform::from_xyz(side * beast.radius * 0.7, 0.0, beast.radius * 0.2)
                        .with_scale(Vec3::new(1.4, 0.18, 0.7)),
                    Mesh3d(fin.clone()),
                    MeshMaterial3d(skin.clone()),
                ));
            }
        });
    }
}

/// Чудовище должно плыть, а не скользить.
///
/// Фаза хвоста приходит с сервера, поэтому все видят одно и то же существо:
/// считай её клиент сам, у двух игроков рядом оно извивалось бы вразнобой.
fn animate_leviathans(
    beasts: Query<(&Leviathan, &Children, &mut Transform)>,
    mut parts: Query<&mut Transform, Without<Leviathan>>,
) {
    for (beast, children, mut transform) in beasts {
        transform.translation = beast.position;
        // Смотрит туда, куда плывёт: иначе туша идёт боком.
        let facing = Quat::from_rotation_arc(Vec3::NEG_Z, beast.heading);
        // Тело переваливается на ходу — от этого туша выглядит тяжёлой.
        let roll = (beast.swim * 0.5).sin() * 0.12;
        transform.rotation = facing * Quat::from_rotation_z(roll);

        for (index, child) in children.iter().enumerate() {
            let Ok(mut part) = parts.get_mut(child) else { continue };
            match index {
                // Туловище: медленный изгиб вдоль оси движения.
                0 => {
                    part.rotation = Quat::from_rotation_y(beast.swim.sin() * 0.10);
                    // Наевшееся раздувается: по брюху видно, сколько оно съело.
                    let belly = 1.0 + beast.fed * 0.06;
                    part.scale = Vec3::new(0.62 * belly, 0.45 * belly, 1.7);
                }
                // Плавники бьют в противофазе друг другу.
                _ => {
                    let side = if index == 1 { 1.0 } else { -1.0 };
                    part.rotation =
                        Quat::from_rotation_z((beast.swim * 1.4 + side).sin() * 0.35 * side);
                }
            }
        }
    }
}

fn spawn_feast_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    feasts: Query<(Entity, &Feast), Without<FeastVisual>>,
) {
    for (entity, feast) in &feasts {
        commands.entity(entity).insert((
            FeastVisual,
            Transform::from_translation(feast.position),
            Visibility::default(),
        ));
        // Тёплое свечение над скоплением: заметно издалека, но ничего не
        // загораживает — саму еду видно сквозь него.
        commands.entity(entity).with_children(|parent| {
            parent.spawn((
                Transform::from_scale(Vec3::splat(feast.radius)),
                Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(20, 14))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgba(1.0, 0.86, 0.45, 0.07),
                    emissive: LinearRgba::new(0.55, 0.40, 0.10, 1.0),
                    alpha_mode: AlphaMode::Add,
                    ..default()
                })),
                NotShadowCaster,
            ));
        });
    }
}

/// Угасающее пятно тускнеет заранее, чтобы к нему не плыли зря.
fn animate_feasts(
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    feasts: Query<(&Feast, &Children)>,
    haze: Query<&MeshMaterial3d<StandardMaterial>>,
) {
    let pulse = 1.0 + (time.elapsed_secs() * 0.8).sin() * 0.12;
    for (feast, children) in &feasts {
        for child in children.iter() {
            let Ok(material) = haze.get(child) else { continue };
            let Some(mut material) = materials.get_mut(material.id()) else { continue };
            let glow = feast.strength.clamp(0.0, 1.0) * pulse;
            material.emissive = LinearRgba::new(0.55, 0.40, 0.10, 1.0) * glow;
        }
    }
}
