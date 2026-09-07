#![no_main]

use hearthline_sim::ReplayArtifact;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let bounded = &data[..data.len().min(256 * 1_024)];
    if let Ok(artifact) = serde_json::from_slice::<ReplayArtifact>(bounded) {
        let _ = artifact.normalize();
    }
});
