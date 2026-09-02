//! Атлас органов: сетка из двадцати кнопок и живая 3D-модель выбранного.
//!
//! Картинка здесь не нарисованная, а настоящая: отдельная камера снимает тот же
//! меш, который вырастет на теле, и отдаёт кадр в текстуру интерфейса. Поэтому
//! превью не может разойтись с игрой — это буквально она и есть.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use cellborn_common::*;

use crate::body::part_color;

/// Слой, который видит только камера превью: модель органа висит далеко от
/// игрового мира и не мешает ему.
pub const PREVIEW_LAYER: usize = 3;
/// Куда уезжает сцена превью, чтобы не пересекаться с ареной.
const PREVIEW_ORIGIN: Vec3 = Vec3::new(0.0, 500.0, 0.0);

#[derive(Resource)]
pub struct PreviewImage(pub Handle<Image>);

/// Какой орган показан в атласе.
#[derive(Resource, Default)]
pub struct AtlasSelection {
    pub family: usize,
}

#[derive(Component)]
pub struct PreviewCamera;

/// Модель органа: пересобирается при смене выбора.
#[derive(Component)]
pub struct PreviewModel;

#[derive(Component)]
pub struct OrganButton {
    pub index: usize,
}

#[derive(Component)]
pub struct OrganSwatch {
    pub index: usize,
}

#[derive(Component)]
pub struct OrganTitle;

#[derive(Component)]
pub struct OrganFacts;

/// Создаёт целевую текстуру и камеру, которая в неё рисует.
pub fn setup_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let image =
        images.add(Image::new_target_texture(320, 320, TextureFormat::Rgba8UnormSrgb, None));
    commands.insert_resource(PreviewImage(image.clone()));

    commands.spawn((
        PreviewCamera,
        Camera3d::default(),
        Camera {
            // Рисуем раньше основной камеры, чтобы кадр успел попасть в интерфейс.
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.02, 0.07, 0.09)),
            ..default()
        },
        // RenderTarget — отдельный компонент, а не поле камеры.
        RenderTarget::Image(image.into()),
        // Без Hdr: цель — восьмибитная текстура, и HDR-проход с ней несовместим.
        Transform::from_translation(PREVIEW_ORIGIN + Vec3::new(0.0, 0.9, 3.1))
            .looking_at(PREVIEW_ORIGIN, Vec3::Y),
        RenderLayers::layer(PREVIEW_LAYER),
    ));

    commands.spawn((
        DirectionalLight { illuminance: 9000.0, ..default() },
        Transform::from_translation(PREVIEW_ORIGIN + Vec3::new(2.0, 4.0, 3.0))
            .looking_at(PREVIEW_ORIGIN, Vec3::Y),
        RenderLayers::layer(PREVIEW_LAYER),
    ));

    // Полупрозрачная мембрана-подложка, чтобы орган было видно «в клетке».
    commands.spawn((
        Transform::from_translation(PREVIEW_ORIGIN).with_scale(Vec3::splat(0.95)),
        Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(28, 16))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.40, 0.80, 0.70, 0.16),
            emissive: LinearRgba::new(0.05, 0.16, 0.13, 1.0),
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            ..default()
        })),
        RenderLayers::layer(PREVIEW_LAYER),
    ));
}

/// Пересобирает модель, когда выбран другой орган.
pub fn rebuild_preview(
    mut commands: Commands,
    selection: Res<AtlasSelection>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<Entity, With<PreviewModel>>,
    mut shown: Local<Option<usize>>,
) {
    let index = selection.family % PartFamily::ALL.len();
    if *shown == Some(index) {
        return;
    }
    *shown = Some(index);
    for entity in &existing {
        commands.entity(entity).despawn();
    }

    let family = PartFamily::ALL[index];
    let color = part_color(PartKind::basic(family));
    let material = materials.add(StandardMaterial {
        base_color: color,
        emissive: LinearRgba::from(color) * 0.35,
        perceptual_roughness: 0.4,
        ..default()
    });

    let root = commands
        .spawn((
            PreviewModel,
            Transform::from_translation(PREVIEW_ORIGIN),
            Visibility::default(),
            RenderLayers::layer(PREVIEW_LAYER),
        ))
        .id();

    // Та же геометрия, что вырастет на теле, только собранная отдельно.
    match family {
        PartFamily::Flagellum | PartFamily::Pseudopod => {
            let mesh = meshes.add(Capsule3d::new(0.13, 0.22).mesh().latitudes(8).longitudes(10));
            for i in 0..7 {
                let taper = 1.0 - i as f32 / 9.0;
                spawn_child(
                    &mut commands, root, mesh.clone(), material.clone(),
                    Transform::from_xyz(0.0, 0.55 - i as f32 * 0.34, 0.0)
                        .with_scale(Vec3::splat(taper.max(0.3))),
                );
            }
        }
        PartFamily::Cilia => {
            let mesh = meshes.add(Capsule3d::new(0.05, 0.7).mesh().latitudes(6).longitudes(8));
            for i in 0..7 {
                let a = i as f32 / 7.0 * std::f32::consts::TAU;
                spawn_child(
                    &mut commands, root, mesh.clone(), material.clone(),
                    Transform::from_xyz(a.cos() * 0.35, 0.0, a.sin() * 0.35)
                        .with_rotation(Quat::from_rotation_z(a.cos() * 0.5)),
                );
            }
        }
        PartFamily::Mouth => {
            spawn_child(
                &mut commands, root, meshes.add(Torus::new(0.22, 0.55).mesh().major_resolution(24)),
                material.clone(), Transform::from_rotation(Quat::from_rotation_x(0.5)),
            );
        }
        PartFamily::Ram | PartFamily::Chemoreceptor => {
            spawn_child(
                &mut commands, root, meshes.add(Sphere::new(1.0).mesh().uv(20, 14)),
                materials.add(StandardMaterial {
                    base_color: Color::srgb(0.92, 0.94, 0.96),
                    perceptual_roughness: 0.08,
                    ..default()
                }),
                Transform::from_scale(Vec3::splat(0.6)),
            );
            spawn_child(
                &mut commands, root, meshes.add(Sphere::new(1.0).mesh().uv(16, 10)),
                material.clone(),
                Transform::from_xyz(0.0, 0.0, 0.45).with_scale(Vec3::splat(0.3)),
            );
        }
        PartFamily::Spike | PartFamily::Nematocyst => {
            spawn_child(
                &mut commands, root, meshes.add(Cone::new(0.32, 1.3).mesh().resolution(16)),
                material.clone(), Transform::from_xyz(0.0, -0.1, 0.0),
            );
        }
        PartFamily::Carapace | PartFamily::MucusCoat => {
            spawn_child(
                &mut commands, root, meshes.add(Sphere::new(1.0).mesh().uv(24, 14)),
                materials.add(StandardMaterial {
                    base_color: color.with_alpha(0.55),
                    emissive: LinearRgba::from(color) * 0.25,
                    alpha_mode: AlphaMode::Blend,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                }),
                Transform::from_scale(Vec3::new(0.85, 0.6, 0.85)),
            );
        }
        // Органеллы: пара сросшихся долек внутри клетки.
        _ => {
            let mesh = meshes.add(Sphere::new(1.0).mesh().uv(20, 14));
            for (offset, scale) in [
                (Vec3::new(0.0, 0.0, 0.0), 0.52),
                (Vec3::new(0.34, 0.14, 0.10), 0.34),
                (Vec3::new(-0.28, -0.16, -0.08), 0.28),
            ] {
                spawn_child(
                    &mut commands, root, mesh.clone(), material.clone(),
                    Transform::from_translation(offset).with_scale(Vec3::splat(scale)),
                );
            }
        }
    }
}

fn spawn_child(
    commands: &mut Commands,
    parent: Entity,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    transform: Transform,
) {
    let child = commands
        .spawn((
            transform,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            RenderLayers::layer(PREVIEW_LAYER),
        ))
        .id();
    commands.entity(parent).add_child(child);
}

/// Модель медленно поворачивается — так видно её со всех сторон.
pub fn spin_preview(time: Res<Time>, mut models: Query<&mut Transform, With<PreviewModel>>) {
    for mut transform in &mut models {
        transform.rotate_y(time.delta_secs() * 0.7);
        transform.rotation *= Quat::from_rotation_z((time.elapsed_secs() * 0.5).sin() * 0.004);
    }
}

/// Цвет плашки органа в сетке — тот же, которым он рисуется на теле.
pub fn swatch_color(index: usize) -> Color {
    part_color(PartKind::basic(PartFamily::ALL[index % PartFamily::ALL.len()]))
}
