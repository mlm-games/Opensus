use bevy::input::gamepad::{Gamepad, GamepadRumbleRequest};
use bevy::prelude::*;

use super::{
    Alive, Body, CHARACTER_HEIGHT, GameAssets, GamePhase, Ghost, LocalPlayer, MatchCleanup,
    MatchConfig, MeetingState, Player, Role, make_ghost,
};
use crate::app::{AppState, Paused};
use game_utils_bevy::game_feel::GameFeel;
use game_utils_bevy::screen_effects::{ScreenEffects, Trauma};
use game_utils_bevy::transitions::Transition;
use game_utils_bevy::vfx::VfxSpawner;

#[derive(Resource, Default)]
pub struct KillCooldown {
    pub remaining: f32,
}

#[derive(Message, Clone, Copy)]
pub struct KillRequest {
    pub actor_id: u64,
}

#[derive(Message, Clone, Copy)]
pub struct ReportBody {
    pub reporter_id: u64,
}

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
                .run_if(|ph: Res<GamePhase>| matches!(*ph, GamePhase::Playing))
                .run_if(super::has_authority),
        );
    }
}

fn tick_kill_cd(time: Res<Time>, mut cd: ResMut<KillCooldown>) {
    cd.remaining = (cd.remaining - time.delta_secs()).max(0.0);
}

fn kill_input(
    keys: Res<ButtonInput<KeyCode>>,
    local: Query<&Player, (With<LocalPlayer>, With<Alive>)>,
    mut requests: MessageWriter<KillRequest>,
) {
    if !keys.just_pressed(KeyCode::KeyQ) {
        return;
    }
    let Ok(player) = local.single() else {
        return;
    };
    requests.write(KillRequest {
        actor_id: player.id,
    });
}

fn do_kill(
    mut requests: MessageReader<KillRequest>,
    mut commands: Commands,
    cfg: Res<MatchConfig>,
    mut cd: ResMut<KillCooldown>,
    mut trauma: ResMut<Trauma>,
    assets: Res<GameAssets>,
    actors: Query<(&Transform, &Role, &Player), With<Alive>>,
    targets: Query<(Entity, &Player, &Transform, &Role, Option<&Children>), With<Alive>>,
    mut sprites: Query<&mut Sprite>,
    gamepads: Query<(Entity, &Gamepad)>,
    mut rumble: MessageWriter<GamepadRumbleRequest>,
) {
    for request in requests.read() {
        let actor_id = request.actor_id;

        if cd.remaining > 0.0 {
            continue;
        }

        let Some((actor_transform, actor_role, _)) =
            actors.iter().find(|(_, _, player)| player.id == actor_id)
        else {
            continue;
        };

        if !matches!(actor_role, Role::Impostor) {
            continue;
        }

        let actor_position = actor_transform.translation.truncate();
        let mut best: Option<(Entity, Vec2, u64, String, u8)> = None;
        let mut best_d = cfg.kill_range;
        for (e, p, t, r, _) in &targets {
            if p.id == actor_id {
                continue; // no self kill
            }
            if matches!(r, Role::Impostor) {
                continue; // no team kill in v1
            }
            let d = actor_position.distance(t.translation.truncate());
            if d < best_d {
                best_d = d;
                best = Some((e, t.translation.truncate(), p.id, p.name.clone(), p.color_index));
            }
        }
        let Some((victim, pos, id, name, color_index)) = best else {
            continue;
        };
        cd.remaining = cfg.kill_cooldown;

        let children = targets.get(victim).ok().and_then(|t| t.4);
        make_ghost(&mut commands, victim, children, &mut sprites);

        commands.spawn((
            MatchCleanup,
            Body {
                player_id: id,
                name,
                reported: false,
            },
            Sprite {
                image: assets.clothes_for(color_index),
                color: Color::srgba(0.55, 0.1, 0.12, 0.95),
                custom_size: Some(Vec2::new(CHARACTER_HEIGHT * 0.7, CHARACTER_HEIGHT * 0.35)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y - 10.0, 3.0)
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
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

fn report_input(
    keys: Res<ButtonInput<KeyCode>>,
    local: Query<&Player, (With<LocalPlayer>, With<Alive>)>,
    mut requests: MessageWriter<ReportBody>,
) {
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    let Ok(player) = local.single() else {
        return;
    };
    requests.write(ReportBody {
        reporter_id: player.id,
    });
}

fn do_report(
    mut requests: MessageReader<ReportBody>,
    mut phase: ResMut<GamePhase>,
    mut meeting: ResMut<MeetingState>,
    config: Res<MatchConfig>,
    mut sabotage: ResMut<super::ActiveSabotage>,
    reporters: Query<(&Player, &Transform), With<Alive>>,
    mut bodies: Query<(Entity, &mut Body, &Transform)>,
    players: Query<(&Player, Option<&Alive>, Option<&Ghost>)>,
    mut trauma: ResMut<Trauma>,
) {
    for request in requests.read() {
        if !matches!(*phase, GamePhase::Playing) {
            continue;
        }

        let Some((_, reporter_transform)) = reporters
            .iter()
            .find(|(player, _)| player.id == request.reporter_id)
        else {
            continue;
        };

        let reporter_position = reporter_transform.translation.truncate();

        let nearest = bodies
            .iter()
            .filter(|(_, body, _)| !body.reported)
            .filter_map(|(entity, body, transform)| {
                let distance = reporter_position.distance(transform.translation.truncate());

                (distance <= config.report_range).then_some((entity, body.name.clone(), distance))
            })
            .min_by(|left, right| {
                left.2
                    .partial_cmp(&right.2)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

        let Some((body_entity, victim_name, _)) = nearest else {
            continue;
        };

        if let Ok((_, mut body, _)) = bodies.get_mut(body_entity) {
            body.reported = true;
        }

        // A report cancels the active sabotage and begins a meeting.
        sabotage.clear();

        ScreenEffects::add_trauma(&mut trauma, 0.4);

        meeting.begin_meeting(
            format!("{victim_name}'s body was reported!"),
            &players,
            config.discussion_time,
        );

        *phase = GamePhase::Meeting;
    }
}
