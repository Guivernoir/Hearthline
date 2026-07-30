#![no_main]

use hearthline_config::ApplianceConfig;
use libfuzzer_sys::fuzz_target;

const INPUT_LIMIT: usize = 16 * 1_024;

fuzz_target!(|data: &[u8]| {
    let bounded = &data[..data.len().min(INPUT_LIMIT)];
    let Ok(source) = core::str::from_utf8(bounded) else {
        return;
    };
    let _ = ApplianceConfig::from_yaml(source);
});
