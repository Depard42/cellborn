use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::Biome;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FoodKind {
    /// Drifting plankton: common, small, everywhere.
    Plankton,
    /// Algae clumps: rich, thrive in Bloom.
    Algae,
    /// Sunken remains: what corpses leave behind, and what Storm stirs up.
    Detritus,
}

impl FoodKind {
    pub fn energy(self) -> f32 {
        match self {
            FoodKind::Plankton => 9.0,
            FoodKind::Algae => 16.0,
            FoodKind::Detritus => 12.0,
        }
    }

    pub fn radius(self) -> f32 {
        match self {
            FoodKind::Plankton => 0.16,
            FoodKind::Algae => 0.26,
            FoodKind::Detritus => 0.22,
        }
    }

    /// Насколько вероятна эта пища в этом биоме.
    ///
    /// Не только количество еды отличает биомы, но и её **состав**: на отмели
    /// одна мелочь, в разломе почти сплошь водоросли, которые вдвое питательнее.
    /// Поэтому тяжёлый биом выгоден дважды — там и гуще, и сытнее.
    pub fn weight(self, biome: Biome) -> f32 {
        match (self, biome) {
            (FoodKind::Plankton, Biome::Open) => 2.0,
            (FoodKind::Plankton, Biome::Shallows) => 3.5,
            (FoodKind::Plankton, Biome::Brine) => 1.0,
            (FoodKind::Plankton, Biome::Vents) => 0.6,
            (FoodKind::Plankton, Biome::Abyss) => 1.4,

            (FoodKind::Algae, Biome::Open) => 1.2,
            (FoodKind::Algae, Biome::Shallows) => 2.0,
            (FoodKind::Algae, Biome::Brine) => 1.4,
            (FoodKind::Algae, Biome::Vents) => 3.2,
            (FoodKind::Algae, Biome::Abyss) => 0.4,

            (FoodKind::Detritus, Biome::Open) => 0.8,
            (FoodKind::Detritus, Biome::Shallows) => 0.5,
            (FoodKind::Detritus, Biome::Brine) => 2.6,
            (FoodKind::Detritus, Biome::Vents) => 1.6,
            (FoodKind::Detritus, Biome::Abyss) => 2.2,
        }
    }
}

/// A replicated edible particle. Food never moves, so it is neither predicted nor
/// interpolated — it is spawned, replicated once, and despawned when eaten.
#[derive(Component, Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Nutrient {
    pub kind: FoodKind,
    pub energy: f32,
}

#[derive(Component, Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct FoodPosition(pub Vec3);

/// A drifting patch of poisoned water, left behind by a cell with a toxin gland.
///
/// The spiteful mutation: it raises the toxin level for everyone swimming through
/// it, including the organism that made it — carrying the gland is what makes you
/// resistant enough to live in your own mess.
#[derive(Component, Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ToxinCloud {
    pub position: Vec3,
    pub radius: f32,
    pub strength: f32,
}

impl ToxinCloud {
    /// Extra toxin at a point, fading to nothing at the edge.
    pub fn toxin_at(&self, point: Vec3) -> f32 {
        let d = self.position.distance(point);
        if d >= self.radius {
            return 0.0;
        }
        self.strength * (1.0 - d / self.radius)
    }
}
