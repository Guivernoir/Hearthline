# Golden Replays

These compact replay artifacts bind representative simulation outcomes to an
immutable model digest. Each artifact records ordered inputs and full normalized
initial and final cell snapshots; component and field digests make divergence
reporting precise. CI reruns each scenario and reports the first divergent
checkpoint, component, field, or final outcome. Event-by-event traces remain CI
artifacts and are not committed here.
