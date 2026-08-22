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
- Map with collision (SolidAabb, shared `step_player_position` for host + prediction)
- Movement, tasks (hold E), kill (Q w/ cooldown), report (R), emergency (F at button), sabotage (1/2/3)
- Meeting → voting (bot voting restricted to AiPlayer, skip/tie handling) → eject → win checks (tasks / deaths / sabotage)
- Meeting chat (server-authoritative echo, length-capped, ghost-isolated: living→living only, ghost→all, per-client filtered delivery + `visible_to` display, log cap 50)
- Procedural `Pitch` audio soundscape (role-reveal, meeting/voting/results, crew/impostor win, body, task, vote, chat, sabotage/lights, critical alarm with urgency ramp)
- Renet2 native networking (reliable action/lobby/chat + batched `NetInputCommand` at 64 Hz, ordered transport on `Time<Real>`/`Time<Fixed>`, sequence validation, handshake auth + 5 s timeout, token-bucket rate limits, per-client private state with `acknowledged_input_sequence`)
- Client interpolation (100 ms delay, `SNAP_DISTANCE` snap guard, no extrapolation) + client prediction/reconciliation (redundant `INPUT_BATCH_SIZE` batches, `MAX_CLIENT/SERVER_PENDING`, replay via `step_player_position`, `RemoteNetworkPlayer` excluded from intent movement)
- Session lifecycle hardened (despawn abandoned `RemoteNetworkPlayer` on disconnect, reject late joins outside `Lobby`, deferred lobby slot to `Hello`, handshake timeout)
- Client replicas use tinted character art (`bake_body_tint` + `PlayerLayer` body/clothes) for visual parity
- Full ecosystem juice (trauma, VFX, transitions, i18n, save, Repose UI)

## Networking details

- Server-authoritative (Host owns simulation, clients send fixed-step commands)
- Snapshot/input sequencing with wrapping comparison to drop stale UDP
- Transport runs on `Time<Real>` and ordered `PreUpdate`/`PostUpdate` sets; prediction on `Time<Fixed>` @ 64 Hz

## Next

- WebTransport, bot waypoint navigation polish (room/doorway graph + A*), real SFX asset pass (swap `Pitch` for `AudioSource`), additional cosmetics / maps, spectator/rejoin

## License

GPL-3.0
