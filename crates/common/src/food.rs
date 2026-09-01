use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::Season;

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

    /// How likely this kind is to spawn in a given season. This is the wire that
    /// finally makes seasons observable: Bloom floods the map with algae, Storm
    /// leaves only detritus.
    pub fn weight(self, season: Season) -> f32 {
        match (self, season) {
            (FoodKind::Plankton, Season::Bloom) => 3.0,
            (FoodKind::Plankton, Season::Hot) => 2.0,
            (FoodKind::Plankton, Season::Storm) => 1.0,
            (FoodKind::Plankton, Season::Cold) => 2.0,
            (FoodKind::Algae, Season::Bloom) => 3.0,
            (FoodKind::Algae, Season::Hot) => 1.2,
            (FoodKind::Algae, Season::Storm) => 0.3,
            (FoodKind::Algae, Season::Cold) => 0.8,
            (FoodKind::Detritus, Season::Bloom) => 0.6,
            (FoodKind::Detritus, Season::Hot) => 1.0,
            (FoodKind::Detritus, Season::Storm) => 2.0,
            (FoodKind::Detritus, Season::Cold) => 1.2,
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
