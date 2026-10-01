use repose_core::{ImageHandle, RenderContext};

use crate::game::PLAYER_COLORS;

const CARPET: &[u8] = include_bytes!("../assets/sprites/map/carpet.png");
const HARDWOOD: &[u8] = include_bytes!("../assets/sprites/map/hardwood.png");
const WALL_FRONT: &[u8] = include_bytes!("../assets/sprites/map/wall_front.png");
const WALL_SIDE: &[u8] = include_bytes!("../assets/sprites/map/wall_side.png");
const TABLE: &[u8] = include_bytes!("../assets/sprites/map/table.png");
const SEAT: &[u8] = include_bytes!("../assets/sprites/map/seat.png");
const DOOR: &[u8] = include_bytes!("../assets/sprites/map/door.png");
const TASK_BEAKER: &[u8] = include_bytes!("../assets/items/ingame_texture/beaker-empty.png");
const TASK_FLASK: &[u8] = include_bytes!("../assets/items/ingame_texture/flask-empty.png");
const TASK_BURNER: &[u8] = include_bytes!("../assets/items/ingame_texture/gas_burner.png");

const BODIES: [&[u8]; 6] = [
    include_bytes!("../assets/sprites/character/textures/body/black_round_eyes-small_nose.png"),
    include_bytes!("../assets/sprites/character/textures/body/black_round_eyes-big_nose.png"),
    include_bytes!("../assets/sprites/character/textures/body/black_round_eyes-sharp_nose.png"),
    include_bytes!("../assets/sprites/character/textures/body/closed_eyes-small_nose.png"),
    include_bytes!("../assets/sprites/character/textures/body/closed_eyes-big_nose.png"),
    include_bytes!("../assets/sprites/character/textures/body/closed_eyes-sharp_nose.png"),
];

const CLOTHES: [&[u8]; 12] = [
    include_bytes!("../assets/sprites/character/textures/clothes/labcoat.png"),
    include_bytes!("../assets/sprites/character/textures/clothes/pink_sweater-blue_jeans.png"),
    include_bytes!("../assets/sprites/character/textures/clothes/beige_suit.png"),
    include_bytes!("../assets/sprites/character/textures/clothes/brown_suit.png"),
    include_bytes!("../assets/sprites/character/textures/clothes/blue_blaser_skirt.png"),
    include_bytes!("../assets/sprites/character/textures/clothes/green_dress.png"),
    include_bytes!("../assets/sprites/character/textures/clothes/leather_jacket.png"),
    include_bytes!("../assets/sprites/character/textures/clothes/police_uniform.png"),
    include_bytes!("../assets/sprites/character/textures/clothes/tuxedo.png"),
    include_bytes!("../assets/sprites/character/textures/clothes/checked_shirt.png"),
    include_bytes!("../assets/sprites/character/textures/clothes/white_shirt-red_sweater.png"),
    include_bytes!("../assets/sprites/character/textures/clothes/labcoat.png"),
];

#[derive(Default)]
pub struct GameImages {
    pub floor_wood: Option<ImageHandle>,
    pub floor_carpet: Option<ImageHandle>,
    pub wall_front: Option<ImageHandle>,
    pub wall_side: Option<ImageHandle>,
    pub table: Option<ImageHandle>,
    pub seat: Option<ImageHandle>,
    pub door: Option<ImageHandle>,
    pub task: [Option<ImageHandle>; 3],
    pub bodies: [Option<ImageHandle>; 12],
    pub clothes: [Option<ImageHandle>; 12],
    pub vision_crew: Option<ImageHandle>,
    pub vision_lights: Option<ImageHandle>,
    pub vision_impostor: Option<ImageHandle>,
}

impl GameImages {
    pub fn is_loaded(&self) -> bool {
        self.floor_wood.is_some()
    }

    pub fn load(ctx: &RenderContext) -> Self {
        Self {
            floor_wood: Some(ctx.image_from_encoded(HARDWOOD, true)),
            floor_carpet: Some(ctx.image_from_encoded(CARPET, true)),
            wall_front: Some(ctx.image_from_encoded(WALL_FRONT, true)),
            wall_side: Some(ctx.image_from_encoded(WALL_SIDE, true)),
            table: Some(ctx.image_from_encoded(TABLE, true)),
            seat: Some(ctx.image_from_encoded(SEAT, true)),
            door: Some(ctx.image_from_encoded(DOOR, true)),
            task: [
                Some(ctx.image_from_encoded(TASK_BEAKER, true)),
                Some(ctx.image_from_encoded(TASK_FLASK, true)),
                Some(ctx.image_from_encoded(TASK_BURNER, true)),
            ],
            bodies: bake_bodies(ctx),
            clothes: CLOTHES.map(|bytes| Some(ctx.image_from_encoded(bytes, true))),
            vision_crew: Some(bake_mask(ctx, 175.0)),
            vision_lights: Some(bake_mask(ctx, 72.0)),
            vision_impostor: Some(bake_mask(ctx, 275.0)),
        }
    }
}

fn bake_bodies(ctx: &RenderContext) -> [Option<ImageHandle>; 12] {
    std::array::from_fn(|color_index| {
        let template = image::load_from_memory(BODIES[color_index % BODIES.len()]).ok()?;
        let mut rgba = template.into_rgba8();
        let color = PLAYER_COLORS[color_index % PLAYER_COLORS.len()];
        for px in rgba.as_chunks_mut::<4>().0 {
            let (r, g, b, a) = (px[0], px[1], px[2], px[3]);
            if a < 8 {
                continue;
            }
            if r > 80 && b > 80 && g < r.saturating_sub(30) && g < b.saturating_sub(30) {
                px[0] = color.0;
                px[1] = color.1;
                px[2] = color.2;
            }
        }
        let (width, height) = rgba.dimensions();
        let handle = ctx.alloc_image_handle();
        ctx.set_image_rgba8(handle, width, height, rgba.into_raw(), true);
        Some(handle)
    })
}

const MASK_RESOLUTION: u32 = 512;
pub const MASK_WORLD_SIZE: f32 = 1800.0;
const MAX_DARKNESS_ALPHA: f32 = 0.94;
const FEATHER_WORLD_SIZE: f32 = 46.0;

fn bake_mask(ctx: &RenderContext, clear_radius_world: f32) -> ImageHandle {
    let handle = ctx.alloc_image_handle();
    let mut rgba = vec![0u8; (MASK_RESOLUTION * MASK_RESOLUTION * 4) as usize];
    let center = MASK_RESOLUTION as f32 * 0.5;
    let pixels_per_world = MASK_RESOLUTION as f32 / MASK_WORLD_SIZE;
    let radius_pixels = clear_radius_world * pixels_per_world;
    let feather_pixels = FEATHER_WORLD_SIZE * pixels_per_world;
    for y in 0..MASK_RESOLUTION {
        for x in 0..MASK_RESOLUTION {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let distance = (dx * dx + dy * dy).sqrt();
            let darkness = if distance <= radius_pixels {
                0.0
            } else {
                ((distance - radius_pixels) / feather_pixels).clamp(0.0, 1.0) * MAX_DARKNESS_ALPHA
            };
            let index = ((y * MASK_RESOLUTION + x) * 4) as usize;
            rgba[index + 3] = (darkness * 255.0) as u8;
        }
    }
    ctx.set_image_rgba8(handle, MASK_RESOLUTION, MASK_RESOLUTION, rgba, true);
    handle
}
