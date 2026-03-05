use std::collections::HashMap;

use home_camera::config::AppConfig;
use proptest::prelude::*;

fn base_env() -> HashMap<String, String> {
    HashMap::from([
        (
            "RTSP_URL".to_string(),
            "rtsp://camera.local/live".to_string(),
        ),
        (
            "RUSTFS_S3_ENDPOINT".to_string(),
            "http://rustfs-svc.rustfs.svc:9000".to_string(),
        ),
        ("RUSTFS_S3_REGION".to_string(), "us-east-1".to_string()),
        ("AWS_ACCESS_KEY_ID".to_string(), "test-access".to_string()),
        (
            "AWS_SECRET_ACCESS_KEY".to_string(),
            "test-secret".to_string(),
        ),
    ])
}

proptest! {
    #[test]
    fn positive_record_seconds_are_accepted(secs in 1u32..86_400u32) {
        let mut vars = base_env();
        vars.insert("RECORD_SECONDS".to_string(), secs.to_string());

        let cfg = AppConfig::from_map(&vars).expect("config should parse");
        prop_assert_eq!(cfg.record_seconds, secs as u64);
    }

    #[test]
    fn non_positive_record_seconds_are_rejected(secs in 0u32..=1u32) {
        let mut vars = base_env();
        vars.insert("RECORD_SECONDS".to_string(), secs.to_string());
        let parsed = AppConfig::from_map(&vars);

        if secs == 0 {
            prop_assert!(parsed.is_err());
        } else {
            prop_assert!(parsed.is_ok());
        }
    }

    #[test]
    fn ffmpeg_crf_in_range_is_accepted(crf in 0u8..=51u8) {
        let mut vars = base_env();
        vars.insert("FFMPEG_CRF".to_string(), crf.to_string());

        let cfg = AppConfig::from_map(&vars).expect("config should parse");
        prop_assert_eq!(cfg.ffmpeg_crf, crf);
    }
}

#[test]
fn missing_required_fields_fail() {
    let required = [
        "RTSP_URL",
        "RUSTFS_S3_ENDPOINT",
        "RUSTFS_S3_REGION",
        "AWS_ACCESS_KEY_ID",
        "AWS_SECRET_ACCESS_KEY",
    ];

    for key in required {
        let mut vars = base_env();
        vars.remove(key);
        assert!(
            AppConfig::from_map(&vars).is_err(),
            "{key} should be required"
        );
    }
}

#[test]
fn ffmpeg_defaults_are_applied() {
    let vars = base_env();
    let cfg = AppConfig::from_map(&vars).expect("config should parse");
    assert_eq!(
        cfg.ffmpeg_video_codec,
        AppConfig::DEFAULT_FFMPEG_VIDEO_CODEC
    );
    assert_eq!(
        cfg.ffmpeg_audio_codec,
        AppConfig::DEFAULT_FFMPEG_AUDIO_CODEC
    );
    assert_eq!(
        cfg.ffmpeg_audio_bitrate,
        AppConfig::DEFAULT_FFMPEG_AUDIO_BITRATE
    );
    assert_eq!(cfg.ffmpeg_loglevel, AppConfig::DEFAULT_FFMPEG_LOGLEVEL);
    assert_eq!(cfg.ffmpeg_preset, AppConfig::DEFAULT_FFMPEG_PRESET);
    assert_eq!(cfg.ffmpeg_crf, AppConfig::DEFAULT_FFMPEG_CRF);
}

#[test]
fn ffmpeg_crf_above_max_is_rejected() {
    let mut vars = base_env();
    vars.insert("FFMPEG_CRF".to_string(), "52".to_string());
    assert!(AppConfig::from_map(&vars).is_err());
}

#[test]
fn empty_new_ffmpeg_settings_are_rejected() {
    for key in [
        "FFMPEG_VIDEO_CODEC",
        "FFMPEG_AUDIO_CODEC",
        "FFMPEG_AUDIO_BITRATE",
        "FFMPEG_LOGLEVEL",
    ] {
        let mut vars = base_env();
        vars.insert(key.to_string(), "   ".to_string());
        assert!(AppConfig::from_map(&vars).is_err(), "{key} should fail");
    }
}
