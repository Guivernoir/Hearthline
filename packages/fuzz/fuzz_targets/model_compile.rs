#![no_main]

use hearthline_project::BlueprintRepository;
use libfuzzer_sys::fuzz_target;

const SEPARATOR: &str = "\n---INSTANCE---\n";

fuzz_target!(|data: &[u8]| {
    let bounded = &data[..data.len().min(64 * 1_024)];
    let Ok(source) = core::str::from_utf8(bounded) else {
        return;
    };
    let Some((blueprint, instance)) = source.split_once(SEPARATOR) else {
        return;
    };
    if let Ok(repository) = BlueprintRepository::from_sources([blueprint], [instance]) {
        let _ = repository.expand_all();
    }
});
