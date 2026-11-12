use std::{env, fs, path::Path};

fn collect(dir: &Path, root: &Path, out: &Path, entries: &mut String) {
    let mut paths: Vec<_> = fs::read_dir(dir)
        .expect("read frontend directory")
        .map(|entry| entry.expect("frontend entry").path())
        .collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect(&path, root, out, entries);
        } else {
            let name = path
                .strip_prefix(root)
                .expect("asset path")
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = fs::read(&path).expect("read frontend asset");
            let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&bytes, 9);
            let destination = out.join(format!("asset-{}.z", entries.lines().count()));
            fs::write(&destination, compressed).expect("write compressed embedded asset");
            entries.push_str(&format!("({name:?}, include_bytes!({:?})),\n", destination));
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=frontend");
    println!("cargo:rerun-if-changed=resources");
    println!("cargo:rerun-if-changed=THIRD_PARTY_NOTICES.txt");
    let out = env::var_os("OUT_DIR").expect("OUT_DIR");
    let mut entries = String::new();
    collect(
        Path::new("frontend"),
        Path::new("frontend"),
        Path::new(&out),
        &mut entries,
    );
    let notices = fs::read("THIRD_PARTY_NOTICES.txt").expect("third-party notices");
    let notices_path = Path::new(&out).join("notices.z");
    fs::write(
        &notices_path,
        miniz_oxide::deflate::compress_to_vec_zlib(&notices, 9),
    )
    .expect("compress notices");
    entries.push_str(&format!(
        "(\"third-party-notices.txt\", include_bytes!({:?})),\n",
        notices_path
    ));
    fs::write(
        Path::new(&out).join("assets.rs"),
        format!("pub static ASSETS: &[(&str, &[u8])] = &[\n{entries}];\n"),
    )
    .expect("write embedded asset table");
    let mut resource = winresource::WindowsResource::new();
    resource.set_manifest_file("resources\\app.manifest");
    resource.set_icon("resources\\app.ico");
    resource.set("ProductName", "MdViewer");
    resource.set("FileDescription", "View-only Markdown viewer");
    resource
        .compile()
        .expect("Windows SDK resource compiler is required");
}
