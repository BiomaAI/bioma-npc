use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use ggez::graphics::{Canvas, Color, Image};
use ggez::Context;
use image::{ImageBuffer, Rgba};

use crate::{config, output_path, PreWorldHookArgs, PreWorldHookFn, WorldGlobalState};

pub(crate) fn render_rgba_image(
    ctx: &mut Context,
    clear: Color,
    draw: impl FnOnce(&Context, &mut Canvas),
) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let (width, height) = ctx.gfx.drawable_size();
    let width = width.round().max(1.0) as u32;
    let height = height.round().max(1.0) as u32;
    let padded_width = width.div_ceil(64) * 64;

    ctx.gfx.begin_frame().unwrap();

    let image = Image::new_canvas_image(ctx, ctx.gfx.surface_format(), padded_width, height, 1);
    let mut canvas = Canvas::from_image(ctx, image.clone(), clear);
    draw(&*ctx, &mut canvas);
    canvas.finish(ctx).unwrap();
    ctx.gfx.end_frame().unwrap();

    let padded_image_data: ImageBuffer<Rgba<u8>, _> =
        ImageBuffer::from_raw(padded_width, height, image.to_pixels(ctx).unwrap()).unwrap();
    image::imageops::crop_imm(&padded_image_data, 0, 0, width, height).to_image()
}

pub fn screenshot(
    ctx: &mut Context,
    world: &WorldGlobalState,
    assets: &BTreeMap<String, Image>,
    path: &str,
) {
    let flipped_image_data = render_rgba_image(
        ctx,
        Color::new(
            config().display.background.0,
            config().display.background.1,
            config().display.background.2,
            1.0,
        ),
        |ctx, canvas| world.draw(ctx, canvas, assets),
    );

    let dir = {
        let mut path = PathBuf::from(path);
        path.pop();
        path.to_str().unwrap().to_owned()
    };
    fs::create_dir_all(dir).unwrap();

    flipped_image_data.save(path).unwrap();
}

pub fn screenshot_hook() -> PreWorldHookFn {
    Box::new(
        |PreWorldHookArgs {
             world,
             ctx,
             assets,
             run,
             turn,
             ..
         }| {
            if let Some(ctx) = ctx.as_deref_mut() {
                screenshot(
                    ctx,
                    world,
                    assets,
                    &format!(
                        "{}/{}/screenshots/turn{:06}.png",
                        output_path(),
                        run.map(|n| n.to_string()).unwrap_or_default(),
                        turn,
                    ),
                );
            }
        },
    )
}
