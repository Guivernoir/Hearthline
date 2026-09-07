#![no_main]

use hearthline_config::ConnectionConfig;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let bounded = &data[..data.len().min(16 * 1_024)];
    if let Ok(source) = core::str::from_utf8(bounded) {
        let _ = ConnectionConfig::from_yaml(source);
    }
});
