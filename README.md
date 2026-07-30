# My Ecosystem Bevy

A WIP Bevy 2D game template with ecosystem plugins ported from [my-ecosystem-template](https://github.com/mlm-games/my-ecosystem-template) (Godot).

## Features

- **Game Feel** - recoil, knockback, slow-motion, rumble (gamepad)
- **Screen Effects** - trauma shake, freeze frame, flash white, chromatic aberration pulse + decay
- **Transitions** - fade to black, circle wipe scene transitions with input blocking
- **Audio** - channel-based SFX/Music/UI buses with independent volume control via `AudioSink`, pitch variation (uses Bevy built-in audio, no external dep)
- **Localization** - Fluent-based i18n with 7 bundled locales (en, es, fr, de, ja, zh, pt), language switcher in settings, `LocaleResources` resource
- **Save System** - persistent RON save + backup via `directories`
- **Object Pooling** - generic entity pool with acquire/release
- **Juice** - pop-in, squash & stretch, bounce scale, shake, particles with gravity/fade
- **VFX** - damage numbers, particle bursts, trail emitters
- **UI Effects** - hover scale, typewriter text, number counter
- **Math Utils** - smooth_damp, approach, wave (f32, Vec2, Vec3)
- **Center Pivot** - sprite origin centering component
- **UI** - animated buttons, popup system, pause/settings/credits with localized text (Repose)
- **States** - Splash -> Loading -> Title -> InGame with pause overlay
- **Theme** - centralized color constants
- **Dev Tools** - FPS overlay, state logging (dev feature)
- **Demo Scene** - player with shooting, enemies, trauma, recoil, burst effects, damage numbers, gamepad rumble

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
├── ecosystem/           # Game feel, transitions, audio, save, i18n, vfx, etc.
│   ├── audio.rs         # Channel-based audio buses (SfxChannel/MusicChannel/UiChannel)
│   ├── center_pivot.rs  # Sprite origin centering
│   ├── game_feel.rs     # Recoil, knockback, slow-motion, gamepad rumble
│   ├── i18n.rs          # Fluent-based localization (7 locales, language switcher)
│   ├── juice.rs         # Pop-in, squash/stretch, bounce, shake, particles
│   ├── math_utils.rs    # smooth_damp, approach, wave (f32/Vec2/Vec3)
│   ├── pooling.rs       # Generic entity pooling
│   ├── save.rs          # RON save/load with backup
│   ├── screen_effects.rs# Trauma, freeze frame, flash white, chromatic aberration
│   ├── transitions.rs   # Fade/circle wipe with input blocking
│   ├── ui_effects.rs    # Hover scale, typewriter, number counter
│   └── vfx.rs           # Damage numbers, particle bursts, trail emitters
├── screens/             # Splash, loading, title
├── menus/               # Main, pause, settings, credits (localized)
├── theme/               # Theme resource
├── demo/                # Sample gameplay with all juice
├── dev_tools.rs         # FPS overlay, state logging
└── asset_tracking.rs    # Preload tracking
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `bevy` (git rev) | Engine |
| `repose-bevy` / `repose-*` | UI framework |
| `fluent-bundle` + `unic-langid` | Localization (Fluent) |
| `serde` + `ron` + `directories` | Save system |
| `rand` | Random variation (audio pitch, VFX) |
| `avian2d` (optional) | Physics |

## License

GPL-3.0
