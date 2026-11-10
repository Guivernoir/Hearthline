use hearthline_sim::physics::clock::{PhysicsPacer, PlaybackRate};

pub fn exercise(input: &[u8]) {
    let step = u64::from(input.first().copied().unwrap_or(0)) + 1;
    let initial = input
        .get(..8)
        .map_or(0, |bytes| u64::from_le_bytes(bytes.try_into().unwrap()));
    let mut clock = PhysicsPacer::at_boundary(step, initial / step * step).unwrap();
    for (index, &byte) in input.iter().take(256).enumerate() {
        let before = clock;
        let result = match byte % 9 {
            0 => {
                clock.set_paused(true);
                Ok(())
            }
            1 => {
                clock.set_paused(false);
                Ok(())
            }
            2 => {
                clock.set_headless();
                Ok(())
            }
            3 => {
                clock.set_rate(
                    [
                        PlaybackRate::One,
                        PlaybackRate::Two,
                        PlaybackRate::Five,
                        PlaybackRate::Ten,
                    ][usize::from(byte / 9) % 4],
                );
                Ok(())
            }
            4 => clock.request_single_step(),
            5 => clock.commit_step(),
            6 => clock.accrue(u64::MAX),
            _ => clock.accrue(u64::from(byte) * (index as u64 + 1)),
        };
        if result.is_err() {
            assert_eq!(clock, before);
        }
        assert!(clock.completed_us() >= before.completed_us());
        assert_eq!(clock.wait_us() == Some(0), clock.step_due());
        if byte % 9 == 5 && result.is_ok() {
            assert_eq!(clock.completed_us() - before.completed_us(), step);
            assert_eq!(
                clock.automatic_completed_us() - before.automatic_completed_us(),
                if before.paused() { 0 } else { step }
            );
        }
    }
}
