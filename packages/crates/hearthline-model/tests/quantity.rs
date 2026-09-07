use hearthline_model::{
    Angle, FixedValue, Flow, Mass, Percentage, Position, Pressure, QuantityError, Temperature,
    fixed,
};

#[test]
fn fixed_quantities_preserve_exact_domain_resolution() {
    let pressure = Pressure::from_raw(4_500);
    let temperature = Temperature::from_raw(40_000);
    let flow = Flow::from_raw(12_500);
    let mass = Mass::from_raw(800_000);
    let position = Position::from_raw(1_250_000);
    let angle = Angle::from_raw(45_000);
    let percentage = Percentage::from_raw(7_500);

    assert_eq!(pressure.raw(), 4_500);
    assert_eq!(temperature.raw(), 40_000);
    assert_eq!(flow.raw(), 12_500);
    assert_eq!(mass.raw(), 800_000);
    assert_eq!(position.raw(), 1_250_000);
    assert_eq!(angle.raw(), 45_000);
    assert_eq!(percentage.raw(), 7_500);
}

#[test]
fn fixed_quantities_report_overflow_and_saturate_explicitly() {
    assert!(
        Pressure::from_raw(i64::MAX)
            .checked_add(Pressure::from_raw(1))
            .is_err()
    );
    assert_eq!(
        Pressure::from_raw(i64::MAX)
            .saturating_add(Pressure::from_raw(1))
            .raw(),
        i64::MAX
    );
}

#[test]
fn fixed_literals_and_arithmetic_are_quantized_and_saturating() {
    const VALUE: FixedValue = fixed!(12.345678);
    const NEGATIVE: FixedValue = fixed!(-0.5);
    assert_eq!(VALUE.raw(), 12_345_678);
    assert_eq!(NEGATIVE.raw(), -500_000);
    assert_eq!((fixed!(2.5) * fixed!(4.0)).raw(), 10_000_000);
    assert_eq!((fixed!(10.0) / fixed!(4.0)).raw(), 2_500_000);
    assert_eq!((fixed!(2.0) + fixed!(0.75) - fixed!(0.5)).raw(), 2_250_000);
}

#[test]
fn fixed_point_display_preserves_the_sign_and_canonical_resolution() {
    assert_eq!(fixed!(-1.25).to_string(), "-1.250000");
    assert_eq!(fixed!(0).to_string(), "0.000000");
}

#[test]
fn fixed_literal_parser_rejects_every_ambiguous_form() {
    assert_eq!(
        FixedValue::from_decimal_literal("1_000.25").raw(),
        1_000_250_000
    );
    assert_eq!(FixedValue::from_decimal_literal("-2").raw(), -2_000_000);
    for invalid in ["", "-", ".", "1.2.3", "1x", "1.0000001"] {
        assert!(
            std::panic::catch_unwind(|| FixedValue::from_decimal_literal(invalid)).is_err(),
            "{invalid} must not be accepted"
        );
    }
}

#[test]
fn ratios_rounding_clamping_and_saturation_cover_signed_boundaries() {
    assert_eq!(
        FixedValue::from_ratio(1, 0),
        Err(QuantityError::InvalidScale)
    );
    assert_eq!(
        FixedValue::from_ratio(i64::MAX, 1),
        Err(QuantityError::Overflow)
    );
    assert_eq!(
        FixedValue::from_ratio(i64::MIN, 1),
        Err(QuantityError::Overflow)
    );
    assert_eq!(FixedValue::from_ratio(1, 4).unwrap().raw(), 250_000);

    assert_eq!(FixedValue::from_u64_ratio(1, 0), FixedValue::ZERO);
    assert_eq!(FixedValue::from_u64_ratio(u64::MAX, 1).raw(), i64::MAX);
    assert_eq!(FixedValue::from_u64_ratio(1, 2).raw(), 500_000);
    assert_eq!(FixedValue::from_raw(-1).round_to_u64(), 0);
    assert_eq!(fixed!(1.6).round_to_u64(), 2);

    assert_eq!(
        FixedValue::from_raw(i64::MAX)
            .saturating_mul(fixed!(2))
            .raw(),
        i64::MAX
    );
    assert_eq!(
        FixedValue::from_raw(i64::MIN)
            .saturating_mul(fixed!(2))
            .raw(),
        i64::MIN
    );
    assert_eq!(fixed!(3).saturating_mul(fixed!(2)).raw(), 6_000_000);

    assert_eq!(
        fixed!(1).checked_div(FixedValue::ZERO),
        Err(QuantityError::InvalidScale)
    );
    assert_eq!(
        FixedValue::from_raw(i64::MAX).checked_div(FixedValue::from_raw(1)),
        Err(QuantityError::Overflow)
    );
    assert_eq!(
        FixedValue::from_raw(i64::MIN)
            .saturating_div(FixedValue::from_raw(1))
            .raw(),
        i64::MIN
    );
    assert_eq!(
        FixedValue::from_raw(i64::MAX)
            .saturating_div(FixedValue::from_raw(1))
            .raw(),
        i64::MAX
    );
    assert_eq!(fixed!(1).saturating_div(FixedValue::ZERO), FixedValue::ZERO);

    assert_eq!(fixed!(-1).clamp(fixed!(0), fixed!(10)), fixed!(0));
    assert_eq!(fixed!(11).clamp(fixed!(0), fixed!(10)), fixed!(10));
    assert_eq!(fixed!(5).clamp(fixed!(0), fixed!(10)), fixed!(5));
}

#[test]
fn runtime_decimal_parsing_and_typed_quantities_cover_all_error_paths() {
    assert_eq!("12".parse::<FixedValue>().unwrap().raw(), 12_000_000);
    assert_eq!("-1.25".parse::<FixedValue>().unwrap().raw(), -1_250_000);
    for invalid in [".1", "1.0000001", "1.a", "word"] {
        assert_eq!(
            invalid.parse::<FixedValue>(),
            Err(QuantityError::InvalidDecimal)
        );
    }
    assert_eq!(
        "9223372036854775807".parse::<FixedValue>(),
        Err(QuantityError::Overflow)
    );

    let one = Pressure::from_raw(1);
    assert_eq!(one.checked_add(Pressure::from_raw(2)).unwrap().raw(), 3);
    assert_eq!(one.checked_sub(Pressure::from_raw(2)).unwrap().raw(), -1);
    assert_eq!(
        Pressure::from_raw(i64::MIN).checked_sub(one),
        Err(QuantityError::Overflow)
    );
    assert_eq!(
        Pressure::from_raw(-1)
            .clamp(Pressure::from_raw(0), Pressure::from_raw(10))
            .raw(),
        0
    );
    assert_eq!(
        Pressure::from_raw(11)
            .clamp(Pressure::from_raw(0), Pressure::from_raw(10))
            .raw(),
        10
    );
    assert_eq!(
        Pressure::from_raw(5)
            .clamp(Pressure::from_raw(0), Pressure::from_raw(10))
            .raw(),
        5
    );
    assert_eq!(format!("{}", FixedValue::from_raw(-1_250_000)), "-1.250000");
}
