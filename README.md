# Opensus (Bevy)

OpenSuspect-inspired social deduction game. Migrated from Godot [Opensus](https://github.com/mlm-games/Opensus) onto [my-ecosystem-template-bevy](https://github.com/mlm-games/my-ecosystem-template-bevy).

## Run

```bash
cargo run
cargo run --features dev
cargo run --features physics   # when you add colliders
```

## Current

- Title → Host / Join → Lobby (local + bots + network peers) → Ready → Start
- Roles (crew / impostor) with role reveal, impostor/crewmate vision (radial FOW, Lights dim)
- Map with collision (SolidAabb), rooms, emergency button, tasks + sabotage stations (O₂/Reactor/Lights)
- Movement, tasks (hold E), kill (Q w/ cooldown), report (R), emergency (F at button), sabotage (1/2/3)
- Meeting → voting (bot voting restricted to AiPlayer, skip/tie handling) → eject → win checks (tasks / deaths / sabotage)
- Meeting chat (server-authoritative echo, length-capped, ghost-tagged, Meeting|Voting only, log cap 50)
- Renet2 native networking (reliable action/lobby/chat + unreliable input/snapshot, sequence validation, handshake auth, per-client private state, ordered transport on `Time<Real>`)
- Full ecosystem juice (trauma, VFX, transitions, i18n, save, Repose UI)

## Networking details

- Server-authoritative (Host owns simulation, clients send intent only)
- Snapshot/input sequencing with wrapping comparison to drop stale UDP
- Transport runs on `Time<Real>` and ordered PreUpdate/PostUpdate sets

## Next

- Audio cues (kill sting, meeting alarm, task chime, sabotage klaxon, win themes)
- Client interpolation (snapshot buffering + render-time lerp for replicas)
- Bot waypoint navigation (room/doorway graph + A*)
- Ghost-only chat filtering + per-client rate limits, WebTransport, additional cosmetics / maps

## License

GPL-3.0
