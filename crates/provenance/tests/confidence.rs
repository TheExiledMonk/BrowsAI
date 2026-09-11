use browsai_provenance::Confidence;

#[test]
fn confidence_is_bounded_and_marks_inference() {
    assert_eq!(Confidence::new(2.0).0, 1.0);
    assert_eq!(Confidence::new(-1.0).0, 0.0);
    assert!(!Confidence::DIRECT.is_inferred());
    assert!(Confidence::PROBABLE.is_inferred());
}

#[test]
fn confidence_normalizes_nan_to_a_bounded_value() {
    let confidence = Confidence::new(f32::NAN);
    assert_eq!(confidence, Confidence(0.0));
    assert!(confidence.0.is_finite());
}
