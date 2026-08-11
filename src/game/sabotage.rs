use bevy::prelude::*;

use super::{Alive, GamePhase, LocalPlayer, LocalRole, MatchCleanup, Role};
use crate::app::{AppState, Paused};
use game_utils_bevy::screen_effects::{ScreenEffects, Trauma};
use game_utils_bevy::transitions::Transition;
use game_utils_bevy::vfx::VfxSpawner;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    pub fn critical_remaining(&self) -> f32 {
        self.timer
            .as_ref()
            .map(|t| t.remaining_secs())
            .unwrap_or(0.0)
    }
    pub fn fixed(&self) -> bool {
        self.fixes_needed > 0 && self.fixes_done >= self.fixes_needed
    }
    pub fn clear(&mut self) {
        self.kind = None;
        self.timer = None;
        self.fixes_needed = 0;
        self.fixes_done = 0;
    }
}

#[derive(Component)]
pub struct SabotageFixStation {
    pub kind: SabotageKind,
    pub progress: f32,
}

#[derive(Message, Clone, Copy)]
pub enum SabotageAction {
    Lights,
    Oxygen,
    Reactor,
}

pub struct SabotagePlugin;

impl Plugin for SabotagePlugin {
    fn build(&self, app: &mut App) {
        // Registered here ONLY — do not also add_message in game/mod.rs.
        app.init_resource::<ActiveSabotage>()
            .add_message::<SabotageAction>()
            .add_systems(OnEnter(AppState::InGame), spawn_fix_stations)
            .add_systems(
                Update,
                (
                    sabotage_input,
                    apply_sabotage,
                    tick_sabotage,
                    fix_station_interact,
                    check_sabotage_loss,
                    clear_sabotage_when_fixed,
                )
                    .chain()
                    .run_if(in_state(AppState::InGame))
                    .run_if(|p: Res<Paused>| !p.0)
                    .run_if(|t: Res<Transition<AppState>>| !t.block_input)
                    .run_if(|ph: Res<GamePhase>| matches!(*ph, GamePhase::Playing)),
            );
    }
}

fn spawn_fix_stations(mut commands: Commands) {
    let stations = [
        (Vec2::new(-200.0, 80.0), SabotageKind::Oxygen),
        (Vec2::new(200.0, 80.0), SabotageKind::Oxygen),
        (Vec2::new(-200.0, -80.0), SabotageKind::Reactor),
        (Vec2::new(200.0, -80.0), SabotageKind::Reactor),
        (Vec2::new(0.0, 140.0), SabotageKind::Lights),
    ];
    for (pos, kind) in stations {
        commands.spawn((
            MatchCleanup,
            SabotageFixStation {
                kind,
                progress: 0.0,
            },
            Sprite {
                color: Color::srgba(0.3, 0.3, 0.9, 0.0), // invisible until active
                custom_size: Some(Vec2::splat(18.0)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 5.0),
        ));
    }
}

fn sabotage_input(
    keys: Res<ButtonInput<KeyCode>>,
    sab: Res<ActiveSabotage>,
    local_role: Res<LocalRole>,
    mut ev: MessageWriter<SabotageAction>,
) {
    if sab.is_active() || !matches!(local_role.0, Some(Role::Impostor)) {
        return;
    }
    if keys.just_pressed(KeyCode::Digit1) {
        ev.write(SabotageAction::Lights);
    }
    if keys.just_pressed(KeyCode::Digit2) {
        ev.write(SabotageAction::Oxygen);
    }
    if keys.just_pressed(KeyCode::Digit3) {
        ev.write(SabotageAction::Reactor);
    }
}

fn apply_sabotage(
    mut ev: MessageReader<SabotageAction>,
    mut sab: ResMut<ActiveSabotage>,
    mut trauma: ResMut<Trauma>,
    mut commands: Commands,
    local: Query<&Transform, With<LocalPlayer>>,
    mut stations: Query<(&mut Sprite, &mut SabotageFixStation)>,
) {
    for action in ev.read() {
        if sab.is_active() {
            continue;
        }
        let (kind, timer, needs) = match action {
            SabotageAction::Lights => (SabotageKind::Lights, None, 1u8),
            SabotageAction::Oxygen => (
                SabotageKind::Oxygen,
                Some(Timer::from_seconds(30.0, TimerMode::Once)),
                2,
            ),
            SabotageAction::Reactor => (
                SabotageKind::Reactor,
                Some(Timer::from_seconds(45.0, TimerMode::Once)),
                2,
            ),
        };
        sab.kind = Some(kind);
        sab.timer = timer;
        sab.fixes_needed = needs;
        sab.fixes_done = 0;

        for (mut sprite, mut st) in &mut stations {
            if st.kind == kind {
                st.progress = 0.0;
                sprite.color = Color::srgb(0.9, 0.5, 0.1);
            }
        }
        ScreenEffects::add_trauma(&mut trauma, 0.6);
        if let Ok(tf) = local.single() {
            VfxSpawner::spawn_burst(
                &mut commands,
                tf.translation.truncate(),
                8,
                Color::srgb(0.8, 0.3, 0.1),
                (40.0, 90.0),
            );
        }
    }
}

fn tick_sabotage(time: Res<Time>, mut sab: ResMut<ActiveSabotage>) {
    if let Some(ref mut t) = sab.timer {
        t.tick(time.delta());
    }
}

// FIXED: single ResMut — the previous draft's Res + ResMut of the same
// resource in one system is a guaranteed B0002 panic at startup.
fn fix_station_interact(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut sab: ResMut<ActiveSabotage>,
    local: Query<&Transform, (With<LocalPlayer>, With<Alive>)>,
    mut stations: Query<(&mut SabotageFixStation, &mut Sprite, &Transform)>,
) {
    let Some(active_kind) = sab.kind else { return };
    if sab.fixes_needed == 0 || !keys.pressed(KeyCode::KeyE) {
        return;
    }
    let Ok(pt) = local.single() else { return };
    let ppos = pt.translation.truncate();

    for (mut st, mut sprite, tf) in &mut stations {
        if st.kind != active_kind || st.progress >= 1.0 {
            continue;
        }
        if ppos.distance(tf.translation.truncate()) > 38.0 {
            continue;
        }
        st.progress += time.delta_secs() / 2.5;
        if st.progress >= 1.0 {
            st.progress = 1.0;
            sprite.color = Color::srgb(0.2, 0.7, 0.3);
            sab.fixes_done += 1;
        }
        break;
    }
}

fn check_sabotage_loss(
    sab: Res<ActiveSabotage>,
    mut phase: ResMut<GamePhase>,
    mut save: ResMut<crate::save::SaveData>,
    manager: Res<game_utils_bevy::save::SaveManager>,
) {
    let Some(kind) = sab.kind else { return };
    if !matches!(kind, SabotageKind::Oxygen | SabotageKind::Reactor) {
        return;
    }
    if let Some(ref timer) = sab.timer
        && timer.just_finished()
        && sab.fixes_done < sab.fixes_needed
    {
        *phase = GamePhase::GameOver { crew_win: false };
        save.games_played += 1;
        save.impostor_wins += 1;
        let _ = manager.save(&*save);
    }
}

fn clear_sabotage_when_fixed(
    mut sab: ResMut<ActiveSabotage>,
    mut stations: Query<(&mut SabotageFixStation, &mut Sprite)>,
) {
    if !sab.fixed() {
        return;
    }
    sab.clear();
    for (mut st, mut sprite) in &mut stations {
        st.progress = 0.0;
        sprite.color = Color::srgba(0.3, 0.3, 0.9, 0.0);
    }
}
