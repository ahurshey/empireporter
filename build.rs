// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 Alex Hurshman
//
// This file is part of EmpirePorter.
//
// EmpirePorter is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 only.
//
// EmpirePorter is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.

use std::env;
use std::fs::File;
use std::path::{Path, PathBuf};

const ICON_PNG_PATH: &str = "assets/app_icon.png";

fn main() {
    println!("cargo:rerun-if-changed={ICON_PNG_PATH}");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR should be set"));
    let icon_path = out_dir.join("app_icon.ico");

    generate_icon_file(Path::new(ICON_PNG_PATH), &icon_path);

    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon(&icon_path.to_string_lossy())
        .set(
            "FileDescription",
            "EmpirePorter - Stellaris Empire Import/Export",
        )
        .set("ProductName", "EmpirePorter")
        .set("LegalCopyright", "Copyright (c) 2026 Alex Hurshman");

    resource
        .compile()
        .expect("failed to embed Windows executable resources");
}

fn generate_icon_file(source_png: &Path, output_ico: &Path) {
    let source = image::open(source_png)
        .expect("failed to open assets/app_icon.png")
        .into_rgba8();
    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);

    for size in [16, 24, 32, 48, 64, 128, 256] {
        let resized =
            image::imageops::resize(&source, size, size, image::imageops::FilterType::Lanczos3);
        let icon_image = ico::IconImage::from_rgba_data(size, size, resized.into_raw());
        let entry =
            ico::IconDirEntry::encode(&icon_image).expect("failed to encode generated icon image");
        icon_dir.add_entry(entry);
    }

    let file = File::create(output_ico).expect("failed to create generated app_icon.ico");
    icon_dir
        .write(file)
        .expect("failed to write generated app_icon.ico");
}
