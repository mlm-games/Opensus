use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::{GamePhase, Ghost, LocalPlayer, MatchCleanup, Role, SabotageKind};
use crate::app::AppState;

const MASK_RESOLUTION: u32 = 512;
const MASK_WORLD_SIZE: f32 = 1800.0;
const MAX_DARKNESS_ALPHA: f32 = 0.94;
const FEATHER_WORLD_SIZE: f32 = 46.0;

#[derive(Resource)]
struct VisionMasks {
    crew: Handle<Image>,
    lights: Handle<Image>,
    impostor: Handle<Image>,
}

#[derive(Component)]
struct VisionMask;

pub struct VisionPlugin;

impl Plugin for VisionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, create_masks)
            .add_systems(OnEnter(AppState::InGame), spawn_mask)
            .add_systems(Update, update_mask.run_if(in_state(AppState::InGame)));
    }
}

fn create_masks(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let crew = images.add(make_mask(175.0));
    let lights = images.add(make_mask(72.0));
    let impostor = images.add(make_mask(275.0));

    commands.insert_resource(VisionMasks {
        crew,
        lights,
        impostor,
    });
}

fn make_mask(clear_radius_world: f32) -> Image {
    let mut image = Image::new_fill(
        Extent3d {
            width: MASK_RESOLUTION,
            height: MASK_RESOLUTION,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );

    let center = Vec2::splat(MASK_RESOLUTION as f32 * 0.5);
    let pixels_per_world = MASK_RESOLUTION as f32 / MASK_WORLD_SIZE;

    let radius_pixels = clear_radius_world * pixels_per_world;
    let feather_pixels = FEATHER_WORLD_SIZE * pixels_per_world;

    for y in 0..MASK_RESOLUTION {
        for x in 0..MASK_RESOLUTION {
            let pixel_position = Vec2::new(x as f32, y as f32);
            let distance = pixel_position.distance(center);

            let darkness = if distance <= radius_pixels {
                0.0
            } else {
                ((distance - radius_pixels) / feather_pixels.max(1.0)).clamp(0.0, 1.0)
                    * MAX_DARKNESS_ALPHA
            };

            if let Ok(bytes) = image.pixel_bytes_mut(UVec3::new(x, y, 0)) {
                bytes[0] = 0;
                bytes[1] = 0;
                bytes[2] = 0;
                bytes[3] = (darkness * 255.0) as u8;
            }
        }
    }

    image
}

fn spawn_mask(mut commands: Commands, masks: Res<VisionMasks>) {
    let mut sprite = Sprite::from_image(masks.crew.clone());
    sprite.custom_size = Some(Vec2::splat(MASK_WORLD_SIZE));

    commands.spawn((
        MatchCleanup,
        VisionMask,
        sprite,
        Transform::from_xyz(0.0, 0.0, 900.0),
    ));
}

fn update_mask(
    phase: Res<GamePhase>,
    sabotage: Res<super::ActiveSabotage>,
    masks: Res<VisionMasks>,
    local: Query<(&Transform, Option<&Role>, Option<&Ghost>), With<LocalPlayer>>,
    mut mask: Query<(&mut Transform, &mut Sprite, &mut Visibility), With<VisionMask>>,
) {
    let Ok((player_transform, role, ghost)) = local.single() else {
        return;
    };

    let Ok((mut mask_transform, mut sprite, mut visibility)) = mask.single_mut() else {
        return;
    };

    let should_hide = ghost.is_some() || !matches!(*phase, GamePhase::Playing);

    *visibility = if should_hide {
        Visibility::Hidden
    } else {
        Visibility::Visible
    };

    if should_hide {
        return;
    }

    sprite.image = match role {
        Some(Role::Impostor) => masks.impostor.clone(),
        Some(Role::Crewmate) if matches!(sabotage.kind, Some(SabotageKind::Lights)) => {
            masks.lights.clone()
        }
        _ => masks.crew.clone(),
    };

    mask_transform.translation.x = player_transform.translation.x;
    mask_transform.translation.y = player_transform.translation.y;
}
