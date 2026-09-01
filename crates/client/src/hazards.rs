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

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            spawn_thorn_visuals,
            recolor_thorns,
            spawn_leviathan_visuals,
            animate_leviathans,
            spawn_feast_visuals,
            animate_feasts,
        )
            .chain(),
    );
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

        let core = meshes.add(Sphere::new(thorn.radius * 0.42).mesh().uv(16, 12));
        let spine = meshes.add(Cone {
            radius: thorn.radius * 0.16,
            height: thorn.radius * 0.72,
        });

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
            for i in 0..26 {
                let dir = slot_direction(i);
                parent.spawn((
                    ThornSpine,
                    Transform::from_translation(dir * thorn.radius * 0.55)
                        .with_rotation(Quat::from_rotation_arc(Vec3::Y, dir)),
                    Mesh3d(spine.clone()),
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

fn animate_leviathans(mut beasts: Query<(&Leviathan, &mut Transform)>) {
    for (beast, mut transform) in &mut beasts {
        transform.translation = beast.position;
        // Смотрит туда, куда плывёт: иначе туша идёт боком.
        transform.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, beast.heading);
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
