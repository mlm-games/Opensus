<div align="center">

# 🎮 My Ecosystem Template

[![Godot](https://img.shields.io/badge/Godot-4.6-blue?logo=godot-engine)](https://godotengine.org/)
[![License](https://img.shields.io/badge/License-GPL%20v3.0-blue.svg)](LICENSE)
[![GitHub stars](https://img.shields.io/github/stars/mlm-games/my-ecosystem-template?style=social)](https://github.com/mlm-games/my-ecosystem-template/stargazers)

**A production-ready Godot 4 template packed with essential systems, utilities, and workflows to jumpstart your game development journey.**

[🚀 Quick Start](#quick-start) • [✨ Features](#features) • [📚 Documentation](#documentation) • [🤝 Contributing](#contributing)

<!-- DEMO GIF PLACEHOLDER - Add your gameplay recording here -->
<!-- <img src="docs/demo.gif" alt="Demo" width="800"> -->

</div>

---

## 🎯 What is this?

Stop reinventing the wheel. Start with a battle-tested foundation that handles the boring stuff—transitions, audio, saves, localization, build pipelines—so you can focus on making your game **fun**.

> "Try already successful stuff until you are satisfied (in the profession maintainability aspect) and then, can do something new"

---

## ✨ Features

### 🎬 **Cinematic Transitions**
- Smooth scene transitions (fade to black, circle wipe, flash)
- Async/await-ready for clean code
- Automatic input blocking during transitions

### 🎮 **Game Feel Arsenal**
- **Screen shake** with customizable intensity
- **Recoil** and **knockback** utilities
- **Slow-motion** effects
- **Controller rumble** support
- **Juice** effects for that extra polish

### 🎵 **Audio Management**
- Global audio manager with volume control
- UI audio feedback system
- Easy sound effect triggering

### 💾 **Save System**
- Built-in save/load manager
- Resource management utilities
- Undo/redo system for editor-like functionality

### 🛠️ **Developer Tools**
- Debug shortcut manager
- Screen effects (chromatic aberration, flash white, UI glow)
- Object pooling for performance
- Popup animator for polished UI

### 🌍 **Production Ready**
- **30+ language** localization support
- Multi-platform build configs (Snap, Flatpak, Chocolatey, AUR, Fastlane)
- Window management (fullscreen toggle, resizing)
- Pause menu with settings integration

### 🧩 **UI Components**
- Animated buttons with hover/click feedback
- Credits scene template
- Center pivot component for 2D nodes
- Settings menu integration

---

## 📊 Version Trackers

| Platform   | Version |
|:-----------|:--------|
| Flathub    | [![Flathub Version](https://img.shields.io/flathub/v/io.github.mlm_games.<placeholdername>)](https://flathub.org/apps/io.github.mlm_games.<placeholdername>) |
| Snap Store | [![Snapcraft Version](https://img.shields.io/snapcraft/v/<placeholdername>/latest/stable)](https://snapcraft.io/<placeholdername>) |
| AUR        | [![AUR Version](https://img.shields.io/aur/version/<placeholdername>-bin)](https://aur.archlinux.org/packages/<placeholdername>-bin) |

---

## 🚀 Quick Start

### Prerequisites
- [Godot 4.6+](https://godotengine.org/download)

### Installation

#### Option 1: Clone the Template
```bash
git clone https://github.com/mlm-games/my-ecosystem-template.git
cd my-ecosystem-template
# Open in Godot 4
```

#### Option 2: Download ZIP
[Download latest release](https://github.com/mlm-games/my-ecosystem-template/releases) and extract to your project folder.

### Your First Scene

```gdscript
# In your main game script
extends Node2D

func _ready():
    # Trigger a scene transition
    STransitions.change_scene_with_transition("res://levels/level_2.tscn", "circleIn")
    
    # Add some screen shake
    ScreenEffects.add_trauma(0.5)
    
    # Play a sound
    AudioM.play_sfx("res://sounds/explosion.wav")
```

### Using Game Feel Effects

```gdscript
# Add weapon recoil
func fire_weapon():
    GameFeel.add_recoil($WeaponSprite, Vector2.LEFT, 15.0)
    GameFeel.rumble_controller(0, 0.3, 0.5, 0.1)

# Apply slow-motion on critical hit
func on_critical_hit():
    await GameFeel.slow_motion(0.2, 0.3)

# Knockback enemies
func on_player_hit(enemy: CharacterBody2D):
    var knockback_dir = (enemy.global_position - global_position).normalized()
    GameFeel.apply_knockback(enemy, knockback_dir, 400.0)
```

### Setting Up Transitions

```gdscript
# Simple fade transition
STransitions.change_scene_with_transition("res://scenes/game_over.tscn", "fadeToBlack")

# Custom speed
STransitions.transition("circleIn", false, 2.0)

# Single transition (won't auto-reverse)
STransitions.transition("fadeToBlack", true)
```

---

## 📚 Documentation

### Core Systems

| System | Description | File |
|--------|-------------|------|
| `STransitions` | Scene transition manager | `addons/my-ecosystem-template/transitions/screen_transitions.gd` |
| `AudioM` | Global audio manager | `addons/my-ecosystem-template/managers/audio/autoload_audio_manager.gd` |
| `SaveManager` | Save/load functionality | `addons/my-ecosystem-template/managers/save_manager.gd` |
| `GameFeel` | Screen shake, recoil, slow-mo | `addons/my-ecosystem-template/static_utils/game_feel.gd` |
| `Juice` | Polish effects | `addons/my-ecosystem-template/static_utils/juice.gd` |
| `UIPopups` | Popup animations | `addons/my-ecosystem-template/transitions/popup_animator.gd` |

### Static Utility Classes

All utility classes are static and ready to use anywhere:

```gdscript
# Math utilities
var angle = MathUtils.angle_to_mouse(self)

# Object pooling
var bullet = ObjectPool.acquire("bullets", bullet_scene)
ObjectPool.release("bullets", bullet)

# Common tweens
CommonTweens.pop_in($PopupPanel)
CommonTweens.shake($HealthBar, 0.5)

# VFX spawning
VFXSpawner.spawn_explosion(global_position)
```

### Shaders Included

- **UI Glow** - Beautiful glowing UI elements
- **Flash White** - Damage flash effect
- **Chromatic Aberration** - Glitch/distortion effects

---

## 🎨 Where to Add Demo GIFs

Replace these placeholders with actual recordings of your game:

```markdown
<!-- Place in your repo's /docs or /.github folder -->
![Main Demo](docs/transitions-demo-unneeded.gif)
![Settings?](docs/screenshake-demo.gif)
![Others](docs/audio-demo.gif)
![Misc](docs/pause-menu-demo.gif)
```

**Recording tip:** Use [ScreenToGif](https://www.screentogif.com/) (Windows) or [Kooha](https://github.com/SeaDve/Kooha) (Linux) to capture 5-10 second clips.

---

## 💡 Motivation

Every game developer has been there:

> "I just want to start making my game, but first I need to implement... scene transitions, save systems, audio managers, pause menus, localization, build pipelines..."

**My Ecosystem Template** was born from countless projects where we spent the first week (or month) rebuilding the same foundational systems. This template captures those battle-tested patterns so you can:

- **Start coding your game logic on day one**
- **Ship faster** with production-ready systems
- **Focus on what makes your game unique**
- **Learn from clean, documented code**

Whether you're prototyping a jam game or building the next indie hit, this template gives you a solid foundation to build upon.

---

## 🤝 Contributing

We love contributions! Whether it's:

- 🐛 **Bug fixes** - Found something broken? PRs welcome!
- ✨ **New features** - Got a utility everyone needs?
- 📝 **Documentation** - Help us improve these docs
- 🌍 **Translations** - Add more language support
- 💡 **Ideas** - Open an issue to discuss

### How to Contribute

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

Please read our [Contributing Guide](CONTRIBUTING.md) for details on code style and development setup.

---

## 🙏 Acknowledgments

### Contributors

Thanks to everyone who has contributed to this project (will be auto populated):

<!-- CONTRIBUTORS_START -->
<!-- This section will be auto-populated by all-contributors bot -->

<a href="https://github.com/mlm-games">
  <img src="https://github.com/mlm-games.png?size=50" width="50" height="50" alt="mlm-games" style="border-radius: 50%;">
</a>

<!-- CONTRIBUTORS_END -->

### Special Thanks

- The [Godot Engine](https://godotengine.org/) team for an amazing open-source engine
- Everyone who has provided feedback and bug reports

---

## 📄 License

This project is licensed under the GPL v3.0 License - see the [LICENSE](LICENSE) file for details.

---

<div align="center">

[Report Bug](https://github.com/mlm-games/my-ecosystem-template/issues) • [Request Feature](https://github.com/mlm-games/my-ecosystem-template/issues)

</div>
