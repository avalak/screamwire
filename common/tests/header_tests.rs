use screamwire_common::scream::{channel_map, make_header};
use screamwire_common::types::AudioParams;

#[test]
fn test_standard_header() {
    let format = AudioParams {
        rate: 48000,
        bits: 16,
        channels: 2,
    };
    let header = make_header(format);
    assert_eq!(header, [0x01, 0x10, 0x02, 0x03, 0x00]);
}

#[test]
fn test_44100_stereo() {
    let format = AudioParams {
        rate: 44100,
        bits: 16,
        channels: 2,
    };
    let header = make_header(format);
    assert_eq!(header, [0x81, 0x10, 0x02, 0x03, 0x00]);
}

#[test]
fn test_96000_stereo() {
    let format = AudioParams {
        rate: 96000,
        bits: 16,
        channels: 2,
    };
    let header = make_header(format);
    assert_eq!(header, [0x02, 0x10, 0x02, 0x03, 0x00]);
}

#[test]
fn test_88200_stereo() {
    let format = AudioParams {
        rate: 88200,
        bits: 16,
        channels: 2,
    };
    let header = make_header(format);
    assert_eq!(header, [0x82, 0x10, 0x02, 0x03, 0x00]);
}

#[test]
fn test_192000_stereo() {
    let format = AudioParams {
        rate: 192000,
        bits: 16,
        channels: 2,
    };
    let header = make_header(format);
    assert_eq!(header, [0x04, 0x10, 0x02, 0x03, 0x00]);
}

#[test]
fn test_176400_stereo() {
    let format = AudioParams {
        rate: 176400,
        bits: 16,
        channels: 2,
    };
    let header = make_header(format);
    assert_eq!(header, [0x84, 0x10, 0x02, 0x03, 0x00]);
}

#[test]
fn test_48000_mono() {
    let format = AudioParams {
        rate: 48000,
        bits: 16,
        channels: 1,
    };
    let header = make_header(format);
    assert_eq!(header, [0x01, 0x10, 0x01, 0x01, 0x00]);
}

#[test]
fn test_44100_24bit() {
    let format = AudioParams {
        rate: 44100,
        bits: 24,
        channels: 2,
    };
    let header = make_header(format);
    assert_eq!(header, [0x81, 0x18, 0x02, 0x03, 0x00]);
}

#[test]
fn test_48000_32bit() {
    let format = AudioParams {
        rate: 48000,
        bits: 32,
        channels: 2,
    };
    let header = make_header(format);
    assert_eq!(header, [0x01, 0x20, 0x02, 0x03, 0x00]);
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
fn test_channel_map_6ch_in_header() {
    let format = AudioParams {
        rate: 48000,
        bits: 16,
        channels: 6,
    };
    let header = make_header(format);
    assert_eq!(header, [0x01, 0x10, 0x06, 0x0F, 0x06]);
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
