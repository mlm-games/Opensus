# My Ecosystem Bevy

A WIP Bevy 2D game template with ecosystem plugins ported from [my-ecosystem-template](https://github.com/mlm-games/my-ecosystem-template) (Godot).

## Features

- **Game Feel** - recoil, knockback, slow-motion, rumble
- **Screen Effects** - trauma shake, freeze frame, flash, chromatic aberration
- **Transitions** - fade to black, circle wipe scene transitions
- **Audio** - channel-based SFX/Music/UI with volume control
- **Save System** - persistent save via `bevy_pkv` + backup
- **Object Pooling** - generic entity pool with acquire/release
- **Juice** - pop-in, squash & stretch, shake animations
- **UI** - animated buttons, popup system, pause/settings/credits
- **States** - Splash -> Loading -> Title -> InGame with pause state
- **Theme** - centralized color/font constants
- **Dev Tools** - FPS overlay, state logging (dev feature)
- **Demo Scene** - player with shooting, enemies, trauma, recoil

## Quick Start

```bash
cargo run
```

With physics (Avian2d, will be switched to rapier soon):
```bash
cargo run --features physics
```

Dev build with hot-reload:
```bash
cargo run --features dev
```

## Structure

```
src/
├── main.rs              # Entry point
├── app.rs               # AppPlugin, states, system sets
├── ecosystem/           # Game feel, transitions, audio, save, etc.
├── screens/             # Splash, loading, title
├── menus/               # Main, pause, settings, credits
├── theme/               # Theme resource
├── demo/                # Sample gameplay
├── dev_tools.rs         # FPS overlay, state logging
└── asset_tracking.rs    # Preload tracking
```

## License

GPL-3.0
