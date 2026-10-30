use std::{env, fs::File, path::PathBuf};

use image::{
    codecs::ico::{IcoEncoder, IcoFrame},
    imageops::{crop_imm, resize, FilterType},
    ExtendedColorType, RgbaImage,
};

fn crop_to_visible_pixels(image: RgbaImage) -> RgbaImage {
    let (width, height) = image.dimensions();
    let mut left = width;
    let mut top = height;
    let mut right = 0;
    let mut bottom = 0;

    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel[3] != 0 {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
        }
    }

    if left > right || top > bottom {
        image
    } else {
        crop_imm(&image, left, top, right - left + 1, bottom - top + 1).to_image()
    }
}

fn main() {
    println!("cargo:rerun-if-changed=logo.png");
    if env::var_os("CARGO_CFG_TARGET_OS").as_deref() != Some(std::ffi::OsStr::new("windows")) {
        return;
    }

    let source = image::open("logo.png").expect("logo.png must be a valid PNG").into_rgba8();
    let source = crop_to_visible_pixels(source);
    let sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256];
    let images: Vec<_> = sizes
        .iter()
        .map(|&size| resize(&source, size, size, FilterType::Lanczos3))
        .collect();
    let frames: Vec<_> = images
        .iter()
        .zip(sizes)
        .map(|(image, size)| IcoFrame::as_png(image.as_raw(), size, size, ExtendedColorType::Rgba8).expect("valid icon frame"))
        .collect();

    let icon_path = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set")).join("host-list-maker.ico");
    IcoEncoder::new(File::create(&icon_path).expect("icon file is writable"))
        .encode_images(&frames)
        .expect("icon encoding succeeds");

    let mut resource = winres::WindowsResource::new();
    resource.set_icon(icon_path.to_str().expect("UTF-8 icon path"));
    resource.compile().expect("Windows resource compilation succeeds");
}
