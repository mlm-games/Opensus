use bevy::prelude::*;

use super::{Alive, LocalPlayer, MatchCleanup, Role};
use crate::app::{AppState, Paused};
use game_utils_bevy::juice::Juice;
use game_utils_bevy::transitions::Transition;
use game_utils_bevy::vfx::VfxSpawner;

#[derive(Resource, Default)]
pub struct TaskBoard {
    pub completed: u32,
    pub total: u32,
}

#[derive(Component)]
pub struct TaskStation {
    pub id: u32,
    pub label: &'static str,
    pub progress: f32,
    pub done: bool,
}

#[derive(Message)]
pub struct TaskInteract;

pub struct TasksPlugin;
impl Plugin for TasksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), spawn_task_stations)
            .add_systems(
                Update,
                (task_interact_input, tick_task_hold)
                    .run_if(in_state(AppState::InGame))
                    .run_if(|p: Res<Paused>| !p.0)
                    .run_if(|t: Res<Transition<AppState>>| !t.block_input)
                    .run_if(|ph: Res<super::GamePhase>| matches!(*ph, super::GamePhase::Playing)),
            );
    }
}

fn spawn_task_stations(mut commands: Commands) {
    let stations = [
        (1, "Wire tap", Vec2::new(-280.0, 120.0)),
        (2, "Decode", Vec2::new(280.0, 120.0)),
        (3, "Stabilize", Vec2::new(-280.0, -120.0)),
        (4, "Scan", Vec2::new(280.0, -120.0)),
        (5, "Upload", Vec2::new(0.0, 40.0)),
    ];
    for (id, label, pos) in stations {
        let e = commands
            .spawn((
                MatchCleanup,
                TaskStation {
                    id,
                    label,
                    progress: 0.0,
                    done: false,
                },
                Sprite {
                    color: Color::srgb(0.35, 0.7, 0.45),
                    custom_size: Some(Vec2::splat(22.0)),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, 4.0),
            ))
            .id();
        Juice::pop_in(&mut commands, e, 0.2);
    }
}

fn task_interact_input(keys: Res<ButtonInput<KeyCode>>, mut ev: MessageWriter<TaskInteract>) {
    if keys.just_pressed(KeyCode::KeyE) {
        ev.write(TaskInteract);
    }
}

fn tick_task_hold(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut board: ResMut<TaskBoard>,
    mut stations: Query<(&mut TaskStation, &mut Sprite, &Transform)>,
    player: Query<(&Transform, &Role), (With<LocalPlayer>, With<Alive>)>,
    mut commands: Commands,
) {
    let Ok((pt, role)) = player.single() else {
        return;
    };
    // Impostors can stand near and "fake" — no board progress
    let holding = keys.pressed(KeyCode::KeyE);
    if !holding {
        return;
    }
    let ppos = pt.translation.truncate();
    for (mut st, mut sprite, tf) in &mut stations {
        if st.done {
            continue;
        }
        if ppos.distance(tf.translation.truncate()) > 40.0 {
            continue;
        }
        st.progress += time.delta_secs() / 2.0; // 2s hold
        if st.progress >= 1.0 {
            st.progress = 1.0;
            st.done = true;
            sprite.color = Color::srgb(0.2, 0.35, 0.25);
            if matches!(role, Role::Crewmate) {
                board.completed += 1;
                VfxSpawner::spawn_burst(
                    &mut commands,
                    tf.translation.truncate(),
                    10,
                    Color::srgb(0.4, 0.9, 0.5),
                    (30.0, 80.0),
                );
            }
        }
        break;
    }
}
