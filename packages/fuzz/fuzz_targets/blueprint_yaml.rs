#![no_main]

use hearthline_project::BlueprintDefinition;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let bounded = &data[..data.len().min(32 * 1_024)];
    if let Ok(source) = core::str::from_utf8(bounded) {
        let _ = BlueprintDefinition::from_yaml(source);
    }
});
