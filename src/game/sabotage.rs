use bevy::prelude::*;

use super::{Alive, GamePhase, LocalPlayer, MatchCleanup, MatchConfig, Player, Role};
use crate::app::{AppState, Paused};
use game_utils_bevy::screen_effects::{ScreenEffects, Trauma};
use game_utils_bevy::transitions::Transition;
use game_utils_bevy::vfx::VfxSpawner;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum SabotageKind {
    Lights,
    Oxygen,
    Reactor,
}

#[derive(Resource, Default)]
pub struct ActiveSabotage {
    pub kind: Option<SabotageKind>,
    pub timer: Option<Timer>,
    pub fixes_needed: u8,
    pub fixes_done: u8,
}

impl ActiveSabotage {
    pub fn is_active(&self) -> bool {
        self.kind.is_some()
    }

    pub fn is_critical(&self) -> bool {
        matches!(
            self.kind,
            Some(SabotageKind::Oxygen | SabotageKind::Reactor)
        )
    }

    pub fn critical_remaining(&self) -> f32 {
        self.timer
            .as_ref()
            .map(Timer::remaining_secs)
            .unwrap_or(0.0)
    }

    pub fn is_fixed(&self) -> bool {
        self.fixes_needed > 0 && self.fixes_done >= self.fixes_needed
    }

    pub fn clear(&mut self) {
        self.kind = None;
        self.timer = None;
        self.fixes_needed = 0;
        self.fixes_done = 0;
    }
}

#[derive(Resource, Default)]
pub struct SabotageCooldown {
    pub remaining: f32,
}

#[derive(Component)]
pub struct SabotageFixStation {
    pub kind: SabotageKind,
    pub progress: f32,
}

#[derive(Message, Clone, Copy)]
pub struct SabotageAction {
    pub actor_id: u64,
    pub kind: SabotageKind,
}

pub struct SabotagePlugin;

impl Plugin for SabotagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveSabotage>()
            .init_resource::<SabotageCooldown>()
            .add_message::<SabotageAction>()
            .add_systems(
                OnEnter(AppState::InGame),
                (reset_sabotage, spawn_fix_stations).chain(),
            )
            .add_systems(OnExit(AppState::InGame), reset_sabotage)
            .add_systems(
                Update,
                (
                    sabotage_input,
                    apply_sabotage,
                    tick_sabotage,
                    check_sabotage_loss,
                    clear_fixed_sabotage,
                )
                    .chain()
                    .run_if(in_state(AppState::InGame))
                    .run_if(|paused: Res<Paused>| !paused.0)
                    .run_if(|transition: Res<Transition<AppState>>| !transition.block_input)
                    .run_if(super::has_authority),
            );
    }
}

fn reset_sabotage(mut sabotage: ResMut<ActiveSabotage>, mut cooldown: ResMut<SabotageCooldown>) {
    sabotage.clear();
    cooldown.remaining = 0.0;
}

fn spawn_fix_stations(mut commands: Commands) {
    let stations = [
        (Vec2::new(-200.0, 80.0), SabotageKind::Oxygen),
        (Vec2::new(200.0, 80.0), SabotageKind::Oxygen),
        (Vec2::new(-200.0, -80.0), SabotageKind::Reactor),
        (Vec2::new(200.0, -80.0), SabotageKind::Reactor),
        (Vec2::new(0.0, 140.0), SabotageKind::Lights),
    ];

    for (position, kind) in stations {
        commands.spawn((
            MatchCleanup,
            SabotageFixStation {
                kind,
                progress: 0.0,
            },
            Sprite {
                color: Color::srgba(0.3, 0.3, 0.9, 0.0),
                custom_size: Some(Vec2::splat(18.0)),
                ..default()
            },
            Transform::from_xyz(position.x, position.y, 5.0),
        ));
    }
}

fn sabotage_input(
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<GamePhase>,
    sabotage: Res<ActiveSabotage>,
    cooldown: Res<SabotageCooldown>,
    local: Query<(&Player, &Role), (With<LocalPlayer>, With<Alive>)>,
    mut actions: MessageWriter<SabotageAction>,
) {
    if !matches!(*phase, GamePhase::Playing) || sabotage.is_active() || cooldown.remaining > 0.0 {
        return;
    }

    let Ok((player, role)) = local.single() else {
        return;
    };

    if !matches!(role, Role::Impostor) {
        return;
    }

    let kind = if keys.just_pressed(KeyCode::Digit1) {
        Some(SabotageKind::Lights)
    } else if keys.just_pressed(KeyCode::Digit2) {
        Some(SabotageKind::Oxygen)
    } else if keys.just_pressed(KeyCode::Digit3) {
        Some(SabotageKind::Reactor)
    } else {
        None
    };

    if let Some(kind) = kind {
        actions.write(SabotageAction {
            actor_id: player.id,
            kind,
        });
    }
}

fn apply_sabotage(
    mut actions: MessageReader<SabotageAction>,
    config: Res<MatchConfig>,
    mut sabotage: ResMut<ActiveSabotage>,
    mut cooldown: ResMut<SabotageCooldown>,
    actors: Query<(&Player, &Role), With<Alive>>,
    mut stations: Query<(&mut Sprite, &mut SabotageFixStation)>,
    mut trauma: ResMut<Trauma>,
    mut commands: Commands,
    transforms: Query<(&Player, &Transform)>,
) {
    for action in actions.read() {
        if sabotage.is_active() || cooldown.remaining > 0.0 {
            continue;
        }

        let valid_actor = actors
            .iter()
            .any(|(player, role)| player.id == action.actor_id && matches!(role, Role::Impostor));

        if !valid_actor {
            continue;
        }

        let (timer, fixes_needed) = match action.kind {
            SabotageKind::Lights => (None, 1),
            SabotageKind::Oxygen => (
                Some(Timer::from_seconds(config.oxygen_time, TimerMode::Once)),
                2,
            ),
            SabotageKind::Reactor => (
                Some(Timer::from_seconds(config.reactor_time, TimerMode::Once)),
                2,
            ),
        };

        sabotage.kind = Some(action.kind);
        sabotage.timer = timer;
        sabotage.fixes_needed = fixes_needed;
        sabotage.fixes_done = 0;

        cooldown.remaining = config.sabotage_cooldown;

        for (mut sprite, mut station) in &mut stations {
            station.progress = 0.0;

            sprite.color = if station.kind == action.kind {
                Color::srgb(0.9, 0.5, 0.1)
            } else {
                Color::srgba(0.3, 0.3, 0.9, 0.0)
            };
        }

        ScreenEffects::add_trauma(&mut trauma, 0.6);

        if let Some((_, transform)) = transforms
            .iter()
            .find(|(player, _)| player.id == action.actor_id)
        {
            VfxSpawner::spawn_burst(
                &mut commands,
                transform.translation.truncate(),
                8,
                Color::srgb(0.8, 0.3, 0.1),
                (40.0, 90.0),
            );
        }
    }
}

fn tick_sabotage(
    time: Res<Time>,
    phase: Res<GamePhase>,
    mut sabotage: ResMut<ActiveSabotage>,
    mut cooldown: ResMut<SabotageCooldown>,
) {
    cooldown.remaining = (cooldown.remaining - time.delta_secs()).max(0.0);

    if !matches!(*phase, GamePhase::Playing) {
        return;
    }

    if let Some(timer) = sabotage.timer.as_mut() {
        timer.tick(time.delta());
    }
}

fn check_sabotage_loss(
    sabotage: Res<ActiveSabotage>,
    mut phase: ResMut<GamePhase>,
    mut save: ResMut<crate::save::SaveData>,
    manager: Res<game_utils_bevy::save::SaveManager>,
) {
    if matches!(*phase, GamePhase::GameOver { .. }) {
        return;
    }

    if !sabotage.is_critical() || sabotage.is_fixed() {
        return;
    }

    let expired = sabotage.timer.as_ref().is_some_and(Timer::just_finished);

    if !expired {
        return;
    }

    *phase = GamePhase::GameOver { crew_win: false };
    save.games_played = save.games_played.saturating_add(1);
    save.impostor_wins = save.impostor_wins.saturating_add(1);

    if let Err(error) = manager.save(&*save) {
        warn!("Unable to save sabotage result: {error}");
    }
}

fn clear_fixed_sabotage(
    mut sabotage: ResMut<ActiveSabotage>,
    mut stations: Query<(&mut SabotageFixStation, &mut Sprite)>,
) {
    if !sabotage.is_fixed() {
        return;
    }

    sabotage.clear();

    for (mut station, mut sprite) in &mut stations {
        station.progress = 0.0;
        sprite.color = Color::srgba(0.3, 0.3, 0.9, 0.0);
    }
}
