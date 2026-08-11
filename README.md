# Opensus (Bevy)

OpenSuspect-inspired social deduction game. Migrated from Godot [Opensus](https://github.com/mlm-games/Opensus) onto [my-ecosystem-template-bevy](https://github.com/mlm-games/my-ecosystem-template-bevy).

## Run

```bash
cargo run
cargo run --features dev
cargo run --features physics   # when you add colliders
```

## Current sandbox

- Title → Host → Lobby (local + bot agents) → Ready → Start
- Roles (crew / impostor), movement, tasks (hold E), kill (Q), report (R), emergency (F)
- Meeting → vote → eject → win checks (tasks / deaths)
- Full ecosystem juice (trauma, VFX, transitions, i18n, save, Repose UI)

## Next

1. Lightyear / bevy_replicon networking
2. Real map (LDtk) + vision
3. Sabotage timers
4. Chat
5. Character cosmetics

## License

GPL-3.0
