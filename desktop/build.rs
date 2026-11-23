fn main() {
    // The vendored translations use plain source strings as msgids. Bundle them
    // in both generated units so App language and Follow system are real choices.
    let config = slint_build::CompilerConfiguration::new()
        .with_bundled_translations(concat!(env!("CARGO_MANIFEST_DIR"), "/translations"))
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    // The tray is compiled as its own Slint unit: it exports a `SystemTrayIcon`
    // component rather than a window, so Slint creates no window adapter for it
    // and it can be created before the event loop runs. `src/tray.rs` includes
    // the generated file; the window UI keeps the default `include_modules!()`
    // output of the app compilation below.
    let tray_output = std::path::PathBuf::from(
        std::env::var_os("OUT_DIR").expect("cargo runs the build script with OUT_DIR"),
    )
    .join("tray.rs");
    let tray_dependencies = slint_build::compile_with_output_path(
        concat!(env!("CARGO_MANIFEST_DIR"), "/ui/tray.slint"),
        tray_output,
        config.clone(),
    )
    .expect("compile the tray icon");
    // `compile_with_output_path` prints no cargo messages of its own, so the
    // icon asset and the tray's imports have to be named here or a change to
    // them would not rerun this script.
    for path in tray_dependencies {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    slint_build::compile_with_config("ui/app.slint", config).expect("compile OpenSCL4 UI");
    println!("cargo:rerun-if-changed=translations");
}
