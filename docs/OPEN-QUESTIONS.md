# Open questions

Design decisions that the current code either leaves undecided or answers in a way
that contradicts `docs/ARCHITECTURE.md` / `README.md`. Each one blocks a concrete
next slice, so they are written as decisions to make, not as bugs to file.

State this refers to: the slice where the first networked movement, replication,
prediction and the genome-driven body renderer work end to end. Numbers below were
computed from the constants actually in the code at 64 Hz (`FIXED_TIMESTEP_HZ`).

---

## 1. The survival economy is a one-way countdown

**What the code does.** `survival` (`crates/server/src/main.rs`) drains energy every
fixed tick: `energy -= 0.025 + penalty * 0.10`. When energy reaches 0, health drains
by `0.05` per tick. Nothing anywhere adds energy back except Photosynthesis during
Bloom. There are no food entities, so for almost every genome energy only goes down.

With a default genome, 100 energy and 100 health:

| Season | Penalty | Energy drain | Time to 0 energy | Time to 0 health after that |
|--------|---------|--------------|------------------|------------------------------|
| Bloom  | 0.100   | 2.24 /s      | 44.6 s           | 31.2 s                       |
| Hot    | 0.350   | 3.84 /s      | 26.0 s           | 31.2 s                       |
| Storm  | 0.303   | 3.54 /s      | 28.3 s           | 31.2 s                       |
| Cold   | 0.265   | 3.30 /s      | 30.3 s           | 31.2 s                       |

So a player is out of energy in under 45 seconds and at zero health roughly a minute
after connecting — visible live in the HUD energy bar, which drains from full to empty
while you watch and never refills. A season lasts 180 s and a full cycle 720 s, meaning **no player
ever lives long enough to see a season change** — the entire seasonal system, which is
the heart of the design, is currently unobservable in play.

Two more consequences of the same constants:

- **Photosynthesis is a hard on/off switch.** In Bloom it adds `+5.12 /s` against a
  `2.24 /s` drain, so energy is pinned at the 100 cap and the organism is immortal.
  In every other season it does nothing at all. There is no middle ground.
- **The constants are per tick, not per second.** Changing `FIXED_TIMESTEP_HZ` from 64
  to 30 would roughly halve every drain rate and silently rebalance the whole game.
  Movement was converted to `speed * dt` for exactly this reason; the survival loop
  was deliberately left alone because changing it changes balance, which is your call.

**Decisions needed.**

1. What is the intended lifetime of an organism that never eats — 60 s, 5 minutes,
   one full season? That single number fixes the base drain constant.
2. Should the survival constants become per-second rates multiplied by `delta_secs()`,
   like movement now is? (Recommended: yes; otherwise tick-rate changes are balance
   changes.)
3. Should Photosynthesis scale with something (light level, depth, membrane area)
   instead of being a flat larger-than-drain constant?

Until food exists (`README.md`, slice 2), consider a low regeneration floor so the
seasonal system is at least observable while testing.

---

## 2. `adaptation_penalty` saturates, and its `0.20` constant is a hidden second tolerance

**What the code does** (`crates/common/src/simulation.rs`):

```rust
let temp_delta = (organism.temperature_tolerance - (env.temperature - 0.5).abs()).max(0.0);
// ...
+ (0.20 - temp_delta).max(0.0) * 0.60
```

`temp_delta` is not a delta — it is the organism's *remaining margin*, clamped at 0.
Expanding for the default tolerance of 0.25, the temperature term is
`(|Δtemp| - 0.05).max(0) * 0.6`, capped at `0.12`.

Three properties follow, and I do not know which of them are intended:

- **The penalty saturates at the tolerance limit.** Once the environment exceeds the
  organism's tolerance, exceeding it by 0.01 and by 0.5 cost exactly the same. There
  is no such thing as a lethal environment — only a mildly expensive one.
- **`0.20` acts as a reference tolerance.** An organism with tolerance below 0.20 pays
  a penalty even in a perfectly neutral environment; an organism above 0.20 gets a
  free zone of `tolerance - 0.20`. That threshold is invisible in the API and easy to
  break by tuning `BASE_TEMPERATURE_TOLERANCE`.
- **Adaptation buys very little.** Even after tolerances were made genome-derived, a
  ThermalMembrane only moves Hot from 26.0 s to 30.6 s of life (+18%) and Cold from
  30.3 s to 34.3 s (+13%). Osmoregulator and ToxinGland are worth under 3% each,
  because the flat `0.10` baseline plus the oxygen term dominate everything.

**The bigger gap: oxygen cannot be adapted to at all.** The oxygen term
`(0.8 - env.oxygen).max(0) * 0.45` never looks at the organism. In Storm it is **52%
of the total penalty**, in Hot 26%. Storm is the season the design describes as the
survival test, and it is precisely the one no body plan can answer.

**Decisions needed.**

1. Should the penalty keep growing past the tolerance limit (i.e. rewrite as
   `(|Δ| - tolerance).max(0) * k`), so that a badly adapted organism can actually be
   killed by the environment?
2. Which part answers low oxygen? Nothing in `PartKind` currently maps to it. A gill
   or the MucusCoat are the obvious candidates; adding one closes the Storm gap.
3. Should the flat `0.10` baseline scale with body size / number of parts, so that a
   large organism has a real upkeep instead of a constant one?

---

## 3. Every mutation is a pure upgrade — the promised trade-offs do not exist

`docs/ARCHITECTURE.md` states the rule plainly: *"A mutation must have a cost and a
context"*, and lists five examples. None of the costs are implemented:

| Part             | Documented cost              | Implemented cost |
|------------------|------------------------------|------------------|
| ThermalMembrane  | adds mass                    | none — and it *adds* +5% speed (see §5) |
| Osmoregulator    | increases energy upkeep      | none |
| ToxinGland       | increases metabolic cost     | none |
| Photosynthesis   | weak in low-light/deep biomes| none — biomes do not exist |
| MucusCoat        | reduces movement             | none — MucusCoat has no effect anywhere |

Right now adding any part is strictly beneficial, so the mutation system, once it is
reachable, degenerates into "take everything". This is the single largest gap between
the design documents and the code.

**Decisions needed.**

1. Where does upkeep live — a per-part energy cost folded into `adaptation_penalty`,
   or a separate `metabolic_cost(genome)` term added to the drain?
2. Does mass come from parts (`mass = base + Σ part_mass`), and does mass feed back
   into speed and upkeep? The project is described as a mass-growth PvP game, but
   `PlayerVitals::mass` is currently written once and never changes.
3. Is `BodyPart::level` meant to scale both a part's benefit and its cost? It is
   stored, replicated and never read.

---

## 4. The mutation economy is unreachable from the game

`OrganismState::apply_mutation` is complete and correct, and nothing can ever call it:

- `mutation_points` is never incremented anywhere.
- There is no client → server message to request a mutation. `ProtocolPlugin`
  registers replicated components only; the protocol has no messages or triggers at
  all.
- There is no UI.

**Decisions needed.**

1. How are mutation points earned — survival time, energy absorbed, surviving a season
   transition, kills? This determines whether points are a server-side counter on the
   organism or derived from an existing stat.
2. Message shape for the request. The natural fit for the current stack is a
   `MutationRequest { kind: PartKind }` client message, validated server-side against
   `mutation_cost` (the client must never be trusted with the point balance), with the
   resulting genome replicating back through `PlayerGenome`. Confirm before I add the
   first message type to the protocol, since it sets the pattern for all later ones.
3. Should a mutation be applicable at any moment, or only at a season boundary /
   reproduction event? The latter makes seasons matter strategically.

---

## 5. `movement_speed` gives ThermalMembrane a speed bonus

```rust
let thermal = organism.has_part(crate::PartKind::ThermalMembrane);
let base = 5.0 + flagella * 1.8 + cilia * 0.8;
if thermal { base } else { base * 0.95 }
```

A temperature-adaptation organelle making the cell 5% faster looks like a wrong
`PartKind` rather than a design choice — and the polarity is inverted from the usual
reading: it is expressed as a 5% *penalty for not having* a thermal membrane, which
every default organism pays.

Per `ARCHITECTURE.md`, the part that belongs in this expression is MucusCoat, as a
movement *penalty*. Should I swap it, drop the clause entirely, or is the bonus
intentional?

---

## 6. Death and respawn are unhandled

Health reaches 0 and nothing happens: the entity keeps existing, keeps being
replicated, keeps accepting input, and keeps moving. There is no death state, no
corpse, no respawn, no scoreboard event.

Disconnects are handled — `ControlledBy { lifetime: Lifetime::SessionBased }` makes
lightyear despawn the organism when its owner leaves — but that is the only way an
organism can ever leave the world.

**Decisions needed.**

1. On death: despawn and respawn a fresh organism, or keep the entity as a spectator
   and let the player re-enter at the next season boundary?
2. Does death cost genome progress (restart from the default three parts), keep the
   genome, or keep a fraction of the mutation points? This is the core progression
   decision for the whole prototype.
3. Should the corpse become food, once food exists?

---

## 7. Where does non-replicated organism state live?

`OrganismState` is a `Component` but is never stored on any entity. The server
reconstructs one from `PlayerGenome` + `PlayerVitals` on every tick and throws it
away. That works today only because everything in it is derivable from the genome —
which is exactly why the tolerance bug happened in the first place.

`age`, `mutation_points` and any future non-derivable field have nowhere to live and
would be silently reset every tick if added now.

**Decision needed.** Insert `OrganismState` as a server-only component on the player
entity as the single source of truth, and treat `PlayerVitals` / `PlayerGenome` purely
as the replicated projection of it? That is the option I would take, but it changes
who owns the state, so it should be a deliberate choice rather than a refactor I make
on the way past.

---

## 8. Is the world planar or volumetric?

Movement writes only `x` and `z`; `y` stays at whatever the entity spawned with, and
the arena clamp (`ARENA_HALF_EXTENT`) is 2D. The renderer, the follow camera and the
seabed grid all assume a plane, so this is currently a 2D game drawn in 3D — which is
fine as a default, but it should be a decision rather than an accident.

**Decisions needed.**

1. Is cell-stage gameplay a plane (like the games this is inspired by) or true 3D
   swimming with depth? Depth is what would make Photosynthesis "weak in deep biomes"
   meaningful, and it changes the camera, the arena bounds and the body renderer.
2. Should the arena be a soft boundary (a pressure gradient that damages) rather than
   a hard clamp? A hard clamp is the cheapest thing that keeps prediction consistent,
   but it reads as an invisible wall.
3. Should the organism rotate to face its swimming direction? Right now the body never
   turns, so the mouth and flagellum point in a fixed world direction regardless of
   movement — which makes the body plan look decorative rather than functional.

## 9. Per-player `PlayerEnvironment` — deliberate or accidental?

Every organism carries its own replicated copy of season, temperature, salinity and
oxygen, and the server writes identical values into all of them every tick. Today
that is pure duplicated bandwidth, scaling with player count.

It is the right shape only if local environmental gradients are coming (README slice
6, "biomes and local environmental gradients"), in which case each organism genuinely
samples a different environment. If biomes are further out than the next few slices,
a single replicated environment entity would be cheaper and simpler.

**Decision needed.** Keep it per-player as forward-compatibility for biomes, or
collapse to one global replicated environment now and split it again later?

---

## Suggested defaults

If you would rather not decide all of this up front, these are the answers I would
implement, in this order, and each is reversible:

1. Convert the survival constants to per-second rates and set the no-food lifetime to
   ~3 minutes, so a season change becomes observable (§1).
2. Rewrite `adaptation_penalty` to grow past the tolerance limit, and give one part an
   oxygen effect so Storm is answerable (§2).
3. Add a `metabolic_cost(genome)` term and mass-from-parts, which is what makes every
   mutation a trade-off (§3).
4. Add a follow camera (§8) — smallest change here, and it removes the biggest source
   of "is it broken or am I just off-screen?".
5. Only then the mutation message, the point economy and the UI (§4), because they are
   the slice that most depends on the answers above.
