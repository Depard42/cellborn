//! Lightweight particle system.
//!
//! Deliberately not a general engine: a handful of shared meshes and materials,
//! entities that shrink and die. Enough for bites, spores and drifting silt, and
//! cheap enough to run thousands of them on integrated graphics.

use bevy::prelude::*;
use rand::Rng;

#[derive(Resource)]
pub struct FxAssets {
    pub blob: Handle<Mesh>,
    pub bite: Handle<StandardMaterial>,
    pub algae: Handle<StandardMaterial>,
    pub hurt: Handle<StandardMaterial>,
}

impl FxAssets {
    pub fn load(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        let emissive = |c: Color, strength: f32| StandardMaterial {
            base_color: c.with_alpha(0.85),
            emissive: (LinearRgba::from(c) * strength),
            alpha_mode: AlphaMode::Blend,
            unlit: false,
            perceptual_roughness: 0.4,
            ..default()
        };
        Self {
            blob: meshes.add(Sphere::new(1.0).mesh().uv(8, 5)),
            bite: materials.add(emissive(Color::srgb(0.55, 0.95, 0.75), 2.5)),
            algae: materials.add(emissive(Color::srgb(0.45, 0.85, 0.35), 2.0)),
            hurt: materials.add(emissive(Color::srgb(0.95, 0.35, 0.30), 3.0)),
        }
    }
}

#[derive(Component)]
pub struct Particle {
    pub velocity: Vec3,
    pub life: f32,
    pub max_life: f32,
    pub size: f32,
    /// How fast the particle is slowed by the water.
    pub drag: f32,
}

/// A burst of particles, used for bites and for spores.
pub fn spawn_burst(
    commands: &mut Commands,
    fx: &FxAssets,
    material: Handle<StandardMaterial>,
    origin: Vec3,
    count: usize,
    speed: f32,
    size: f32,
) {
    let mut rng = rand::rng();
    for _ in 0..count {
        let dir = Vec3::new(
            rng.random_range(-1.0..1.0),
            rng.random_range(-0.4..1.0),
            rng.random_range(-1.0..1.0),
        )
        .normalize_or(Vec3::Y);
        let life = rng.random_range(0.35..0.8);
        let scale = size * rng.random_range(0.6..1.4);
        commands.spawn((
            Particle {
                velocity: dir * speed * rng.random_range(0.5..1.3),
                life,
                max_life: life,
                size: scale,
                drag: 3.0,
            },
            Transform::from_translation(origin).with_scale(Vec3::splat(scale)),
            Mesh3d(fx.blob.clone()),
            MeshMaterial3d(material.clone()),
            NotShadowCaster,
        ));
    }
}

/// Marker so particles do not pay for shadow rendering.
#[derive(Component)]
pub struct NotShadowCaster;

pub fn update_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut particles: Query<(Entity, &mut Particle, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut particle, mut transform) in &mut particles {
        particle.life -= dt;
        if particle.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let drag = particle.drag;
        particle.velocity *= 1.0 - (drag * dt).min(0.9);
        // A slight upward drift reads as buoyancy in water.
        particle.velocity.y += 0.35 * dt;
        let velocity = particle.velocity;
        transform.translation += velocity * dt;
        let fade = particle.life / particle.max_life;
        transform.scale = Vec3::splat(particle.size * fade.powf(0.6));
    }
}
