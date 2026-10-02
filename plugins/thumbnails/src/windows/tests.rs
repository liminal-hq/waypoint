// Live tests of the shell thumbnails, run by hand on Windows: `cargo test -p tauri-plugin-thumbnails -- --ignored live_`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

/// Asks the real shell for the thumbnail of a PNG written to a temporary folder, then checks the pixels and the PNG round trip. Needs a desktop session with the shell's thumbnail handlers (any Windows 11 install).
#[test]
#[ignore = "talks to the real Windows shell: run by hand"]
fn live_shell_thumbnail() {
    let tmp = std::env::temp_dir().join(format!("thumbnails-live-{}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    let path = tmp.join("sample.png");
    let image = image::RgbImage::from_pixel(640, 480, image::Rgb([200, 40, 40]));
    image.save(&path).unwrap();

    let guard = ShellProcessor {
        store: Arc::new(Store::new(
            tmp.join("cache"),
            "live",
            "0",
            Arc::new(file_uri),
        )),
        mem: Arc::new(MemCache::new(1 << 20)),
    }
    .on_worker_start();
    let rendered =
        live_thumbnail(&path.to_string_lossy(), 256).expect("the shell makes a thumbnail");
    drop(guard);
    assert!(rendered.width <= 256 && rendered.height <= 256);
    assert!(rendered.width == 256 || rendered.height == 256);
    // The middle pixel is the picture's red, give or take colour management.
    let at = ((rendered.height / 2 * rendered.width + rendered.width / 2)
        * if rendered.color == png::ColorType::Rgba {
            4
        } else {
            3
        }) as usize;
    assert!(
        rendered.pixels[at] > 150,
        "red channel was {}",
        rendered.pixels[at]
    );
    let _ = fs::remove_dir_all(&tmp);
}
