#[cfg(all(target_arch = "wasm32", feature = "wasm-test"))]
use wasm_bindgen_test::*;

#[cfg(all(target_arch = "wasm32", feature = "wasm-test"))]
wasm_bindgen_test_configure!(run_in_browser);

#[cfg(all(target_arch = "wasm32", feature = "wasm-test"))]
mod wasm_tests {
    use super::*;
    use crate::merge::merge_files_wasm;
    use crate::merge::MergeOptions;

    #[wasm_bindgen_test]
    fn wasm_merge_two_files() {
        // Load test fixtures as bytes
        let yoshi_bytes = include_bytes!("../../Yoshi.3mf");
        let mario_bytes = include_bytes!("../../Mario.3mf");

        let options = MergeOptions {
            force: true,
            printer_preset: false,
            color_presets: false,
            keep_first_printer: true,
            keep_first_filament: true,
            merge_filament: true,
            merge_printer: true,
            dedupe_filaments: false,
        };

        let result = merge_files_wasm(&[yoshi_bytes.to_vec(), mario_bytes.to_vec()], &options);
        assert!(result.is_ok(), "Merge failed: {:?}", result.err());

        let merged = result.unwrap();
        assert!(!merged.is_empty(), "Merged output is empty");

        // Verify it's a valid ZIP by checking magic bytes
        assert_eq!(&merged[0..4], b"PK\x03\x04", "Output is not a valid ZIP");
    }

    #[wasm_bindgen_test]
    fn wasm_merge_with_dedupe() {
        let yoshi_bytes = include_bytes!("../../Yoshi.3mf");

        let options = MergeOptions {
            force: true,
            printer_preset: false,
            color_presets: false,
            keep_first_printer: true,
            keep_first_filament: true,
            merge_filament: true,
            merge_printer: true,
            dedupe_filaments: true,
        };

        let result = merge_files_wasm(&[yoshi_bytes.to_vec(), yoshi_bytes.to_vec()], &options);
        assert!(result.is_ok(), "Merge failed: {:?}", result.err());

        let merged = result.unwrap();
        assert!(!merged.is_empty(), "Merged output is empty");
        assert_eq!(&merged[0..4], b"PK\x03\x04", "Output is not a valid ZIP");
    }

    #[wasm_bindgen_test]
    fn wasm_merge_rejects_too_few() {
        let yoshi_bytes = include_bytes!("../../Yoshi.3mf");

        let options = MergeOptions::default();
        let result = merge_files_wasm(&[yoshi_bytes.to_vec()], &options);
        assert!(result.is_err(), "Expected error for single input");
    }
}