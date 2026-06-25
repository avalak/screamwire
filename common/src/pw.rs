use crate::scream::AUDIO_PAYLOAD_SIZE;
use crate::types::AudioParams;
use pipewire::spa;

/// Build a SPA format pod for the given sample rate, bit depth and channel count.
pub fn make_format_data(format: AudioParams) -> Vec<u8> {
    let audio_format = match format.bits {
        16 => spa::sys::SPA_AUDIO_FORMAT_S16_LE,
        24 => spa::sys::SPA_AUDIO_FORMAT_S24_LE,
        32 => spa::sys::SPA_AUDIO_FORMAT_S32_LE,
        _ => panic!("Unsupported bit depth: {}", format.bits),
    };

    let obj = spa::pod::Object {
        type_: spa::sys::SPA_TYPE_OBJECT_Format,
        id: spa::sys::SPA_PARAM_EnumFormat,
        properties: vec![
            spa::pod::Property {
                key: spa::sys::SPA_FORMAT_mediaType,
                flags: spa::pod::PropertyFlags::empty(),
                value: spa::pod::Value::Id(spa::utils::Id(spa::sys::SPA_MEDIA_TYPE_audio)),
            },
            spa::pod::Property {
                key: spa::sys::SPA_FORMAT_mediaSubtype,
                flags: spa::pod::PropertyFlags::empty(),
                value: spa::pod::Value::Id(spa::utils::Id(spa::sys::SPA_MEDIA_SUBTYPE_raw)),
            },
            spa::pod::Property {
                key: spa::sys::SPA_FORMAT_AUDIO_format,
                flags: spa::pod::PropertyFlags::empty(),
                value: spa::pod::Value::Id(spa::utils::Id(audio_format)),
            },
            spa::pod::Property {
                key: spa::sys::SPA_FORMAT_AUDIO_rate,
                flags: spa::pod::PropertyFlags::empty(),
                value: spa::pod::Value::Int(format.rate as i32),
            },
            spa::pod::Property {
                key: spa::sys::SPA_FORMAT_AUDIO_channels,
                flags: spa::pod::PropertyFlags::empty(),
                value: spa::pod::Value::Int(format.channels as i32),
            },
        ],
    };

    spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(obj),
    )
    .unwrap()
    .0
    .into_inner()
}

/// Build a serialized SPA buffers parameter that requests two buffers
/// with a block size of one Scream audio payload (1152 bytes).
pub fn make_buffers_data() -> Vec<u8> {
    let obj = spa::pod::Object {
        type_: spa::sys::SPA_TYPE_OBJECT_ParamBuffers,
        id: spa::sys::SPA_PARAM_Buffers,
        properties: vec![
            spa::pod::Property {
                key: spa::sys::SPA_PARAM_BUFFERS_buffers,
                flags: spa::pod::PropertyFlags::empty(),
                value: spa::pod::Value::Int(2),
            },
            spa::pod::Property {
                key: spa::sys::SPA_PARAM_BUFFERS_blocks,
                flags: spa::pod::PropertyFlags::empty(),
                value: spa::pod::Value::Int(1),
            },
            spa::pod::Property {
                key: spa::sys::SPA_PARAM_BUFFERS_size,
                flags: spa::pod::PropertyFlags::empty(),
                value: spa::pod::Value::Int(AUDIO_PAYLOAD_SIZE as i32),
            },
        ],
    };

    spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(obj),
    )
    .unwrap()
    .0
    .into_inner()
}
