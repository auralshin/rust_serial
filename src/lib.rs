use base64::{
    alphabet,
    engine::{self, general_purpose},
    Engine,
};
use flate2::{bufread::GzDecoder, write::GzEncoder, Compression};
use neon::prelude::*;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

#[derive(Serialize, Deserialize, Debug)]
struct Data {
    value: String,
}

pub struct SerializerOptions {
    compression: Compression,
}

impl SerializerOptions {
    pub fn new(compression: Compression) -> Self {
        Self { compression }
    }
}

fn compress_data(
    data: &[u8],
    compression: Compression,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut encoder = GzEncoder::new(Vec::new(), compression);
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

fn decompress_data(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut decoder = GzDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

fn serialize(mut cx: FunctionContext) -> JsResult<JsString> {
    let input = cx.argument::<JsString>(0)?.value(&mut cx);
    let data: Data = serde_json::from_str(&input).unwrap();

    let compression = cx.argument::<JsNumber>(1)?.value(&mut cx) as u32;
    let options = SerializerOptions::new(Compression::new(compression));

    let serialized = serde_json::to_string(&data).unwrap();
    let compressed = compress_data(serialized.as_bytes(), options.compression)
        .or_else(|err| cx.throw_error(format!("Compression error: {}", err)))?;

    // Create a base64 engine instance
    let base64_engine = general_purpose::STANDARD; // Adjust according to your needs

    // Encode using the engine
    let encoded = base64_engine.encode(&compressed);

    Ok(cx.string(encoded))
}

fn deserialize(mut cx: FunctionContext) -> JsResult<JsString> {
    let input = cx.argument::<JsString>(0)?.value(&mut cx);

    // Create a custom base64 decoding engine
    const CUSTOM_ENGINE: engine::GeneralPurpose =
        engine::GeneralPurpose::new(&alphabet::URL_SAFE, general_purpose::NO_PAD);

    // Decode using the custom engine
    let compressed_data = CUSTOM_ENGINE.decode(input).unwrap();

    let decompressed = decompress_data(&compressed_data)
        .or_else(|err| cx.throw_error(format!("Decompression error: {}", err)))?;
    let data: Data = serde_json::from_slice(&decompressed).unwrap();

    Ok(cx.string(data.value))
}

#[neon::main]
fn main(mut cx: ModuleContext) -> NeonResult<()> {
    cx.export_function("serialize", serialize)?;
    cx.export_function("deserialize", deserialize)?;
    Ok(())
}
