//! The water column, the seabed and everything drifting in between.
//!
//! The look is driven by one palette table indexed by season, so Bloom, Hot, Storm
//! and Cold are four different places rather than four numbers in the HUD.

use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;
use cellborn_common::*;
use lightyear::prelude::Controlled;
use rand::Rng;

use crate::fx::NotShadowCaster;

pub const SEABED_Y: f32 = -2.6;
const ARENA: f32 = ARENA_HALF_EXTENT;
const SNOW_COUNT: usize = 900;
const SNOW_BOX: Vec3 = Vec3::new(58.0, 16.0, 58.0);

#[derive(Resource)]
pub struct ToxinAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

#[derive(Resource)]
pub struct WorldAssets {
    pub food_mesh: [Handle<Mesh>; 3],
    pub food_material: [Handle<StandardMaterial>; 3],
}

#[derive(Component)]
pub struct Snow {
    pub drift: Vec3,
    pub spin: f32,
}

#[derive(Component)]
pub struct Kelp {
    pub phase: f32,
    pub bend: f32,
}

#[derive(Component)]
pub struct FoodVisual {
    pub phase: f32,
}

/// Marks toxin clouds that already have a body.
#[derive(Component)]
pub struct CloudVisual;

/// One puff of the haze: a cloud is a dozen of these, drifting and turning
/// inside its radius. A single translucent sphere read as a bubble, not as
/// poisoned water.
#[derive(Component)]
pub struct CloudPuff {
    pub offset: Vec3,
    pub drift: Vec3,
    pub spin: f32,
    pub scale: f32,
    pub phase: f32,
}

/// Look of one season.
#[derive(Clone, Copy)]
pub struct Palette {
    pub water: Color,
    pub fog_density: f32,
    pub ambient: Color,
    pub ambient_strength: f32,
    pub sun: Color,
    pub sun_lux: f32,
    pub sun_dir: Vec3,
}

pub fn palette(season: Season) -> Palette {
    match season {
        // Green, rich, alive: light comes straight down through a plankton haze.
        Season::Bloom => Palette {
            water: Color::srgb(0.045, 0.20, 0.19),
            fog_density: 0.030,
            ambient: Color::srgb(0.55, 0.90, 0.75),
            ambient_strength: 220.0,
            sun: Color::srgb(0.85, 1.0, 0.85),
            sun_lux: 11000.0,
            sun_dir: Vec3::new(0.25, -1.0, 0.3),
        },
        // Warm, murky, close: visibility drops and everything turns brassy.
        Season::Hot => Palette {
            water: Color::srgb(0.13, 0.15, 0.09),
            fog_density: 0.055,
            ambient: Color::srgb(0.95, 0.80, 0.50),
            ambient_strength: 190.0,
            sun: Color::srgb(1.0, 0.85, 0.55),
            sun_lux: 13000.0,
            sun_dir: Vec3::new(0.6, -1.0, 0.1),
        },
        // Dark and violent: almost no light, heavy silt.
        Season::Storm => Palette {
            water: Color::srgb(0.035, 0.055, 0.085),
            fog_density: 0.080,
            ambient: Color::srgb(0.35, 0.45, 0.60),
            ambient_strength: 110.0,
            sun: Color::srgb(0.55, 0.65, 0.85),
            sun_lux: 4000.0,
            sun_dir: Vec3::new(-0.5, -1.0, -0.4),
        },
        // Deep blue, crisp and empty: long sight lines, low ambient.
        Season::Cold => Palette {
            water: Color::srgb(0.03, 0.09, 0.17),
            fog_density: 0.022,
            ambient: Color::srgb(0.45, 0.65, 0.95),
            ambient_strength: 150.0,
            sun: Color::srgb(0.70, 0.85, 1.0),
            sun_lux: 8000.0,
            sun_dir: Vec3::new(-0.2, -1.0, 0.5),
        },
    }
}

// --- procedural terrain ------------------------------------------------------

fn hash2(x: i32, z: i32) -> f32 {
    let mut n = x.wrapping_mul(374_761_393).wrapping_add(z.wrapping_mul(668_265_263));
    n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    ((n ^ (n >> 16)) & 0x7fff_ffff) as f32 / 0x7fff_ffff as f32
}

fn value_noise(x: f32, z: f32) -> f32 {
    let (xi, zi) = (x.floor(), z.floor());
    let (xf, zf) = (x - xi, z - zi);
    let (sx, sz) = (xf * xf * (3.0 - 2.0 * xf), zf * zf * (3.0 - 2.0 * zf));
    let (x0, z0) = (xi as i32, zi as i32);
    let n00 = hash2(x0, z0);
    let n10 = hash2(x0 + 1, z0);
    let n01 = hash2(x0, z0 + 1);
    let n11 = hash2(x0 + 1, z0 + 1);
    (n00 * (1.0 - sx) + n10 * sx) * (1.0 - sz) + (n01 * (1.0 - sx) + n11 * sx) * sz
}

/// Fractal noise: the seabed needs both dunes and grain to read as a surface.
pub fn terrain_height(x: f32, z: f32) -> f32 {
    let mut height = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 0.020;
    for _ in 0..4 {
        height += value_noise(x * frequency, z * frequency) * amplitude;
        amplitude *= 0.5;
        frequency *= 2.3;
    }
    (height - 0.9) * 2.6
}

fn seabed_mesh(half: f32, resolution: usize) -> Mesh {
    let mut positions = Vec::with_capacity((resolution + 1) * (resolution + 1));
    let mut uvs = Vec::with_capacity(positions.capacity());
    let mut indices = Vec::with_capacity(resolution * resolution * 6);
    let step = half * 2.0 / resolution as f32;
    for iz in 0..=resolution {
        for ix in 0..=resolution {
            let x = -half + ix as f32 * step;
            let z = -half + iz as f32 * step;
            positions.push([x, terrain_height(x, z), z]);
            uvs.push([ix as f32 / resolution as f32, iz as f32 / resolution as f32]);
        }
    }
    let stride = resolution + 1;
    for iz in 0..resolution {
        for ix in 0..resolution {
            let i = (iz * stride + ix) as u32;
            indices.extend_from_slice(&[i, i + stride as u32, i + 1]);
            indices.extend_from_slice(&[i + 1, i + stride as u32, i + stride as u32 + 1]);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh.compute_normals();
    mesh
}

// --- setup -------------------------------------------------------------------

pub fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let p = palette(Season::Bloom);

    commands.insert_resource(ClearColor(p.water));
    commands.insert_resource(GlobalAmbientLight {
        color: p.ambient,
        brightness: p.ambient_strength,
        ..default()
    });

    commands.spawn((
        DirectionalLight { illuminance: p.sun_lux, color: p.sun, ..default() },
        Transform::from_xyz(20.0, 40.0, 12.0).looking_to(p.sun_dir, Vec3::Y),
    ));

    // Seabed.
    commands.spawn((
        Transform::from_xyz(0.0, SEABED_Y, 0.0),
        Mesh3d(meshes.add(seabed_mesh(150.0, 150))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.10, 0.13, 0.12),
            perceptual_roughness: 0.96,
            reflectance: 0.05,
            ..default()
        })),
    ));

    let mut rng = rand::rng();

    // Rocks: irregular scaled spheres half-buried in the seabed.
    let rock_mesh = meshes.add(Sphere::new(1.0).mesh().uv(10, 7));
    let rock_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.17, 0.18),
        perceptual_roughness: 0.9,
        ..default()
    });
    for _ in 0..190 {
        let x = rng.random_range(-ARENA - 8.0..ARENA + 8.0);
        let z = rng.random_range(-ARENA - 8.0..ARENA + 8.0);
        let size = rng.random_range(0.4..2.2);
        commands.spawn((
            Transform::from_xyz(x, SEABED_Y + terrain_height(x, z) - size * 0.35, z)
                .with_scale(Vec3::new(size, size * rng.random_range(0.4..0.8), size))
                .with_rotation(Quat::from_rotation_y(rng.random_range(0.0..std::f32::consts::TAU))),
            Mesh3d(rock_mesh.clone()),
            MeshMaterial3d(rock_material.clone()),
        ));
    }

    // Kelp: blades that sway on the same current the silt drifts on.
    let blade_mesh = meshes.add(Cuboid::new(0.18, 1.0, 0.04));
    let kelp_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.13, 0.32, 0.18),
        emissive: LinearRgba::new(0.01, 0.05, 0.02, 1.0),
        perceptual_roughness: 0.8,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    for _ in 0..120 {
        let cx = rng.random_range(-ARENA..ARENA);
        let cz = rng.random_range(-ARENA..ARENA);
        for _ in 0..rng.random_range(3..7) {
            let x = cx + rng.random_range(-1.2..1.2);
            let z = cz + rng.random_range(-1.2..1.2);
            let height = rng.random_range(1.6..4.2);
            commands.spawn((
                Kelp { phase: rng.random_range(0.0..std::f32::consts::TAU), bend: rng.random_range(0.06..0.2) },
                Transform::from_xyz(x, SEABED_Y + terrain_height(x, z) + height * 0.5, z)
                    .with_scale(Vec3::new(1.0, height, 1.0))
                    .with_rotation(Quat::from_rotation_y(rng.random_range(0.0..std::f32::consts::TAU))),
                Mesh3d(blade_mesh.clone()),
                MeshMaterial3d(kelp_material.clone()),
                NotShadowCaster,
            ));
        }
    }

    // Arena boundary: a shimmering curtain, so the clamp looks like a rule of the
    // world instead of an invisible wall.
    let wall_mesh = meshes.add(Cuboid::new(ARENA * 2.0, 10.0, 0.12));
    let wall_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.35, 0.85, 0.90, 0.10),
        emissive: LinearRgba::new(0.10, 0.45, 0.50, 1.0),
        alpha_mode: AlphaMode::Blend,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    for (offset, rotation) in [
        (Vec3::new(0.0, 2.0, -ARENA), Quat::IDENTITY),
        (Vec3::new(0.0, 2.0, ARENA), Quat::IDENTITY),
        (Vec3::new(-ARENA, 2.0, 0.0), Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
        (Vec3::new(ARENA, 2.0, 0.0), Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
    ] {
        commands.spawn((
            Transform::from_translation(offset).with_rotation(rotation),
            Mesh3d(wall_mesh.clone()),
            MeshMaterial3d(wall_material.clone()),
            NotShadowCaster,
        ));
    }

    // Marine snow: the single cheapest thing that turns fog into water, and the
    // only reference that makes your own speed readable in open water.
    let snow_mesh = meshes.add(Sphere::new(1.0).mesh().uv(5, 4));
    let snow_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.80, 0.90, 0.88, 0.55),
        emissive: LinearRgba::new(0.15, 0.22, 0.20, 1.0),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    for _ in 0..SNOW_COUNT {
        let size = rng.random_range(0.02..0.075);
        commands.spawn((
            Snow {
                drift: Vec3::new(
                    rng.random_range(-0.25..0.25),
                    rng.random_range(-0.22..-0.04),
                    rng.random_range(-0.25..0.25),
                ),
                spin: rng.random_range(-1.0..1.0),
            },
            Transform::from_xyz(
                rng.random_range(-SNOW_BOX.x..SNOW_BOX.x),
                rng.random_range(-2.0..SNOW_BOX.y),
                rng.random_range(-SNOW_BOX.z..SNOW_BOX.z),
            )
            .with_scale(Vec3::splat(size)),
            Mesh3d(snow_mesh.clone()),
            MeshMaterial3d(snow_material.clone()),
            NotShadowCaster,
        ));
    }

    // Toxin haze: one shared material for every cloud.
    commands.insert_resource(ToxinAssets {
        mesh: meshes.add(Sphere::new(1.0).mesh().uv(20, 12)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgba(0.60, 0.28, 0.74, 0.10),
            emissive: LinearRgba::new(0.035, 0.008, 0.05, 1.0),
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
    });

    // Food assets, shared by every nutrient of the same kind.
    commands.insert_resource(WorldAssets {
        food_mesh: [
            meshes.add(Sphere::new(1.0).mesh().uv(8, 6)),
            meshes.add(Sphere::new(1.0).mesh().uv(10, 7)),
            meshes.add(Sphere::new(1.0).mesh().uv(8, 6)),
        ],
        food_material: [
            materials.add(food_material(Color::srgb(0.60, 0.90, 0.95), 1.6)),
            materials.add(food_material(Color::srgb(0.45, 0.90, 0.35), 1.9)),
            materials.add(food_material(Color::srgb(0.80, 0.65, 0.35), 1.2)),
        ],
    });
}

fn food_material(color: Color, glow: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        emissive: LinearRgba::from(color) * glow,
        perceptual_roughness: 0.3,
        ..default()
    }
}

// --- per-frame behaviour -----------------------------------------------------

/// Silt drifts on a slow current and wraps around the camera, so the water never
/// runs out of particles however far you swim.
pub fn drift_snow(
    time: Res<Time>,
    camera: Query<&Transform, (With<crate::MainCamera>, Without<Snow>)>,
    mut snow: Query<(&Snow, &mut Transform), Without<crate::MainCamera>>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let centre = camera.single().map(|c| c.translation).unwrap_or(Vec3::ZERO);
    // A single global current, so silt and kelp visibly agree with each other.
    let current = Vec3::new((t * 0.13).sin() * 0.35, 0.0, (t * 0.09).cos() * 0.28);
    for (flake, mut transform) in &mut snow {
        transform.translation += (flake.drift + current) * dt;
        transform.rotate_y(flake.spin * dt);
        let offset = transform.translation - centre;
        if offset.x.abs() > SNOW_BOX.x {
            transform.translation.x -= offset.x.signum() * SNOW_BOX.x * 2.0;
        }
        if offset.z.abs() > SNOW_BOX.z {
            transform.translation.z -= offset.z.signum() * SNOW_BOX.z * 2.0;
        }
        if transform.translation.y < SEABED_Y {
            transform.translation.y = SNOW_BOX.y;
        }
        if transform.translation.y > SNOW_BOX.y {
            transform.translation.y = SEABED_Y + 0.2;
        }
    }
}

pub fn sway_kelp(time: Res<Time>, mut kelp: Query<(&Kelp, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (blade, mut transform) in &mut kelp {
        let sway = (t * 0.8 + blade.phase).sin() * blade.bend;
        let lean = (t * 0.5 + blade.phase * 0.6).cos() * blade.bend * 0.6;
        transform.rotation = Quat::from_rotation_z(sway) * Quat::from_rotation_x(lean);
    }
}

/// Gives every replicated nutrient a body the first time it is seen.
pub fn spawn_food_visuals(
    mut commands: Commands,
    assets: Res<WorldAssets>,
    nutrients: Query<(Entity, &Nutrient, &FoodPosition), Without<FoodVisual>>,
) {
    let mut rng = rand::rng();
    for (entity, nutrient, position) in &nutrients {
        let index = match nutrient.kind {
            FoodKind::Plankton => 0,
            FoodKind::Algae => 1,
            FoodKind::Detritus => 2,
        };
        commands.entity(entity).insert((
            FoodVisual { phase: rng.random_range(0.0..std::f32::consts::TAU) },
            Transform::from_translation(position.0)
                .with_scale(Vec3::splat(nutrient.kind.radius())),
            Visibility::default(),
            Mesh3d(assets.food_mesh[index].clone()),
            MeshMaterial3d(assets.food_material[index].clone()),
            NotShadowCaster,
        ));
    }
}

/// Food bobs and pulses; a still glowing dot reads as UI, a moving one as life.
pub fn animate_food(
    time: Res<Time>,
    mut food: Query<(&FoodVisual, &Nutrient, &FoodPosition, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    for (visual, nutrient, position, mut transform) in &mut food {
        let bob = (t * 1.3 + visual.phase).sin() * 0.12;
        let pulse = 1.0 + (t * 2.1 + visual.phase).sin() * 0.12;
        transform.translation = position.0 + Vec3::Y * bob;
        transform.scale = Vec3::splat(nutrient.kind.radius() * pulse);
        transform.rotate_y(0.4 * time.delta_secs());
    }
}

/// Cross-fades the whole look when the season changes.
pub fn apply_season(
    time: Res<Time>,
    water: Res<WorldUpdate>,
    // Глаза и хеморецепторы игрока: они разгоняют муть.
    player: Query<&PlayerGenome, With<Controlled>>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform)>,
    mut fog: Query<&mut DistanceFog>,
) {
    let target = palette(water.season);
    let k = (time.delta_secs() * 0.6).min(1.0);

    clear.0 = mix(clear.0, target.water, k);
    ambient.color = mix(ambient.color, target.ambient, k);
    ambient.brightness = ambient.brightness.lerp(target.ambient_strength, k);

    if let Ok((mut light, mut transform)) = sun.single_mut() {
        light.color = mix(light.color, target.sun, k);
        light.illuminance = light.illuminance.lerp(target.sun_lux, k);
        let current = transform.forward().as_vec3();
        let wanted = target.sun_dir.normalize();
        transform.look_to(current.lerp(wanted, k).normalize_or(wanted), Vec3::Y);
    }

    if let Ok(mut fog) = fog.single_mut() {
        fog.color = mix(fog.color, target.water, k);
        if let FogFalloff::ExponentialSquared { density } = &mut fog.falloff {
            // Органы чувств буквально дают зрение: чем их больше, тем дальше
            // видно сквозь муть.
            //
            // До этого глаз только раздвигал подсветку ближней еды — а еду и
            // так видно всю, что помещается на экран, так что орган был
            // украшением. Туман — то единственное, что реально мешает смотреть,
            // и особенно в шторм, где его вдвое больше обычного.
            let clarity = player
                .single()
                .map(|genome| sense_range(&OrganismState::from_genome(genome.0.clone())))
                .unwrap_or(BASE_SENSE_RANGE);
            let sharpened = target.fog_density * fog_factor(clarity);
            *density = density.lerp(sharpened, k);
        }
    }
}

/// Во сколько раз реже становится муть при такой чувствительности.
///
/// Голое тело видит на [`BASE_SENSE_RANGE`] и получает множитель 1: мир для
/// него выглядит ровно так, как задуман сезоном. Дальше зрение улучшается, но
/// с насыщением — иначе десяток глаз просто отменил бы погоду.
pub fn fog_factor(sense: f32) -> f32 {
    let extra = (sense - BASE_SENSE_RANGE).max(0.0);
    // Половина мути уходит примерно на двадцати единицах сверх базы: это три
    // глаза или пять хеморецепторов.
    (1.0 / (1.0 + extra / 20.0)).clamp(0.35, 1.0)
}

fn mix(a: Color, b: Color, k: f32) -> Color {
    let (a, b) = (LinearRgba::from(a), LinearRgba::from(b));
    Color::from(LinearRgba {
        red: a.red.lerp(b.red, k),
        green: a.green.lerp(b.green, k),
        blue: a.blue.lerp(b.blue, k),
        alpha: a.alpha.lerp(b.alpha, k),
    })
}

/// Gives every replicated toxin cloud a visible haze: a scatter of soft puffs
/// rather than one sphere.
pub fn spawn_cloud_visuals(
    mut commands: Commands,
    assets: Res<ToxinAssets>,
    clouds: Query<(Entity, &ToxinCloud), Without<CloudVisual>>,
) {
    let mut rng = rand::rng();
    for (entity, cloud) in &clouds {
        commands.entity(entity).insert((
            CloudVisual,
            Transform::from_translation(cloud.position),
            Visibility::default(),
        ));

        for _ in 0..26 {
            // Puffs sit anywhere inside the sphere, biased toward the middle so
            // the haze has a dense core and ragged edges.
            let direction = Vec3::new(
                rng.random_range(-1.0..1.0),
                rng.random_range(-0.55..0.55),
                rng.random_range(-1.0..1.0),
            )
            .normalize_or(Vec3::X);
            let radius = rng.random_range(0.0f32..1.0).powf(0.6);
            let puff = commands
                .spawn((
                    CloudPuff {
                        offset: direction * radius,
                        drift: Vec3::new(
                            rng.random_range(-0.05..0.05),
                            rng.random_range(0.01..0.06),
                            rng.random_range(-0.05..0.05),
                        ),
                        spin: rng.random_range(-0.5..0.5),
                        scale: rng.random_range(0.18..0.46),
                        phase: rng.random_range(0.0..std::f32::consts::TAU),
                    },
                    Transform::default(),
                    Mesh3d(assets.mesh.clone()),
                    MeshMaterial3d(assets.material.clone()),
                    NotShadowCaster,
                ))
                .id();
            commands.entity(entity).add_child(puff);
        }
    }
}

/// The haze roils: puffs drift, turn and breathe, and the whole thing thins out
/// as the cloud disperses.
pub fn animate_clouds(
    time: Res<Time>,
    player: Query<&PlayerGenome, With<Controlled>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut clouds: Query<(&ToxinCloud, &Children, &mut Transform), With<CloudVisual>>,
    mut puffs: Query<
        (&mut CloudPuff, &mut Transform, &MeshMaterial3d<StandardMaterial>),
        Without<CloudVisual>,
    >,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();

    // Своя стойкость к яду: чем она выше, тем прозрачнее для тебя чужая дымка.
    // Облако рисуется настолько густым, насколько оно тебе опасно, — так по
    // одному взгляду видно, лезть туда или нет.
    let resistance = player
        .single()
        .map(|genome| OrganismState::from_genome(genome.0.clone()).toxin_resistance)
        .unwrap_or(BASE_TOXIN_RESISTANCE);

    for (cloud, children, mut root) in &mut clouds {
        // Само облако стоит там, где его оставили. Раньше эта строка пыталась
        // найти CloudVisual среди детей, но он висит на самой сущности облака,
        // и позиция не обновлялась вовсе.
        root.translation = cloud.position;

        let menace = ((cloud.strength - resistance) / cloud.strength.max(0.01)).clamp(0.0, 1.0);
        for child in children.iter() {
            let Ok((mut puff, mut transform, material)) = puffs.get_mut(child) else { continue };
            // Безобидная для тебя дымка почти не мешает смотреть; смертельная
            // висит плотной кляксой.
            if let Some(mut material) = materials.get_mut(material.id()) {
                // Даже безобидная для тебя дымка остаётся видимой: свой
                // собственный след надо видеть, иначе непонятно, работает ли
                // железа вообще.
                let alpha = 0.10 + menace * 0.26;
                let current = material.base_color.alpha();
                if (current - alpha).abs() > 0.005 {
                    material.base_color = material.base_color.with_alpha(alpha);
                }
            }
            // Slow internal convection, plus a rise: poison drifts upward.
            let (drift, scale, phase, spin) = (puff.drift, puff.scale, puff.phase, puff.spin);
            puff.offset += drift * dt;
            if puff.offset.length() > 1.15 {
                puff.offset = -puff.offset * 0.8;
            }
            let breathe = 1.0 + (t * 0.7 + phase).sin() * 0.18;
            // Смещение внутри облака, а не координата в мире: клуб — ребёнок
            // облака, и его позиция считается от родителя.
            //
            // Здесь жил баг, из-за которого облака оказывались вдвое дальше от
            // центра карты, чем должны: к позиции родителя прибавлялась она же.
            // У нуля это незаметно, а на краю арены облако уезжало за горизонт —
            // и собственный ядовитый след игрок не видел никогда.
            transform.translation = puff.offset * cloud.radius;
            // Клубы не шары: слегка сплюснуты и вытянуты по-разному, иначе
            // облако читается как гроздь пузырей.
            transform.scale = Vec3::new(
                cloud.radius * scale * breathe * 1.25,
                cloud.radius * scale * breathe * 0.7,
                cloud.radius * scale * breathe * 1.1,
            );
            transform.rotate_y(spin * dt);
        }
    }
}
