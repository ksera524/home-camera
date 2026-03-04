use home_camera::key::{build_object_key, sanitize_camera_id};
use proptest::prelude::*;
use time::OffsetDateTime;

proptest! {
    #[test]
    fn object_key_has_expected_shape(
        ts in -2_000_000_000i64..4_000_000_000i64,
        camera in "[a-zA-Z0-9_\\-/ ]{1,24}"
    ) {
        let dt = OffsetDateTime::from_unix_timestamp(ts).unwrap();
        let key = build_object_key(&camera, dt);
        prop_assert!(key.ends_with(".mp4"));
        prop_assert_eq!(key.matches('/').count(), 4);
    }

    #[test]
    fn camera_id_is_s3_key_safe(input in ".{0,32}") {
        let out = sanitize_camera_id(&input);
        prop_assert!(!out.is_empty());
        prop_assert!(!out.contains('/'));
        prop_assert!(out.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }
}
