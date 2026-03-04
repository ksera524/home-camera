use home_camera::retry::RetryPolicy;
use proptest::prelude::*;

proptest! {
    #[test]
    fn delay_is_monotonic(
        a in 0u32..20,
        b in 0u32..20,
        base in 1u64..1000,
        cap in 1000u64..20000
    ) {
        let policy = RetryPolicy {
            max_retries: 5,
            base_delay_ms: base,
            max_delay_ms: cap,
        };

        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let d1 = policy.delay_for_attempt(lo);
        let d2 = policy.delay_for_attempt(hi);

        prop_assert!(d1 <= d2);
        prop_assert!(d2.as_millis() <= cap as u128);
    }
}

#[test]
fn first_delay_is_base_delay() {
    let policy = RetryPolicy::default();
    assert_eq!(
        policy.delay_for_attempt(0).as_millis(),
        policy.base_delay_ms as u128
    );
}
