use bevy::input::gamepad::{Gamepad, GamepadRumbleRequest};
use bevy::prelude::*;

use super::{Alive, Body, GamePhase, Ghost, LocalPlayer, MatchCleanup, MatchConfig, Player, Role};
use crate::app::{AppState, Paused};
use game_utils_bevy::game_feel::GameFeel;
use game_utils_bevy::screen_effects::{ScreenEffects, Trauma};
use game_utils_bevy::transitions::Transition;
use game_utils_bevy::vfx::VfxSpawner;

#[derive(Resource, Default)]
pub struct KillCooldown {
    pub remaining: f32,
}

#[derive(Message)]
pub struct KillRequest;

#[derive(Message)]
pub struct ReportBody;

pub struct KillSabotagePlugin;
impl Plugin for KillSabotagePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (tick_kill_cd, kill_input, do_kill, report_input, do_report)
                .chain()
                .run_if(in_state(AppState::InGame))
                .run_if(|p: Res<Paused>| !p.0)
                .run_if(|t: Res<Transition<AppState>>| !t.block_input)
                .run_if(|ph: Res<GamePhase>| matches!(*ph, GamePhase::Playing)),
        );
    }
}

fn tick_kill_cd(time: Res<Time>, mut cd: ResMut<KillCooldown>) {
    cd.remaining = (cd.remaining - time.delta_secs()).max(0.0);
}

fn kill_input(keys: Res<ButtonInput<KeyCode>>, mut ev: MessageWriter<KillRequest>) {
    if keys.just_pressed(KeyCode::KeyQ) {
        ev.write(KillRequest);
    }
}

fn do_kill(
    mut ev: MessageReader<KillRequest>,
    mut commands: Commands,
    cfg: Res<MatchConfig>,
    mut cd: ResMut<KillCooldown>,
    mut trauma: ResMut<Trauma>,
    local: Query<(&Transform, &Role), (With<LocalPlayer>, With<Alive>)>,
    targets: Query<(Entity, &Player, &Transform, &Role), (With<Alive>, Without<LocalPlayer>)>,
    gamepads: Query<(Entity, &Gamepad)>,
    mut rumble: MessageWriter<GamepadRumbleRequest>,
) {
    for _ in ev.read() {
        if cd.remaining > 0.0 {
            continue;
        }
        let Ok((lt, role)) = local.single() else {
            continue;
        };
        if !matches!(role, Role::Impostor) {
            continue;
        }
        let lpos = lt.translation.truncate();
        let mut best: Option<(Entity, Vec2, u64, String)> = None;
        let mut best_d = cfg.kill_range;
        for (e, p, t, r) in &targets {
            if matches!(r, Role::Impostor) {
                continue; // no team kill in v1
            }
            let d = lpos.distance(t.translation.truncate());
            if d < best_d {
                best_d = d;
                best = Some((e, t.translation.truncate(), p.id, p.name.clone()));
            }
        }
        let Some((victim, pos, id, name)) = best else {
            continue;
        };
        cd.remaining = cfg.kill_cooldown;
        // Kill
        commands.entity(victim).remove::<Alive>();
        commands.entity(victim).insert(Ghost);
        commands.entity(victim).insert(Sprite {
            color: Color::srgba(0.7, 0.7, 0.8, 0.35),
            custom_size: Some(Vec2::splat(28.0)),
            ..default()
        });
        // Body
        commands.spawn((
            MatchCleanup,
            Body {
                player_id: id,
                name,
            },
            Sprite {
                color: Color::srgb(0.5, 0.05, 0.08),
                custom_size: Some(Vec2::new(30.0, 14.0)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y - 8.0, 3.0),
        ));
        ScreenEffects::add_trauma(&mut trauma, 0.55);
        GameFeel::rumble_controller(&mut rumble, &gamepads, 0.5, 0.9, 0.2);
        VfxSpawner::spawn_burst(
            &mut commands,
            pos,
            14,
            Color::srgb(0.9, 0.15, 0.1),
            (50.0, 120.0),
        );
    }
}

fn report_input(keys: Res<ButtonInput<KeyCode>>, mut ev: MessageWriter<ReportBody>) {
    if keys.just_pressed(KeyCode::KeyR) {
        ev.write(ReportBody);
    }
}

fn do_report(
    mut ev: MessageReader<ReportBody>,
    mut phase: ResMut<GamePhase>,
    mut meeting: ResMut<super::MeetingState>,
    cfg: Res<MatchConfig>,
    local: Query<&Transform, (With<LocalPlayer>, With<Alive>)>,
    bodies: Query<(&Body, &Transform)>,
    players: Query<(&Player, Option<&Alive>, Option<&Ghost>)>,
    mut trauma: ResMut<Trauma>,
) {
    for _ in ev.read() {
        let Ok(lt) = local.single() else {
            continue;
        };
        let lpos = lt.translation.truncate();
        let near_body = bodies
            .iter()
            .any(|(_, t)| lpos.distance(t.translation.truncate()) < 50.0);
        if !near_body {
            continue;
        }
        ScreenEffects::add_trauma(&mut trauma, 0.4);
        meeting.begin_meeting("Body reported!".into(), &players, cfg.discussion_time);
        *phase = GamePhase::Meeting;
    }
}
