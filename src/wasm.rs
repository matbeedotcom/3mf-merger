#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use crate::merge::{merge_files_wasm, MergeOptions};

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct WasmMergeOptions {
    inner: MergeOptions,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl WasmMergeOptions {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmMergeOptions {
        let mut inner = MergeOptions::default();
        // Match native CLI defaults: don't merge printer/filament by default
        // This avoids "custom printer settings dialog" in Bambu Studio
        inner.force = true;
        inner.keep_first_printer = true;
        inner.keep_first_filament = true;
        inner.merge_filament = false;  // default: false (match native CLI)
        inner.merge_printer = false;   // default: false (match native CLI)
        WasmMergeOptions { inner }
    }

    #[wasm_bindgen(getter = dedupe_filaments)]
    pub fn dedupe_filaments_get(&self) -> bool {
        self.inner.dedupe_filaments
    }

    #[wasm_bindgen(setter = dedupe_filaments)]
    pub fn dedupe_filaments_set(&mut self, value: bool) {
        self.inner.dedupe_filaments = value;
    }

    #[wasm_bindgen(getter = merge_filament)]
    pub fn merge_filament_get(&self) -> bool {
        self.inner.merge_filament
    }

    #[wasm_bindgen(setter = merge_filament)]
    pub fn merge_filament_set(&mut self, value: bool) {
        self.inner.merge_filament = value;
    }

    #[wasm_bindgen(getter = merge_printer)]
    pub fn merge_printer_get(&self) -> bool {
        self.inner.merge_printer
    }

    #[wasm_bindgen(setter = merge_printer)]
    pub fn merge_printer_set(&mut self, value: bool) {
        self.inner.merge_printer = value;
    }
}

#[cfg(target_arch = "wasm32")]
impl Default for WasmMergeOptions {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn merge_files(
    input_files: js_sys::Array,
    options: &WasmMergeOptions,
) -> Result<js_sys::Uint8Array, JsValue> {
    console_error_panic_hook::set_once();

    let mut inputs: Vec<Vec<u8>> = Vec::new();
    for i in 0..input_files.length() {
        let item = input_files.get(i);
        if let Ok(arr) = item.dyn_into::<js_sys::Uint8Array>() {
            inputs.push(arr.to_vec());
        } else {
            return Err(JsValue::from_str("All input files must be Uint8Array"));
        }
    }

    let result = merge_files_wasm(&inputs, &options.inner)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    Ok(js_sys::Uint8Array::from(&result[..]))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}