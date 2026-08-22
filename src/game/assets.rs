use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

use crate::asset_tracking::AssetsLoading;

/// World-space height for character sprites (portrait art).
pub const CHARACTER_HEIGHT: f32 = 56.0;

#[derive(Resource, Clone)]
pub struct GameAssets {
    pub body_templates: Vec<Handle<Image>>,
    pub clothes: Vec<Handle<Image>>,
    pub floor_wood: Handle<Image>,
    pub floor_carpet: Handle<Image>,
    pub wall_front: Handle<Image>,
    pub wall_side: Handle<Image>,
    pub table: Handle<Image>,
    pub seat: Handle<Image>,
    #[allow(dead_code, reason = "Reserved for doors; wired for Repose/title art")]
    pub door: Handle<Image>,
    pub task_beaker: Handle<Image>,
    pub task_flask: Handle<Image>,
    pub task_burner: Handle<Image>,
    #[allow(dead_code, reason = "Loaded with game art; kept for future avatars")]
    pub player_placeholder: Handle<Image>,
    #[allow(
        dead_code,
        reason = "Uploaded to Repose at startup by ensure_ui_images"
    )]
    pub ui_lab_bg: Handle<Image>,
    #[allow(
        dead_code,
        reason = "Uploaded to Repose at startup by ensure_ui_images"
    )]
    pub ui_background: Handle<Image>,
}

impl GameAssets {
    pub fn load(asset_server: &AssetServer) -> (Self, Vec<UntypedHandle>) {
        let body_paths = [
            "sprites/character/textures/body/black_round_eyes-small_nose.png",
            "sprites/character/textures/body/black_round_eyes-big_nose.png",
            "sprites/character/textures/body/black_round_eyes-sharp_nose.png",
            "sprites/character/textures/body/closed_eyes-small_nose.png",
            "sprites/character/textures/body/closed_eyes-big_nose.png",
            "sprites/character/textures/body/closed_eyes-sharp_nose.png",
        ];
        let clothes_paths = [
            "sprites/character/textures/clothes/labcoat.png",
            "sprites/character/textures/clothes/pink_sweater-blue_jeans.png",
            "sprites/character/textures/clothes/beige_suit.png",
            "sprites/character/textures/clothes/brown_suit.png",
            "sprites/character/textures/clothes/blue_blaser_skirt.png",
            "sprites/character/textures/clothes/green_dress.png",
            "sprites/character/textures/clothes/leather_jacket.png",
            "sprites/character/textures/clothes/police_uniform.png",
            "sprites/character/textures/clothes/tuxedo.png",
            "sprites/character/textures/clothes/checked_shirt.png",
            "sprites/character/textures/clothes/white_shirt-red_sweater.png",
            "sprites/character/textures/clothes/labcoat.png",
        ];

        let mut handles: Vec<UntypedHandle> = Vec::new();

        let body_templates: Vec<_> = body_paths
            .iter()
            .map(|p| load_handle(asset_server, &mut handles, p))
            .collect();
        let clothes: Vec<_> = clothes_paths
            .iter()
            .map(|p| load_handle(asset_server, &mut handles, p))
            .collect();

        let assets = Self {
            body_templates,
            clothes,
            floor_wood: load_handle(asset_server, &mut handles, "sprites/map/hardwood.png"),
            floor_carpet: load_handle(asset_server, &mut handles, "sprites/map/carpet.png"),
            wall_front: load_handle(asset_server, &mut handles, "sprites/map/wall_front.png"),
            wall_side: load_handle(asset_server, &mut handles, "sprites/map/wall_side.png"),
            table: load_handle(asset_server, &mut handles, "sprites/map/table.png"),
            seat: load_handle(asset_server, &mut handles, "sprites/map/seat.png"),
            door: load_handle(asset_server, &mut handles, "sprites/map/door.png"),
            task_beaker: load_handle(
                asset_server,
                &mut handles,
                "items/ingame_texture/beaker-empty.png",
            ),
            task_flask: load_handle(
                asset_server,
                &mut handles,
                "items/ingame_texture/flask-empty.png",
            ),
            task_burner: load_handle(
                asset_server,
                &mut handles,
                "items/ingame_texture/gas_burner.png",
            ),
            player_placeholder: load_handle(
                asset_server,
                &mut handles,
                "sprites/ui/player_placeholder.png",
            ),
            ui_lab_bg: load_handle(asset_server, &mut handles, "sprites/ui/lab-bg.png"),
            ui_background: load_handle(asset_server, &mut handles, "sprites/ui/background.png"),
        };

        (assets, handles)
    }

    pub fn body_for(&self, color_index: u8) -> Handle<Image> {
        self.body_templates[color_index as usize % self.body_templates.len()].clone()
    }

    pub fn clothes_for(&self, color_index: u8) -> Handle<Image> {
        self.clothes[color_index as usize % self.clothes.len()].clone()
    }
}

fn load_handle(
    asset_server: &AssetServer,
    handles: &mut Vec<UntypedHandle>,
    path: &'static str,
) -> Handle<Image> {
    let h = asset_server.load(path);
    handles.push(h.clone().untyped());
    h
}

/// Magenta palette-swap body → solid player/skin color (no custom shader needed).
pub fn bake_body_tint(
    images: &mut Assets<Image>,
    template: &Handle<Image>,
    tint: Color,
) -> Option<Handle<Image>> {
    let src = images.get(template)?;
    let Some(data) = &src.data else {
        return None;
    };
    let mut bytes = data.clone();

    // Match replacement values to the texture's color space so the baked tint
    // isn't double-encoded (linear for Rgba8Unorm, sRGB for Rgba8UnormSrgb).
    let (tr, tg, tb) = match src.texture_descriptor.format {
        TextureFormat::Rgba8UnormSrgb => {
            let s = tint.to_srgba();
            (
                (s.red * 255.0) as u8,
                (s.green * 255.0) as u8,
                (s.blue * 255.0) as u8,
            )
        }
        _ => {
            let l = tint.to_linear();
            (
                (l.red * 255.0) as u8,
                (l.green * 255.0) as u8,
                (l.blue * 255.0) as u8,
            )
        }
    };

    for px in bytes.chunks_exact_mut(4) {
        let (r, g, b, a) = (px[0], px[1], px[2], px[3]);
        if a < 8 {
            continue;
        }
        // OpenSuspect-style magenta body key.
        let is_key = r > 80 && b > 80 && g < r.saturating_sub(30) && g < b.saturating_sub(30);
        if is_key {
            px[0] = tr;
            px[1] = tg;
            px[2] = tb;
        }
    }

    let mut img = Image::new(
        src.texture_descriptor.size,
        src.texture_descriptor.dimension,
        bytes,
        src.texture_descriptor.format,
        src.asset_usage,
    );
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::nearest());
    Some(images.add(img))
}

pub struct GameAssetsPlugin;
impl Plugin for GameAssetsPlugin {
    fn build(&self, _app: &mut App) {
        // The GameAssets resource and its loading gate are inserted by
        // `begin_game_asset_load`, run on `AppState::Loading` (see screens).
    }
}

/// Call from Loading OnEnter (see screens).
pub fn begin_game_asset_load(mut commands: Commands, asset_server: Res<AssetServer>) {
    let (assets, handles) = GameAssets::load(&asset_server);
    commands.insert_resource(assets);
    commands.insert_resource(AssetsLoading(handles));
}
