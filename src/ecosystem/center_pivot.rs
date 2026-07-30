use bevy::prelude::*;

#[derive(Component)]
pub struct CenterPivot;

pub fn apply_center_pivot(
    mut q: Query<(&mut Transform, &Sprite), (With<CenterPivot>, Changed<Sprite>)>,
) {
    for (mut tf, sprite) in &mut q {
        if let Some(size) = sprite.custom_size {
            tf.translation = tf.translation.truncate().extend(tf.translation.z);
            tf.translation.x -= size.x * 0.5;
            tf.translation.y -= size.y * 0.5;
        }
    }
}
