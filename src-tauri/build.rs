// SPDX-License-Identifier: BUSL-1.1

fn main() {
    let stable = "https://github.com/postlane/desktop/releases/latest/download/latest.json";
    let endpoint = std::env::var("TAURI_UPDATER_ENDPOINT").unwrap_or_else(|_| stable.to_owned());
    assert!(
        endpoint.starts_with("https://"),
        "TAURI_UPDATER_ENDPOINT must use https://, got: {endpoint}",
    );
    println!("cargo:rustc-env=UPDATER_ENDPOINT={endpoint}");
    println!("cargo:rerun-if-env-changed=TAURI_UPDATER_ENDPOINT");

    // When building for the beta channel, override the updater endpoint that
    // tauri::generate_context!() embeds in the binary via json-patch merge.
    if endpoint != stable {
        let config = format!(
            r#"{{"plugins":{{"updater":{{"endpoints":["{endpoint}"]}}}}}}"#
        );
        println!("cargo:rustc-env=TAURI_CONFIG={config}");
    }

    tauri_build::build();
}
