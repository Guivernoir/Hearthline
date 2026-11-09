use hearthline_physics::material::caloric::{CaloricKnot, CaloricLaw};
use hearthline_physics::{PhysicsError, Scalar};

fn s(value: &str) -> Scalar {
    value.parse().unwrap()
}

#[test]
fn linear_capacity_integrates_to_its_quadratic_primitive_across_knots() {
    let points = [[s("0"), s("2")], [s("1"), s("5")], [s("2"), s("8")]];
    let mut compiled = [CaloricKnot::default(); 3];
    let measured = allocation_counter::measure(|| {
        let law = CaloricLaw::compile(&points, s("-1"), &mut compiled).unwrap();
        for (temperature, capacity, enthalpy) in [
            ("0", "2", "-1"),
            ("0.5", "3.5", "0.375"),
            ("1", "5", "2.5"),
            ("1.5", "6.5", "5.375"),
            ("2", "8", "9"),
        ] {
            assert_eq!(law.heat_capacity(s(temperature)).unwrap(), s(capacity));
            assert_eq!(law.enthalpy(s(temperature)).unwrap(), s(enthalpy));
        }
        assert_eq!(law.domain(), [s("0"), s("2")]);
        assert!(law.enthalpy(s("-0.000000001")).is_err());
        assert!(law.heat_capacity(s("2.000000001")).is_err());
    });
    assert_eq!(measured.count_total, 0);
    assert_eq!(compiled[1].enthalpy, s("2.5"));
    let mut shifted = [CaloricKnot::default(); 3];
    let shifted = CaloricLaw::compile(&points, s("1234"), &mut shifted).unwrap();
    let original = CaloricLaw::new(&compiled).unwrap();
    assert_eq!(
        shifted
            .enthalpy(s("1.2345"))
            .unwrap()
            .checked_sub(shifted.enthalpy(s("0.1234")).unwrap()),
        original
            .enthalpy(s("1.2345"))
            .unwrap()
            .checked_sub(original.enthalpy(s("0.1234")).unwrap()),
    );
}

#[test]
fn decreasing_heat_capacity_still_has_monotone_rounded_enthalpy() {
    let points = [
        [s("0"), s("1")],
        [Scalar::from_raw(1000), Scalar::from_raw(1)],
    ];
    let mut compiled = [CaloricKnot::default(); 2];
    let law = CaloricLaw::compile(&points, Scalar::ZERO, &mut compiled).unwrap();
    let mut previous = Scalar::ZERO;
    for raw in 0..=1000 {
        let enthalpy = law.enthalpy(Scalar::from_raw(raw)).unwrap();
        assert!(enthalpy >= previous, "nonmonotone enthalpy at {raw}");
        previous = enthalpy;
    }
    assert_eq!(previous, Scalar::from_raw(500));
}

#[test]
fn caloric_validation_rejects_unsafe_domains_prefixes_and_unresolved_energy() {
    let mut output = [CaloricKnot::default(); 2];
    for points in [
        [[s("1"), s("1")], [s("1"), s("2")]],
        [[s("2"), s("1")], [s("1"), s("2")]],
        [[s("0"), s("0")], [s("1"), s("1")]],
        [[s("0"), s("1")], [s("1"), s("-1")]],
    ] {
        assert!(CaloricLaw::compile(&points, Scalar::ZERO, &mut output).is_err());
    }
    assert!(CaloricLaw::compile(&[], Scalar::ZERO, &mut []).is_err());
    assert_eq!(
        CaloricLaw::compile(&[[s("0"), s("1")]], Scalar::ZERO, &mut output).unwrap_err(),
        PhysicsError::BufferLength
    );
    let unresolved = [
        [Scalar::ZERO, Scalar::from_raw(1)],
        [Scalar::from_raw(1), Scalar::from_raw(1)],
    ];
    assert_eq!(
        CaloricLaw::compile(&unresolved, Scalar::ZERO, &mut output).unwrap_err(),
        PhysicsError::InsufficientResolution
    );
    let normal = [[s("0"), s("1")], [s("1"), s("2")]];
    CaloricLaw::compile(&normal, Scalar::ZERO, &mut output).unwrap();
    output[1].enthalpy = output[1].enthalpy.checked_add(Scalar::from_raw(1)).unwrap();
    assert_eq!(
        CaloricLaw::new(&output).unwrap_err(),
        PhysicsError::InvalidCoefficient
    );
    let overflow = [
        [s("0"), s("1000000000")],
        [s("1000000000"), s("1000000000")],
    ];
    assert_eq!(
        CaloricLaw::compile(&overflow, Scalar::ZERO, &mut output).unwrap_err(),
        PhysicsError::Overflow
    );
}

#[test]
fn large_constant_scales_avoid_cubic_intermediates_and_variable_overflow_is_explicit() {
    let mut output = [CaloricKnot::default(); 2];
    let points = [[s("0"), s("0.000001")], [s("1000000000"), s("0.000001")]];
    let law = CaloricLaw::compile(&points, Scalar::ZERO, &mut output).unwrap();
    assert_eq!(law.enthalpy(s("500000000")).unwrap(), s("500"));
    let points = [[s("0"), s("0.000001")], [s("1000000000"), s("0.000002")]];
    let law = CaloricLaw::compile(&points, Scalar::ZERO, &mut output).unwrap();
    assert_eq!(law.enthalpy(s("500000000")), Err(PhysicsError::Overflow));
}
