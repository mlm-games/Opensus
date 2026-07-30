use bevy::prelude::*;

/// Marker component - Bevy sprites are already centered by default.
/// Add this to a sprite entity if you want to track that it uses center pivot
/// (useful when replacing texture assets with different sizes).
/// But might actually be removed later (just a remnant of the port)
#[derive(Component)]
pub struct CenterPivot;

pub struct CenterPivotPlugin;

impl Plugin for CenterPivotPlugin {
    fn build(&self, _app: &mut App) {}
}
