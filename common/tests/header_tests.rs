use screamwire_common::scream::{channel_map, make_header, parse_header};
use screamwire_common::types::AudioParams;

/// Helpers

/// Test `make_header`` function
macro_rules! test_make_header {
    ($($name:ident: rate=$rate:expr, bits=$bits:expr, channels=$channels:expr => $expected:expr;)*) => {
        $(
            #[test]
            fn $name() {
                let format = AudioParams {
                    rate: $rate,
                    bits: $bits,
                    channels: $channels,
                };
                let header = make_header(format);
                assert_eq!(
                    header,
                    $expected,
                    "Failed header generation for rate={}, bits={}, channels={}",
                    $rate, $bits, $channels
                );
            }
        )*
    };
}

/// Test `parse_header` function
macro_rules! test_parse_header {
    ($($name:ident: rate=$rate:expr, bits=$bits:expr, channels=$channels:expr;)*) => {
        $(
            #[test]
            fn $name() {
                let header = make_header(AudioParams {
                    rate: $rate,
                    bits: $bits,
                    channels: $channels,
                });
                let params = parse_header(&header).unwrap_or_else(|| {
                    panic!(
                        "Failed to parse header with params [rate: {}, bits: {}, channels: {}]",
                        $rate, $bits, $channels
                    )
                });

                assert_eq!(params.rate, $rate);
                assert_eq!(params.bits, $bits);
                assert_eq!(params.channels, $channels);
            }
        )*
    };
}

/// Tests
///

// make_header tests
test_make_header! {
    test_standard_header: rate=48000,  bits=16, channels=2 => [0x01, 0x10, 0x02, 0x03, 0x00];
    test_44100_stereo:    rate=44100,  bits=16, channels=2 => [0x81, 0x10, 0x02, 0x03, 0x00];
    test_96000_stereo:    rate=96000,  bits=16, channels=2 => [0x02, 0x10, 0x02, 0x03, 0x00];
    test_88200_stereo:    rate=88200,  bits=16, channels=2 => [0x82, 0x10, 0x02, 0x03, 0x00];
    test_192000_stereo:   rate=192000, bits=16, channels=2 => [0x04, 0x10, 0x02, 0x03, 0x00];
    test_176400_stereo:   rate=176400, bits=16, channels=2 => [0x84, 0x10, 0x02, 0x03, 0x00];
    test_48000_mono:      rate=48000,  bits=16, channels=1 => [0x01, 0x10, 0x01, 0x01, 0x00];
    test_44100_24bit:     rate=44100,  bits=24, channels=2 => [0x81, 0x18, 0x02, 0x03, 0x00];
    test_48000_32bit:     rate=48000,  bits=32, channels=2 => [0x01, 0x20, 0x02, 0x03, 0x00];
    test_channel_map_6ch_in_header:     rate=48000,  bits=16, channels=6 => [0x01, 0x10, 0x06, 0x0F, 0x06];
}

// parse_header tests
test_parse_header! {
    test_parse_16bit:        rate=48000, bits=24, channels=2;
    test_parse_24bit:        rate=48000, bits=24, channels=2;
    test_parse_32bit:        rate=48000, bits=32, channels=2;
    test_parse_stereo_44_1k: rate=44100, bits=16, channels=2;
    test_parse_hi_res_192k:  rate=192000,bits=24, channels=2;
}

#[test]
fn test_channel_map_1ch() {
    assert_eq!(channel_map(1), 0x0001);
}

#[test]
fn test_channel_map_2ch() {
    assert_eq!(channel_map(2), 0x0003);
}

#[test]
fn test_channel_map_6ch() {
    assert_eq!(channel_map(6), 0x060F);
}

#[test]
fn test_channel_map_8ch() {
    assert_eq!(channel_map(8), 0x00FF);
}

#[test]
fn test_channel_map_4ch_quad() {
    assert_eq!(channel_map(4), 0x0033);
}

#[test]
fn test_channel_map_custom() {
    assert_eq!(channel_map(3), 0x0007);
}

#[test]
fn test_frame_bytes() {
    let format = AudioParams {
        rate: 48000,
        bits: 16,
        channels: 2,
    };
    assert_eq!(format.frame_bytes(), 4);
    let format = AudioParams {
        rate: 48000,
        bits: 24,
        channels: 2,
    };
    assert_eq!(format.frame_bytes(), 6);
    let format = AudioParams {
        rate: 48000,
        bits: 32,
        channels: 1,
    };
    assert_eq!(format.frame_bytes(), 4);
}
