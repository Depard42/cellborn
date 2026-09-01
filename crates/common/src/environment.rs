use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Season {
    Bloom,
    Hot,
    Storm,
    Cold,
}

impl Season {
    pub fn next(self) -> Self {
        match self {
            Season::Bloom => Season::Hot,
            Season::Hot => Season::Storm,
            Season::Storm => Season::Cold,
            Season::Cold => Season::Bloom,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Season::Bloom => "Bloom",
            Season::Hot => "Hot",
            Season::Storm => "Storm",
            Season::Cold => "Cold",
        }
    }
}

#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub season: Season,
    pub time_in_season: f32,
    pub season_length: f32,
    pub temperature: f32,
    pub salinity: f32,
    pub oxygen: f32,
    pub toxin_level: f32,
    pub food_density: f32,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            season: Season::Bloom,
            time_in_season: 0.0,
            season_length: 180.0,
            temperature: 0.50,
            salinity: 0.50,
            oxygen: 0.80,
            toxin_level: 0.05,
            food_density: 1.0,
        }
    }
}

impl Environment {
    pub fn advance(&mut self, dt: f32) {
        self.time_in_season += dt;
        if self.time_in_season >= self.season_length {
            self.time_in_season = 0.0;
            self.season = self.season.next();
        }

        let phase = self.time_in_season / self.season_length;
        match self.season {
            Season::Bloom => {
                self.temperature = 0.45 + phase * 0.10;
                self.salinity = 0.45;
                self.oxygen = 0.90;
                self.toxin_level = 0.03;
                self.food_density = 1.40;
            }
            Season::Hot => {
                self.temperature = 0.75;
                self.salinity = 0.50 + phase * 0.15;
                self.oxygen = 0.60;
                self.toxin_level = 0.08;
                self.food_density = 0.80;
            }
            Season::Storm => {
                self.temperature = 0.55;
                self.salinity = 0.40;
                self.oxygen = 0.45;
                self.toxin_level = 0.15;
                self.food_density = 0.65;
            }
            Season::Cold => {
                self.temperature = 0.20;
                self.salinity = 0.55;
                self.oxygen = 0.70;
                self.toxin_level = 0.04;
                self.food_density = 0.90;
            }
        }
    }
}
