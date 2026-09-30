use bevy::prelude::*;

use super::{Alive, Body, Ghost, MatchCleanup, PlayerIntent, PlayerLayer};
use crate::app::AppState;

pub struct PolishPlugin;

impl Plugin for PolishPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (player_bob_and_y_sort, body_y_sort)
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
    }
}

#[inline]
fn actor_z(y: f32) -> f32 {
    30.0 - y * 0.01
}

fn player_bob_and_y_sort(
    time: Res<Time>,
    mut players: Query<(&PlayerIntent, &mut Transform, &Children), Or<(With<Alive>, With<Ghost>)>>,
    mut layers: Query<&mut Transform, With<PlayerLayer>>,
) {
    for (intent, mut root, children) in &mut players {
        root.translation.z = actor_z(root.translation.y);

        let moving = intent.movement.length_squared() > 0.01;
        let t = time.elapsed_secs() * if moving { 13.5 } else { 2.0 };

        let bob = if moving {
            t.sin().abs() * 2.2
        } else {
            t.sin() * 0.25
        };

        let squash = if moving { 1.0 + t.sin() * 0.025 } else { 1.0 };

        for child in children.iter() {
            if let Ok(mut tf) = layers.get_mut(child) {
                tf.translation.y = bob;
                tf.scale.y = squash;
            }
        }
    }
}

fn body_y_sort(mut bodies: Query<&mut Transform, (With<Body>, With<MatchCleanup>)>) {
    for mut tf in &mut bodies {
        tf.translation.z = actor_z(tf.translation.y) - 0.05;
    }
}
