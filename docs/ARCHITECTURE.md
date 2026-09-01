# Architecture

## Workspace

```text
cellborn/
├── crates/
│   ├── common/
│   │   ├── organism.rs
│   │   ├── balance.rs
│   │   ├── environment.rs
│   │   ├── food.rs
│   │   ├── network.rs
│   │   ├── simulation.rs
│   │   └── protocol.rs
│   ├── client/
│   │   ├── body.rs
│   │   ├── world.rs
│   │   ├── fx.rs
│   │   └── ui.rs
│   └── server/
│       ├── life.rs
│       └── ai.rs
├── assets/
├── docs/
├── scripts/          # запуск и кросс-сборка под Windows
└── dist/windows/     # готовый Windows-дистрибутив (не в репозитории)
```

## Authority

The server owns:

- position
- energy
- health
- mass
- environment
- collisions
- feeding
- damage
- mutation validation
- reproduction

The client owns:

- input
- rendering
- camera
- animation
- UI
- prediction/interpolation presentation

## No custom networking

Lightyear is responsible for:

- transport
- replication
- input buffering
- prediction
- rollback
- interpolation

The project deliberately does not implement its own packet format or replication layer.

## No custom physics yet

The current slice does not require a physics engine. When physical contacts are introduced, use Avian3d + the Lightyear Avian integration instead of implementing collision detection manually.

## Mutation philosophy

A mutation must have a cost and a context.

Examples:

- `ThermalMembrane` improves hot/cold survival but adds mass.
- `Osmoregulator` improves salinity resistance but increases energy upkeep.
- `ToxinGland` improves PvP pressure but increases metabolic cost.
- `Photosynthesis` improves survival during Bloom but is weak in low-light/deep biomes.
- `MucusCoat` protects against environmental toxins but reduces movement.

This keeps adaptation from becoming a flat upgrade tree.

## Seasons

The environment is a server resource and therefore cannot be spoofed by clients.

A future version should make seasons affect resource spawning, biome visibility, migration, reproduction windows and mutation availability.
