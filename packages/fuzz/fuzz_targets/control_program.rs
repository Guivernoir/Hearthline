#![no_main]

use hearthline_sim::{validate_robot_control_program, validate_structured_text_control_program};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let bounded = &data[..data.len().min(16 * 1_024)];
    if let Ok(source) = core::str::from_utf8(bounded) {
        let _ = validate_robot_control_program(source);
        let _ = validate_structured_text_control_program(source);
    }
});
