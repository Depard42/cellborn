//! The organism: a modular body assembled from the genome, deformed and animated
//! procedurally.
//!
//! Nothing here is keyframed. Every motion comes from the organism's own state —
//! speed drives the flagellum beat and the squash-and-stretch, energy drives the
//! colour and the breathing depth, bites drive the mouth. That is what makes a
//! genome-built creature look alive without an animator.

use bevy::prelude::*;
use cellborn_common::*;
use lightyear::prelude::*;

use crate::fx::{spawn_burst, FxAssets, NotShadowCaster};

/// Root marker on the replicated organism entity.
#[derive(Component)]
pub struct Body {
    /// Changes when the genome changes, which is what triggers a rebuild.
    pub signature: u64,
    pub phase: f32,
    pub last_position: Vec3,
    pub speed: f32,
    pub bites: u32,
    pub hits: u32,
    pub divisions: u32,
    pub eat_timer: f32,
    pub hurt_timer: f32,
    /// Направление и глубина вмятины от соседа, в локальных осях тела.
    pub dent: Vec3,
    pub dent_depth: f32,
    pub split_timer: f32,
    pub radius: f32,
    /// Kept so the membrane can be recoloured when kinship changes.
    pub material: Handle<StandardMaterial>,
}

/// A blob of cytoplasm drifting inside the cell: what makes it read as something
/// alive under a microscope rather than a coloured ball.
#[derive(Component)]
pub struct Organelle {
    pub orbit: Vec3,
    pub speed: f32,
    pub phase: f32,
}

/// How another organism relates to the one we control.
///
/// Цвет обязан отвечать на один вопрос: ударит ли это тело меня при касании.
/// Поэтому вражда проверяется **первой**, и только потом родство.
///
/// Раньше было наоборот, и это врало. Сородич красился зелёным «своя семья» до
/// того, как проверялась вражда, — а родня дерётся, разойдясь больше чем на
/// `kin_split_threshold` органов: род раскалывается, это правило игры, а не
/// исключение. Ветка с Мутатором расходится быстрее прочих и первой начинала
/// бить «своих», оставаясь при этом зелёной.
///
/// Пороги приходят от сервера: он судит по своему конфигу, и вшитые в клиент
/// числа были второй причиной, по которой цвет мог не совпасть с уроном.
pub fn relation_color(
    mine: Option<&Genome>,
    theirs: &Genome,
    controlled: bool,
    water: &WorldUpdate,
) -> Color {
    if controlled {
        return Color::srgb(0.46, 0.88, 0.74);
    }
    let Some(m) = mine else {
        return Color::srgb(0.86, 0.72, 0.40);
    };
    if hostile_with(m, theirs, water.aggression_threshold, water.kin_split_threshold) {
        // Ударит при касании — неважно, родня это или чужак.
        return Color::srgb(0.92, 0.38, 0.32);
    }
    if m.lineage == theirs.lineage {
        // Своя колония: пока не разошлись — безопасна.
        return Color::srgb(0.55, 0.85, 0.55);
    }
    // Чужак, но слишком похож, чтобы драться.
    Color::srgb(0.86, 0.72, 0.40)
}

/// Докуда цитоплазме позволено дрейфовать, в долях радиуса тела.
///
/// Органеллы — дети мембраны, то есть живут в единичной сфере. Их орбита плюс
/// дрейф доходили почти до единицы, а с собственным размером шарика и вовсе
/// выходили за неё — и внутренности вылезали сквозь оболочку наружу. Тем
/// заметнее, чем сильнее тело сплющивалось при столкновении.
const CYTOPLASM_REACH: f32 = 0.55;

/// На какой глубине ползает паразит.
///
/// У самой стенки, а не в середине: в центре он терялся среди органелл, стоило
/// отрастить их побольше, а по внутренней стенке он читается силуэтом на фоне
/// мембраны — и сразу видно, что этой клеткой управляет человек.
const PARASITE_REACH: f32 = 0.74;

/// The deformable core. Parts hang off it, so they squash with the body.
#[derive(Component)]
pub struct Membrane;

/// One link of the flagellum whip.
#[derive(Component)]
pub struct TailSegment {
    pub index: usize,
    pub spacing: f32,
}

#[derive(Component)]
pub struct CiliaHair {
    pub phase: f32,
    pub base_rotation: Quat,
}

#[derive(Component)]
pub struct MouthVisual;

pub fn genome_signature(genome: &Genome) -> u64 {
    let mut hash: u64 = 1469598103934665603;
    for part in &genome.parts {
        hash ^= part.kind.index() as u64 + 1;
        hash = hash.wrapping_mul(1099511628211);
    }
    hash ^= genome.parts.len() as u64;
    hash
}

pub fn part_color(kind: PartKind) -> Color {
    use PartFamily::*;
    let base = match kind.family {
        Membrane => Color::srgb(0.50, 0.88, 0.74),
        Flagellum => Color::srgb(0.86, 0.83, 0.52),
        Cilia => Color::srgb(0.80, 0.82, 0.66),
        Mouth => Color::srgb(0.82, 0.32, 0.36),
        Eye => Color::srgb(0.16, 0.30, 0.78),
        Spike => Color::srgb(0.88, 0.90, 0.92),
        ToxinGland => Color::srgb(0.62, 0.24, 0.78),
        Osmoregulator => Color::srgb(0.30, 0.72, 0.92),
        ThermalMembrane => Color::srgb(0.95, 0.52, 0.24),
        Photosynthesis => Color::srgb(0.36, 0.82, 0.32),
        StorageVacuole => Color::srgb(0.74, 0.70, 0.42),
        MucusCoat => Color::srgb(0.60, 0.68, 0.72),
        Gill => Color::srgb(0.90, 0.45, 0.55),
        Divisome => Color::srgb(0.95, 0.85, 0.55),
        Mutator => Color::srgb(0.85, 0.35, 0.85),
        Pseudopod => Color::srgb(0.70, 0.60, 0.45),
        Nematocyst => Color::srgb(0.98, 0.75, 0.30),
        Symbiont => Color::srgb(0.45, 0.75, 0.60),
        Chemoreceptor => Color::srgb(0.55, 0.80, 0.85),
        Carapace => Color::srgb(0.55, 0.55, 0.60),
    };
    // The variant shifts the shade, so two flagella of different builds are
    // distinguishable on the body without a legend.
    let lift = match kind.variant {
        PartVariant::Basic => 1.0,
        PartVariant::Small => 0.80,
        PartVariant::Large => 1.15,
        PartVariant::Potent => 1.30,
        PartVariant::Thrifty => 0.90,
        PartVariant::Fragile => 1.20,
        PartVariant::Dense => 0.70,
        PartVariant::Twin => 1.10,
        PartVariant::Feral => 1.25,
        PartVariant::Refined => 1.40,
    };
    let c = LinearRgba::from(base);
    Color::from(LinearRgba::new(
        (c.red * lift).min(1.0),
        (c.green * lift).min(1.0),
        (c.blue * lift).min(1.0),
        1.0,
    ))
}

/// Floating health bar over another organism, so you can see whether the thing
/// biting you is actually dying.
#[derive(Component)]
pub struct HealthBar;

#[derive(Component)]
pub struct HealthFill;

/// Builds — or rebuilds after a mutation — the body of a replicated organism.
pub fn build_bodies(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    organisms: Query<
        (
            Entity,
            &PlayerGenome,
            &PlayerVitals,
            &PlayerPosition,
            Has<Controlled>,
            // Только у игроков есть сетевой идентификатор: у ботов его нет.
            Has<PlayerId>,
            Option<&Body>,
        ),
        Or<(Without<Body>, Changed<PlayerGenome>)>,
    >,
    mine: Query<&PlayerGenome, With<Controlled>>,
    water: Res<WorldUpdate>,
    children: Query<&Children>,
) {
    for (entity, genome, vitals, position, controlled, is_player, body) in &organisms {
        let signature = genome_signature(&genome.0);
        if body.is_some_and(|b| b.signature == signature) {
            continue;
        }

        // A mutation rebuilds the body: drop the old meshes first.
        if let Ok(existing) = children.get(entity) {
            for child in existing.iter() {
                commands.entity(child).despawn();
            }
        }

        let radius = body_radius(vitals.mass);
        let tint = relation_color(mine.iter().next().map(|g| &g.0), &genome.0, controlled, &water);

        let membrane_material = materials.add(StandardMaterial {
            base_color: tint.with_alpha(0.62),
            emissive: LinearRgba::from(tint) * 0.25,
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.25,
            metallic: 0.0,
            // A thin, wet-looking shell rather than a matte ball.
            reflectance: 0.35,
            double_sided: true,
            cull_mode: None,
            ..default()
        });

        commands.entity(entity).insert((
            Body {
                signature,
                phase: (entity.to_bits() % 100) as f32 * 0.31,
                last_position: position.0,
                speed: 0.0,
                bites: 0,
                hits: 0,
                divisions: 0,
                eat_timer: 0.0,
                hurt_timer: 0.0,
                dent: Vec3::X,
                dent_depth: 0.0,
                split_timer: 0.0,
                radius,
                material: membrane_material.clone(),
            },
            Transform::from_translation(position.0),
            Visibility::default(),
        ));

        let membrane = commands
            .spawn((
                Membrane,
                Transform::from_scale(Vec3::splat(radius)),
                Visibility::default(),
                Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(32, 18))),
                MeshMaterial3d(membrane_material),
            ))
            .id();
        commands.entity(entity).add_child(membrane);

        // The nucleus makes the translucent cell read as a volume, not a bubble.
        let nucleus = commands
            .spawn((
                Transform::from_xyz(0.0, 0.0, 0.1).with_scale(Vec3::splat(0.34)),
                Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(16, 10))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.30, 0.55, 0.52).with_alpha(0.85),
                    emissive: LinearRgba::new(0.05, 0.16, 0.14, 1.0),
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                })),
            ))
            .id();
        commands.entity(membrane).add_child(nucleus);

        // Клетками игроков управляет не программа, и это должно быть видно:
        // внутри у них сидит паразит — вытянутое ядро с двумя усиками,
        // которого нет ни у одного бота.
        if is_player {
            let parasite = commands
                .spawn((
                    Parasite { phase: (entity.to_bits() % 60) as f32 * 0.11 },
                    Transform::from_xyz(0.18, 0.0, -0.12).with_scale(Vec3::new(0.22, 0.14, 0.42)),
                    Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(14, 9))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(0.98, 0.28, 0.58),
                        // Паразит должен читаться сквозь мутную мембрану, поэтому
                        // светится заметно ярче остальных органелл.
                        emissive: LinearRgba::new(1.6, 0.15, 0.6, 1.0),
                        perceptual_roughness: 0.2,
                        ..default()
                    })),
                    NotShadowCaster,
                ))
                .id();
            let tail_mesh = meshes.add(Capsule3d::new(0.09, 0.9).mesh().latitudes(4).longitudes(6));
            let tail_material = materials.add(StandardMaterial {
                base_color: Color::srgb(0.99, 0.60, 0.78),
                emissive: LinearRgba::new(1.1, 0.25, 0.5, 1.0),
                ..default()
            });
            for side in [-1.0f32, 1.0] {
                let whisker = commands
                    .spawn((
                        Transform::from_xyz(side * 0.5, 0.0, 0.9)
                            .with_rotation(Quat::from_rotation_x(1.2) * Quat::from_rotation_z(side * 0.4)),
                        Mesh3d(tail_mesh.clone()),
                        MeshMaterial3d(tail_material.clone()),
                        NotShadowCaster,
                    ))
                    .id();
                commands.entity(parasite).add_child(whisker);
            }
            commands.entity(membrane).add_child(parasite);
        }

        // Cytoplasm: vacuoles and granules drifting inside the cell, plus the
        // faint haze that keeps the interior from looking empty.
        let organelle_mesh = meshes.add(Sphere::new(1.0).mesh().uv(10, 7));
        for i in 0..6 {
            let a = i as f32 * 1.7;
            let organelle = commands
                .spawn((
                    Organelle {
                        orbit: Vec3::new(a.sin() * 0.45, (a * 0.7).cos() * 0.35, a.cos() * 0.45),
                        speed: 0.25 + (i as f32 % 3.0) * 0.12,
                        phase: a,
                    },
                    Transform::from_scale(Vec3::splat(0.10 + (i as f32 % 3.0) * 0.05)),
                    Mesh3d(organelle_mesh.clone()),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgba(0.60, 0.85, 0.72, 0.55),
                        emissive: LinearRgba::new(0.06, 0.14, 0.10, 1.0),
                        alpha_mode: AlphaMode::Blend,
                        ..default()
                    })),
                    NotShadowCaster,
                ))
                .id();
            commands.entity(membrane).add_child(organelle);
        }

        // Другие организмы носят полоску здоровья: без неё непонятно, наносишь
        // ли ты урон и умирает ли вообще то, что тебя кусает.
        if !controlled {
            let bar = commands
                .spawn((
                    HealthBar,
                    Transform::from_xyz(0.0, radius + 0.95, 0.0),
                    Visibility::default(),
                ))
                .id();
            let back = commands
                .spawn((
                    Transform::from_scale(Vec3::new(1.9, 0.30, 0.04)),
                    Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgba(0.02, 0.03, 0.04, 0.75),
                        unlit: true,
                        alpha_mode: AlphaMode::Blend,
                        ..default()
                    })),
                    NotShadowCaster,
                ))
                .id();
            let fill = commands
                .spawn((
                    HealthFill,
                    Transform::from_xyz(0.0, 0.0, 0.03).with_scale(Vec3::new(1.78, 0.20, 0.04)),
                    Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(0.40, 0.90, 0.45),
                        emissive: LinearRgba::new(0.2, 0.5, 0.2, 1.0),
                        unlit: true,
                        ..default()
                    })),
                    NotShadowCaster,
                ))
                .id();
            commands.entity(bar).add_children(&[back, fill]);
            commands.entity(entity).add_child(bar);
        }

        for (index, part) in genome.0.parts.iter().enumerate() {
            if part.kind.family == PartFamily::Membrane {
                continue;
            }
            // Мембрана — единичная сфера, растянутая настоящим радиусом тела,
            // поэтому части кладутся в долях от неё, а радиус здесь не нужен
            // вовсе.
            //
            // Раньше сюда шла запечённая в геном позиция, поделённая на радиус.
            // Но запекалась она по прикидочному радиусу (`push_part` не знает
            // массы, которая от неё же и зависит), и чем больше вырастало тело,
            // тем сильнее прикидка отставала: внешние органы уползали внутрь
            // пузыря и пропадали из виду.
            let base = slot_facing(part.kind.family, index) * slot_depth(part.kind.family);
            spawn_part(
                &mut commands,
                &mut meshes,
                &mut materials,
                membrane,
                part.kind,
                base,
                part.rotation,
            );
        }
    }
}

fn spawn_part(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Entity,
    kind: PartKind,
    base: Vec3,
    rotation: Quat,
) {
    let color = part_color(kind);
    let material = materials.add(StandardMaterial {
        base_color: color,
        emissive: LinearRgba::from(color) * if kind.is_external() { 0.15 } else { 0.5 },
        perceptual_roughness: 0.45,
        alpha_mode: if kind.is_external() { AlphaMode::Opaque } else { AlphaMode::Blend },
        ..default()
    });

    let root = commands
        .spawn((
            Transform::from_translation(base).with_rotation(rotation),
            Visibility::default(),
        ))
        .id();
    commands.entity(parent).add_child(root);

    match kind.family {
        // A whip of tapering segments, animated as a travelling wave.
        PartFamily::Flagellum => {
            let segments = 7;
            let mesh = meshes.add(Capsule3d::new(0.10, 0.16).mesh().latitudes(6).longitudes(8));
            for i in 0..segments {
                let taper = 1.0 - i as f32 / (segments as f32 + 1.5);
                let segment = commands
                    .spawn((
                        TailSegment { index: i, spacing: 0.30 },
                        Transform::from_xyz(0.0, 0.30 * (i as f32 + 1.0), 0.0)
                            .with_scale(Vec3::splat(taper.max(0.25))),
                        Visibility::default(),
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        NotShadowCaster,
                    ))
                    .id();
                commands.entity(root).add_child(segment);
            }
        }
        // A crown of beating hairs around the attachment point.
        PartFamily::Cilia => {
            let mesh = meshes.add(Capsule3d::new(0.035, 0.42).mesh().latitudes(4).longitudes(6));
            for i in 0..6 {
                let a = i as f32 / 6.0 * std::f32::consts::TAU;
                let tilt = Quat::from_rotation_y(a) * Quat::from_rotation_x(0.5);
                let hair = commands
                    .spawn((
                        CiliaHair { phase: a, base_rotation: tilt },
                        Transform::from_rotation(tilt).with_translation(Vec3::new(
                            a.cos() * 0.12,
                            0.22,
                            a.sin() * 0.12,
                        )),
                        Visibility::default(),
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        NotShadowCaster,
                    ))
                    .id();
                commands.entity(root).add_child(hair);
            }
        }
        PartFamily::Mouth => {
            let ring = commands
                .spawn((
                    MouthVisual,
                    Transform::from_xyz(0.0, 0.10, 0.0).with_rotation(Quat::from_rotation_x(0.0)),
                    Visibility::default(),
                    Mesh3d(meshes.add(Torus::new(0.16, 0.30).mesh().major_resolution(16))),
                    MeshMaterial3d(material.clone()),
                ))
                .id();
            commands.entity(root).add_child(ring);
        }
        PartFamily::Eye => {
            let ball = commands
                .spawn((
                    Transform::from_xyz(0.0, 0.14, 0.0).with_scale(Vec3::splat(0.22)),
                    Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(12, 8))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(0.92, 0.94, 0.96),
                        perceptual_roughness: 0.1,
                        ..default()
                    })),
                ))
                .id();
            let iris = commands
                .spawn((
                    Transform::from_xyz(0.0, 0.26, 0.0).with_scale(Vec3::splat(0.12)),
                    Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(10, 6))),
                    MeshMaterial3d(material.clone()),
                ))
                .id();
            commands.entity(root).add_child(ball);
            commands.entity(root).add_child(iris);
        }
        PartFamily::Spike => {
            let spike = commands
                .spawn((
                    Transform::from_xyz(0.0, 0.26, 0.0),
                    Mesh3d(meshes.add(Cone::new(0.14, 0.52).mesh().resolution(10))),
                    MeshMaterial3d(material.clone()),
                ))
                .id();
            commands.entity(root).add_child(spike);
        }
        // Everything else is an organelle: a soft blob, brighter when internal so it
        // glows faintly through the membrane.
        _ => {
            let scale = if kind.is_external() { 0.26 } else { 0.30 };
            let blob = commands
                .spawn((
                    Transform::from_xyz(0.0, 0.05, 0.0).with_scale(Vec3::new(
                        scale,
                        scale * 0.8,
                        scale,
                    )),
                    Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(12, 8))),
                    MeshMaterial3d(material.clone()),
                    NotShadowCaster,
                ))
                .id();
            commands.entity(root).add_child(blob);
        }
    }
}

/// Паразит: органелла, которая есть только у клеток под управлением игроков.
///
/// Игроков в море единицы, и отличить их от ботов иначе никак: снаружи те же
/// органы, тот же цвет по родству. Паразит — единственная деталь, которой у
/// ботов не бывает, поэтому по нему видно, что перед тобой не программа.
#[derive(Component)]
pub struct Parasite {
    pub phase: f32,
}

/// Squashes bodies where they press against each other.
///
/// Cells are soft: bumping into someone should visibly dent you, not stop you
/// like a wall. The dent is presentation only — positions are already separated
/// by the simulation, this just makes the contact readable.
pub fn deform_on_contact(
    time: Res<Time>,
    positions: Query<(Entity, &PlayerPosition, &PlayerVitals)>,
    mut bodies: Query<(Entity, &mut Body, &PlayerPosition, &PlayerVitals, &Transform)>,
) {
    let dt = time.delta_secs();
    let neighbours: Vec<(Entity, Vec3, f32)> = positions
        .iter()
        .map(|(e, p, v)| (e, p.0, body_radius(v.mass)))
        .collect();

    for (entity, mut body, position, vitals, transform) in &mut bodies {
        let radius = body_radius(vitals.mass);
        let mut deepest = 0.0;
        let mut direction = Vec3::X;

        for (other, other_position, other_radius) in &neighbours {
            if *other == entity {
                continue;
            }
            let contact = radius + other_radius;
            let offset = *other_position - position.0;
            let distance = offset.length();
            if distance >= contact || distance < 1e-4 {
                continue;
            }
            let depth = (contact - distance) / contact;
            if depth > deepest {
                deepest = depth;
                // The dent is stored in the body's own axes, so it stays on the
                // side that is actually being pressed while the cell turns.
                direction = transform.rotation.inverse() * (offset / distance);
            }
        }

        // Клетки мягкие: вмятина набирается быстро и распускается медленнее,
        // как настоящая мембрана.
        let speed = if deepest > body.dent_depth { 14.0 } else { 5.0 };
        body.dent_depth = body.dent_depth.lerp(deepest.min(0.9), (dt * speed).min(1.0));
        if deepest > 0.0 {
            body.dent = direction;
        }
    }
}

/// Keeps the membrane colour honest about kinship: family, stranger, or enemy.
pub fn recolor_bodies(
    mut materials: ResMut<Assets<StandardMaterial>>,
    water: Res<WorldUpdate>,
    mine: Query<&PlayerGenome, With<Controlled>>,
    bodies: Query<(&Body, &PlayerGenome, Has<Controlled>)>,
) {
    let mine = mine.iter().next().map(|g| &g.0);
    for (body, genome, controlled) in &bodies {
        let tint = relation_color(mine, &genome.0, controlled, &water);
        let Some(mut material) = materials.get_mut(body.material.id()) else { continue; };
        if material.base_color != tint.with_alpha(0.62) {
            material.base_color = tint.with_alpha(0.62);
            material.emissive = LinearRgba::from(tint) * 0.25;
        }
    }
}

/// Drives every procedural motion from the organism's own state.
#[allow(clippy::too_many_arguments)]
pub fn animate_bodies(
    mut commands: Commands,
    time: Res<Time>,
    fx: Res<FxAssets>,
    mut organisms: Query<(
        Entity,
        &mut Body,
        &PlayerPosition,
        &PlayerVitals,
        // Энергия есть только у тел игроков: ботам её никто не реплицирует.
        // Она здесь ради дыхания — голодная клетка дышит чаще и мельче.
        Option<&PlayerEnergy>,
        &PlayerProgress,
        &mut Transform,
    )>,
    children: Query<&Children>,
    mut membranes: Query<&mut Transform, (With<Membrane>, Without<Body>)>,
    mut tails: Query<(&TailSegment, &mut Transform), (Without<Body>, Without<Membrane>)>,
    mut hairs: Query<
        (&CiliaHair, &mut Transform),
        (Without<Body>, Without<Membrane>, Without<TailSegment>),
    >,
    mut mouths: Query<
        &mut Transform,
        (
            With<MouthVisual>,
            Without<Body>,
            Without<Membrane>,
            Without<TailSegment>,
            Without<CiliaHair>,
        ),
    >,
    mut parasites: Query<
        (&Parasite, &mut Transform),
        (
            Without<Body>,
            Without<Membrane>,
            Without<TailSegment>,
            Without<CiliaHair>,
            Without<MouthVisual>,
            Without<Organelle>,
        ),
    >,
    mut organelles: Query<
        (&Organelle, &mut Transform),
        (
            Without<Body>,
            Without<Membrane>,
            Without<TailSegment>,
            Without<CiliaHair>,
            Without<MouthVisual>,
        ),
    >,
) {
    let dt = time.delta_secs().max(1e-5);

    for (entity, mut body, position, vitals, energy, progress, mut transform) in &mut organisms {
        // Velocity is measured, not simulated: presentation never writes state.
        let delta = position.0 - body.last_position;
        let measured = delta.length() / dt;
        body.speed = body.speed.lerp(measured, (dt * 6.0).min(1.0));
        body.last_position = position.0;
        body.phase += dt * (2.0 + body.speed * 1.6);

        transform.translation = position.0;
        if delta.length_squared() > 1e-6 {
            let facing = Quat::from_rotation_arc(Vec3::NEG_Z, delta.normalize());
            transform.rotation = transform.rotation.slerp(facing, (dt * 5.0).min(1.0));
        }

        // Dead organisms sink and fade instead of vanishing.
        let alive = !progress.dead;
        if !alive {
            transform.translation.y -= dt * 0.6;
        }

        // A bite arrived: the counter is a change signal, not a flag.
        if progress.bites != body.bites {
            let jumped = progress.bites != 0 && body.bites != 0;
            body.bites = progress.bites;
            if jumped {
                body.eat_timer = 0.4;
                let mouth = transform.translation + transform.rotation * Vec3::new(0.0, 0.0, -body.radius);
                spawn_burst(&mut commands, &fx, fx.bite.clone(), mouth, 7, 2.4, 0.12);
                spawn_burst(&mut commands, &fx, fx.algae.clone(), mouth, 7, 1.6, 0.09);
                // Digestion: a few grains dissolve inside the cell itself.
                spawn_burst(
                    &mut commands, &fx, fx.algae.clone(), transform.translation, 4, 0.5, 0.10,
                );
            }
        }
        body.eat_timer = (body.eat_timer - dt).max(0.0);

        // Taking damage: a red flash and a spray of torn cytoplasm.
        if progress.hits != body.hits {
            let jumped = body.hits != 0;
            body.hits = progress.hits;
            if jumped {
                body.hurt_timer = 0.35;
                spawn_burst(&mut commands, &fx, fx.hurt.clone(), transform.translation, 6, 2.6, 0.10);
            }
        }
        body.hurt_timer = (body.hurt_timer - dt).max(0.0);

        // Division: the cell pinches and throws off a burst.
        if progress.divisions != body.divisions {
            let jumped = body.divisions != 0;
            body.divisions = progress.divisions;
            if jumped {
                body.split_timer = 0.8;
                spawn_burst(&mut commands, &fx, fx.bite.clone(), transform.translation, 14, 1.8, 0.13);
            }
        }
        body.split_timer = (body.split_timer - dt).max(0.0);

        let radius = body_radius(vitals.mass);
        body.radius = radius;
        // Без своей энергии считаем тело сытым: у чужой клетки дыхание — деталь,
        // а не информация, и врать ею о чужом состоянии не нужно.
        let starving = energy
            .map(|e| (e.energy / e.cap.max(1.0)).clamp(0.0, 1.0))
            .unwrap_or(1.0);
        // Breathing is slow when calm, shallow and fast when starving.
        let breathe = (body.phase * 1.1).sin() * (0.08 + 0.05 * starving);
        // Squash and stretch along the direction of travel.
        let stretch = 1.0 + (body.speed * 0.075).min(0.55) + body.eat_timer * 0.35;
        let squeeze = 1.0 / stretch.sqrt();
        // A neighbour pressing on us flattens that side and bulges the others.
        let dent = body.dent_depth;
        let squeeze_axis = body.dent.abs().normalize_or(Vec3::X);
        // Вдавленная ось сплющивается сильно, остальные заметно раздуваются:
        // объём как будто перетекает в стороны.
        let dent_scale =
            Vec3::ONE - squeeze_axis * dent * 1.05 + Vec3::splat(dent * 0.42);

        // While splitting the cell pinches in the middle and swells along its axis.
        let split = body.split_timer;
        let scale = Vec3::new(
            radius * squeeze * (1.0 + breathe) * (1.0 - split * 0.25),
            radius * squeeze * (1.0 - breathe * 0.6) * (1.0 - split * 0.25),
            radius * stretch * (1.0 + split * 0.35),
        ) * dent_scale
            * if alive { 1.0 } else { 0.9 };

        let Ok(body_children) = children.get(entity) else { continue; };
        for child in body_children.iter() {
            if let Ok(mut membrane) = membranes.get_mut(child) {
                // Мембрана догоняет цель с запаздыванием — отсюда ощущение желе.
                membrane.scale = membrane.scale.lerp(scale, (dt * 13.0).min(1.0));
            }
            let Ok(parts) = children.get(child) else { continue; };
            for part_root in parts.iter() {
                if let Ok((parasite, mut inner)) = parasites.get_mut(part_root) {
                    // Паразит медленно ползает внутри и извивается.
                    let t = body.phase * 0.5 + parasite.phase;
                    inner.translation = Vec3::new(
                        t.sin(),
                        (t * 0.43).sin() * 0.5,
                        t.cos(),
                    )
                    .normalize_or(Vec3::Z)
                        * PARASITE_REACH;
                    // Смотрит по ходу движения вдоль стенки, а не в случайную
                    // сторону: так он читается как ползущее существо.
                    inner.rotation = Quat::from_rotation_y(-t)
                        * Quat::from_rotation_x((t * 1.7).sin() * 0.25);
                    continue;
                }
                if let Ok((organelle, mut inner)) = organelles.get_mut(part_root) {
                    let t = body.phase * organelle.speed;
                    inner.translation = drift(organelle, t);
                    continue;
                }
                let Ok(sub) = children.get(part_root) else { continue; };
                for leaf in sub.iter() {
                    if let Ok((segment, mut tail)) = tails.get_mut(leaf) {
                        // Travelling sine wave: amplitude grows down the whip and
                        // with speed, so a still organism has a limp tail.
                        let i = segment.index as f32;
                        let amp = (0.05 + body.speed * 0.03) * (i + 1.0) * 0.22;
                        let wave = (body.phase * 2.4 - i * 0.9).sin() * amp;
                        tail.translation.x = wave;
                        tail.translation.y = segment.spacing * (i + 1.0);
                        tail.rotation = Quat::from_rotation_z(-wave * 1.6);
                    } else if let Ok((hair, mut hair_transform)) = hairs.get_mut(leaf) {
                        let beat = (body.phase * 3.2 + hair.phase * 2.0).sin();
                        hair_transform.rotation =
                            hair.base_rotation * Quat::from_rotation_x(beat * 0.5);
                    } else if let Ok((organelle, mut inner)) = organelles.get_mut(leaf) {
                        // Slow, uneven drift inside the cytoplasm.
                        let t = body.phase * organelle.speed;
                        inner.translation = drift(organelle, t);
                    } else if let Ok(mut mouth) = mouths.get_mut(leaf) {
                        // Opens on a bite, idles with a slow chew.
                        let open = 1.0 + body.eat_timer * 1.6 + (body.phase * 0.8).sin() * 0.06;
                        mouth.scale = Vec3::new(open, 1.0, open);
                    }
                }
            }
        }
    }
}

/// Медленный неровный дрейф органеллы — но не дальше [`CYTOPLASM_REACH`].
///
/// Предел обязателен: орбита и дрейф складывались почти до самой оболочки, и
/// внутренности вылезали наружу.
fn drift(organelle: &Organelle, t: f32) -> Vec3 {
    let wander = organelle.orbit
        + Vec3::new(
            (t + organelle.phase).sin() * 0.16,
            (t * 0.8 + organelle.phase).cos() * 0.12,
            (t * 1.2 + organelle.phase).sin() * 0.16,
        );
    wander.clamp_length_max(CYTOPLASM_REACH)
}

/// Keeps health bars facing the camera and sized to the health they show.
pub fn update_health_bars(
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera: Query<&GlobalTransform, With<crate::MainCamera>>,
    organisms: Query<(&PlayerVitals, &Body, &Children)>,
    mut bars: Query<(&mut Transform, &Children), With<HealthBar>>,
    mut fills: Query<(&mut Transform, &MeshMaterial3d<StandardMaterial>), (With<HealthFill>, Without<HealthBar>)>,
) {
    let Ok(camera) = camera.single() else { return; };
    let camera_position = camera.translation();

    for (vitals, body, children) in &organisms {
        let fraction = (vitals.health / MAX_HEALTH).clamp(0.0, 1.0);
        for child in children.iter() {
            let Ok((mut bar_transform, bar_children)) = bars.get_mut(child) else { continue; };
            bar_transform.translation.y = body.radius + 0.95;
            // Billboard: the bar is drawn in world space, so it has to turn to
            // face the camera itself.
            let to_camera = camera_position - bar_transform.translation;
            bar_transform.rotation = Quat::from_rotation_y(to_camera.x.atan2(to_camera.z));

            for leaf in bar_children.iter() {
                let Ok((mut fill, material)) = fills.get_mut(leaf) else { continue; };
                fill.scale.x = 1.78 * fraction.max(0.0001);
                // The bar shrinks from the left, like a drained tank.
                fill.translation.x = -(1.78 - fill.scale.x) * 0.5;
                if let Some(mut material) = materials.get_mut(material.id()) {
                    material.base_color = Color::srgb(
                        1.0 - fraction * 0.6,
                        0.25 + fraction * 0.65,
                        0.30,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color::srgb(0.92, 0.38, 0.32);
    const GREEN: Color = Color::srgb(0.55, 0.85, 0.55);
    const SAND: Color = Color::srgb(0.86, 0.72, 0.40);

    fn grown(lineage: u64, extra: usize) -> Genome {
        let mut genome = Genome::starter_of(lineage);
        for _ in 0..extra {
            genome.push_part(PartKind::basic(PartFamily::Spike));
        }
        genome
    }

    /// Внутренности обязаны оставаться внутри при любой фазе дрейфа.
    ///
    /// Органеллы — дети мембраны, то есть живут в единичной сфере: всё, что
    /// дальше единицы, торчит наружу сквозь оболочку.
    #[test]
    fn organelles_never_poke_through_the_membrane() {
        for i in 0..6 {
            let a = i as f32 * 1.7;
            let organelle = Organelle {
                orbit: Vec3::new(a.sin() * 0.45, (a * 0.7).cos() * 0.35, a.cos() * 0.45),
                speed: 0.25 + (i as f32 % 3.0) * 0.12,
                phase: a,
            };
            // Крупнейший шарик цитоплазмы; он тоже должен помещаться.
            let ball = 0.10 + 2.0 * 0.05;
            for step in 0..400 {
                let t = step as f32 * 0.17;
                let far = drift(&organelle, t).length() + ball;
                assert!(far < 1.0, "органелла вылезла наружу: {far}");
            }
        }
    }

    /// Внешние органы сидят на поверхности, внутренние — внутри, и это доли
    /// радиуса, а не расстояния: тело растёт, органы растут вместе с ним.
    #[test]
    fn parts_sit_at_fractions_of_the_body_not_at_baked_distances() {
        for family in PartFamily::ALL {
            let depth = slot_depth(family);
            let place = slot_facing(family, 3) * depth;
            assert!((place.length() - depth).abs() < 1e-4, "{} сместился", family.name());
            if family.is_external() {
                assert!(depth > 0.7, "{} должен быть на виду", family.name());
            } else {
                assert!(depth < 0.6, "{} должен быть внутри", family.name());
            }
            // Паразит ползает глубже внешних органов и дальше цитоплазмы —
            // иначе он снова потеряется среди органелл.
            assert!(PARASITE_REACH > CYTOPLASM_REACH);
        }
    }

    /// Цвет обязан отвечать на вопрос «ударит ли оно меня», а не «одной ли мы
    /// фамилии».
    ///
    /// Здесь жил баг: родство проверялось раньше вражды, поэтому разошедшаяся
    /// ветвь рода красилась зелёным «своя семья», продолжая наносить урон.
    /// Ветка с Мутатором расходится быстрее прочих и упиралась в это первой.
    #[test]
    fn a_split_branch_is_red_even_though_it_is_kin() {
        let water = WorldUpdate::default();
        let mine = grown(7, 0);

        // Родня, разошедшаяся дальше порога раскола, — уже враг.
        let split = grown(7, (water.kin_split_threshold + 1) as usize);
        assert!(
            hostile_with(&mine, &split, water.aggression_threshold, water.kin_split_threshold),
            "предпосылка теста неверна: сервер такую пару врагами не считает"
        );
        assert_eq!(
            relation_color(Some(&mine), &split, false, &water),
            RED,
            "разошедшаяся ветвь рода покрашена как безопасная"
        );
    }

    /// Близкая родня остаётся зелёной, иначе лечение хуже болезни.
    #[test]
    fn close_kin_stay_green() {
        let water = WorldUpdate::default();
        let mine = grown(7, 0);
        let child = grown(7, 2);
        assert_eq!(relation_color(Some(&mine), &child, false, &water), GREEN);
    }

    /// Чужак, слишком похожий для драки, — песочный; разошедшийся — красный.
    #[test]
    fn strangers_are_coloured_by_whether_they_will_fight() {
        let water = WorldUpdate::default();
        let mine = grown(7, 0);

        let similar = grown(9, 1);
        assert_eq!(relation_color(Some(&mine), &similar, false, &water), SAND);

        let different = grown(9, (water.aggression_threshold + 1) as usize);
        assert_eq!(relation_color(Some(&mine), &different, false, &water), RED);
    }

    /// Пороги приходят от сервера: с другими настройками цвет обязан меняться,
    /// иначе интерфейс снова начнёт врать про чужой конфиг.
    #[test]
    fn colours_follow_the_servers_thresholds() {
        let mine = grown(7, 0);
        let stranger = grown(9, 5);

        let lenient = WorldUpdate { aggression_threshold: 20, ..Default::default() };
        assert_eq!(relation_color(Some(&mine), &stranger, false, &lenient), SAND);

        let harsh = WorldUpdate { aggression_threshold: 2, ..Default::default() };
        assert_eq!(relation_color(Some(&mine), &stranger, false, &harsh), RED);
    }
}
