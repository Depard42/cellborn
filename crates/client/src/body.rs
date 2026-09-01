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
pub fn relation_color(mine: Option<&Genome>, theirs: &Genome, controlled: bool) -> Color {
    if controlled {
        return Color::srgb(0.46, 0.88, 0.74);
    }
    match mine {
        // Same line: family, never hostile.
        Some(m) if m.lineage == theirs.lineage => Color::srgb(0.55, 0.85, 0.55),
        // Different enough to be prey — or predator.
        Some(m) if hostile(m, theirs) => Color::srgb(0.92, 0.38, 0.32),
        // A stranger, but too similar to fight.
        _ => Color::srgb(0.86, 0.72, 0.40),
    }
}

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
        (Entity, &PlayerGenome, &PlayerVitals, &PlayerPosition, Has<Controlled>, Option<&Body>),
        Or<(Without<Body>, Changed<PlayerGenome>)>,
    >,
    mine: Query<&PlayerGenome, With<Controlled>>,
    children: Query<&Children>,
) {
    for (entity, genome, vitals, position, controlled, body) in &organisms {
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
        let tint = relation_color(mine.iter().next().map(|g| &g.0), &genome.0, controlled);

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

        for part in genome.0.parts.iter() {
            if part.kind.family == PartFamily::Membrane {
                continue;
            }
            // Positions are stored in the genome in body-radius units; the membrane
            // child is already scaled by the radius, so they are used as-is here.
            let base = part.position / radius.max(0.001);
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

/// Keeps the membrane colour honest about kinship: family, stranger, or enemy.
pub fn recolor_bodies(
    mut materials: ResMut<Assets<StandardMaterial>>,
    mine: Query<&PlayerGenome, With<Controlled>>,
    bodies: Query<(&Body, &PlayerGenome, Has<Controlled>)>,
) {
    let mine = mine.iter().next().map(|g| &g.0);
    for (body, genome, controlled) in &bodies {
        let tint = relation_color(mine, &genome.0, controlled);
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

    for (entity, mut body, position, vitals, progress, mut transform) in &mut organisms {
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
        let starving = (vitals.energy / vitals.energy_cap.max(1.0)).clamp(0.0, 1.0);
        // Breathing is slow when calm, shallow and fast when starving.
        let breathe = (body.phase * 1.1).sin() * (0.05 + 0.03 * starving);
        // Squash and stretch along the direction of travel.
        let stretch = 1.0 + (body.speed * 0.045).min(0.35) + body.eat_timer * 0.25;
        let squeeze = 1.0 / stretch.sqrt();
        // While splitting the cell pinches in the middle and swells along its axis.
        let split = body.split_timer;
        let scale = Vec3::new(
            radius * squeeze * (1.0 + breathe) * (1.0 - split * 0.25),
            radius * squeeze * (1.0 - breathe * 0.6) * (1.0 - split * 0.25),
            radius * stretch * (1.0 + split * 0.35),
        ) * if alive { 1.0 } else { 0.9 };

        let Ok(body_children) = children.get(entity) else { continue; };
        for child in body_children.iter() {
            if let Ok(mut membrane) = membranes.get_mut(child) {
                membrane.scale = membrane.scale.lerp(scale, (dt * 10.0).min(1.0));
            }
            let Ok(parts) = children.get(child) else { continue; };
            for part_root in parts.iter() {
                if let Ok((organelle, mut inner)) = organelles.get_mut(part_root) {
                    let t = body.phase * organelle.speed;
                    inner.translation = organelle.orbit
                        + Vec3::new(
                            (t + organelle.phase).sin() * 0.16,
                            (t * 0.8 + organelle.phase).cos() * 0.12,
                            (t * 1.2 + organelle.phase).sin() * 0.16,
                        );
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
                        inner.translation = organelle.orbit
                            + Vec3::new(
                                (t + organelle.phase).sin() * 0.16,
                                (t * 0.8 + organelle.phase).cos() * 0.12,
                                (t * 1.2 + organelle.phase).sin() * 0.16,
                            );
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

/// Keeps health bars facing the camera and sized to the health they show.
pub fn update_health_bars(
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera: Query<&GlobalTransform, With<Camera3d>>,
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
