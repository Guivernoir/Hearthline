#![no_main]

use hearthline_model::{ComponentId, ComponentKind, PortId};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(value) = core::str::from_utf8(data) else {
        return;
    };
    let _ = ComponentId::new(value);
    let _ = PortId::new(value);
    let _ = value.parse::<ComponentKind>();
});
