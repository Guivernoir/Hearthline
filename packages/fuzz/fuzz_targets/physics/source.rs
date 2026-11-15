use hearthline_config::physics::connected::ConnectedGasDocument;
use hearthline_project::physics::flow::connected::CompiledConnectedGas;

pub fn exercise(input: &[u8]) {
    let mut generated;
    let bytes = match input.first() {
        Some(b'G') => &input[1..input.len().min(65_537)],
        Some(b'H') => {
            generated = include_bytes!("../../../../project/physics/runtime/connected-source.yaml")
                .to_vec();
            if let Some(change) = input.get(1..4) {
                let index =
                    (usize::from(change[0]) * 256 + usize::from(change[1])) % generated.len();
                generated[index] ^= change[2];
            }
            &generated
        }
        _ => return,
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return;
    };
    let Ok(mut document) = ConnectedGasDocument::from_yaml(text) else {
        return;
    };
    // Fuzz compilation, not large allocations or arbitrarily long simulated runs.
    document.allocation_budget_bytes = document.allocation_budget_bytes.min(128 * 1024);
    let serialized = serde_json::to_string(&document).unwrap();
    let roundtrip = ConnectedGasDocument::from_yaml(&serialized).unwrap();
    assert_eq!(document, roundtrip);
    let result = CompiledConnectedGas::compile_document(&document, 128 * 1024);
    let mut reordered = roundtrip;
    reordered.regions.reverse();
    reordered.connections.reverse();
    let reversed = CompiledConnectedGas::compile_document(&reordered, 128 * 1024);
    match (result, reversed) {
        (Ok(left), Ok(right)) => assert_eq!(left.digest(), right.digest()),
        (Err(_), Err(_)) => {}
        _ => panic!("source order changed connected compilation validity"),
    }
}
