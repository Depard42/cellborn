# Cellborn

Multiplayer cell-stage prototype inspired by the *design space* of cell-evolution games and mass-growth PvP games.

## Current slice

- Rust workspace.
- Bevy 0.19.
- Lightyear 0.29 from crates.io, using its server-authoritative client/server architecture, prediction and interpolation.
- Native UDP transport with netcode authentication, wired up in the client/server binaries themselves.
- Mutation-ready genome/body-part model.
- Environmental adaptation model.
- Seasonal environment: Bloom → Hot → Storm → Cold.
- Temperature, salinity, oxygen and toxin pressure.
- Energy/health survival loop.
- The first networked movement slice, with client prediction and the server sharing one movement function.
- Modular 3D body built from the `Genome`, procedurally animated (squash-and-stretch,
  flagellum beat, cilia, mouth), with eating particles.
- Server-authoritative food, feeding, growth, death, corpses and respawn.
- Mutation economy: points from eaten energy and survived seasons, a validated
  client → server request, and an in-game mutation panel.
- Season-driven world: fog, light, palette, seabed, kelp, marine snow, arena curtain.
- Combat between organisms that have drifted more than 7 parts apart; kin never fight.
- Cell division with inherited genomes and random mutation; a dead player takes over
  its own offspring.
- Bots: wild organisms that mutate and hunt on their own, plus colonies of kin.
- Spiteful mutations: a toxin gland poisons the water everyone swims through.
- An F1 debug overlay: client frame rate, what the client can actually see, and
  the server's own tick time, population and food count sent alongside it.
- Version stamped into the binary, and an in-game updater: the main menu checks
  GitHub releases, downloads the build for your platform and installs it over
  itself. The server config is never touched — new settings are appended to the
  player's own file instead.
- Linux and Windows builds are made by GitHub Actions and published as releases.
- See `docs/MECHANICS.md` for how it all works, `docs/TESTING.md` for what to
  check by hand, `docs/PERFORMANCE.md` for what the server costs and why, and
  `docs/RELEASING.md` for how a version gets built and shipped.

## Run

Start the server (listens on UDP `0.0.0.0:5555`):

```bash
cargo run -p cellborn-server
```

And clients in separate terminals:

```bash
cargo run -p cellborn-client
```

Each client connects to `127.0.0.1:5555` and picks a random client id, so several can
run on one machine. To reach another host, pass the address as the first argument:

```bash
cargo run -p cellborn-client -- 192.168.1.10:5555
```

Or use the launch script, which starts the server and the client together and
shuts the server down with the game:

```bash
./scripts/run.sh             # server + client
./scripts/run.sh server      # server only
./scripts/run.sh client 192.168.1.10:5555
```

## Releases

Tag a version and GitHub Actions builds both platforms and publishes the release:

```bash
git tag v0.2.0 && git push origin v0.2.0     # version must match Cargo.toml
```

Players update from inside the game — see `docs/RELEASING.md`.

## Windows build, by hand

Only needed to test a build locally; releases come from CI. Cross-compiled from
Linux with [`cross`](https://github.com/cross-rs/cross) (it runs the build in
Docker, so the daemon has to be up):

```bash
cargo install cross --git https://github.com/cross-rs/cross
./scripts/build-windows.sh
```

The result lands in `dist/windows`: `cellborn-server.exe`, `cellborn-client.exe` and
the `.bat` launchers. Copy the folder to a Windows machine and run `ИГРАТЬ.bat` — no
assets to ship, the font is compiled into the binary and everything else is
procedural.

Swim with WASD or the arrow keys; F1 opens the debug overlay in game. Shared
network constants (port, protocol id, key, tick rate) live in
`crates/common/src/network.rs` — client and server must agree on them or the
netcode handshake fails.

## Design direction

The cell is not just a circle with a mass value.

The core progression is:

`environment → pressure → mutation → body adaptation → survival → growth → reproduction/evolution`

### Planned mutations

- flagellum / cilia: movement
- mouth: feeding
- eye / chemical sensor: detection
- spike: contact defense
- toxin gland: offensive adaptation
- osmoregulator: salinity adaptation
- thermal membrane: temperature adaptation
- photosynthesis: low-cost energy source during bloom
- storage vacuole: reserve energy
- mucus coat: environmental protection

### Seasonal design

Seasons should not be cosmetic. Each season changes environmental pressures.

**Bloom**
- high food
- high oxygen
- low toxins

**Hot**
- lower oxygen
- higher temperature
- increasing salinity

**Storm**
- low oxygen
- high toxin pressure
- poor food availability

**Cold**
- low temperature
- moderate food
- different resource distribution

The long-term goal is that a player can be strong against another organism and still die because its body is badly adapted to the current environment.

## Next implementation slices

`docs/ROADMAP.md` (in Russian) breaks these into milestones with concrete work items;
`docs/OPEN-QUESTIONS.md` holds the design decisions they depend on.

Milestones 1-4 of `docs/ROADMAP.md` are implemented. What is left:

1. Interest management (rooms), tests and CI — roadmap milestone 5. Metrics are
   in: see the F1 overlay and `docs/PERFORMANCE.md`.
2. Fused procedural body and membrane shader — roadmap milestone 3, stages B and C.
3. Reproduction and inherited mutation.
4. Biomes and local environmental gradients.
5. AI organisms using the same genome/body system.
6. Persistence, real connect-token auth and matchmaking — roadmap milestone 6.
