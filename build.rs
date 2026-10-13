#[path = "src/icon.rs"]
mod icon;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/icon.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let path =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("build output directory"))
            .join("repostatus.ico");
    let mut directory = ico::IconDir::new(ico::ResourceType::Icon);
    for size in [16, 32, 48, 64, 128, 256] {
        let image = ico::IconImage::from_rgba_data(size, size, icon::rgba(size, false));
        directory.add_entry(ico::IconDirEntry::encode(&image).expect("encode app icon"));
    }
    directory
        .write(std::fs::File::create(&path).expect("create app icon"))
        .expect("write app icon");

    winresource::WindowsResource::new()
        .set_icon(path.to_str().expect("app icon path"))
        .set("ProductName", "RepoStatus")
        .set("FileDescription", "RepoStatus · Git Projects")
        .compile()
        .expect("compile Windows app resources");
}
