use bson::serde_helpers;
use flate2::{bufread::GzDecoder, write::GzEncoder, Compression};
use neon::prelude::*;
use rmp_serde::{Deserializer, Serializer};
use rmp_serde::{Deserializer, Serializer};
use serde::de::DeserializeOwned;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json;
use std::io::prelude::*;
use std::sync::Arc;
use tokio::runtime::Runtime;

#[derive(Serialize, Deserialize, Debug)]
pub struct Data {
    value: String,
}

#[derive(Debug, Clone)]
pub enum SerializationFormat {
    Json,
    MessagePack,
    Bson,
}

pub struct Options {
    format: SerializationFormat,
    compression: Compression,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            format: SerializationFormat::Json,
            compression: Compression::fast(),
        }
    }
}

fn compress_data(data: &[u8], compression: Compression) -> Result<Vec<u8>, String> {
    let mut encoder = GzEncoder::new(Vec::new(), compression);
    encoder.write_all(data).map_err(|e| e.to_string())?;
    encoder.finish().map_err(|e| e.to_string())
}

fn decompress_data(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = GzDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .map_err(|e| e.to_string())?;
    Ok(decompressed)
}

struct SerializeTask {
    data: Arc<Data>,
    options: Arc<Options>,
}

impl Task for SerializeTask {
    type Output = String;
    type Error = String;
    type JsValue = JsString;

    fn perform(&self) -> Result<Self::Output, Self::Error> {
        let serialized = match self.options.format {
            SerializationFormat::Json => serde_json::to_vec(&self.data),
            SerializationFormat::MessagePack => {
                let mut buf = Vec::new();
                self.data
                    .serialize(&mut Serializer::new(&mut buf))
                    .map(|_| buf)
            }
            SerializationFormat::Bson => {
                serde_helpers::to_bson(&self.data).map_err(|e| e.to_string())?
            }
        };
        let serialized_data = serialized.map_err(|e| e.to_string())?;
        let compressed_data = compress_data(&serialized_data, self.options.compression)?;
        Ok(base64::encode(&compressed_data))
    }

    fn complete(
        self,
        mut cx: TaskContext,
        result: Result<Self::Output, Self::Error>,
    ) -> JsResult<Self::JsValue> {
        let result_str = result.map_err(|e| cx.string(e))?;
        Ok(cx.string(result_str))
    }
}

fn serialize(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let input = cx.argument::<JsString>(0)?.value(&mut cx);
    let data: Data = serde_json::from_str(&input)
        .map_err(|err| cx.throw_error(format!("Failed to parse input: {}", err)))?;

    let options = Options::default(); // You can customize the options here or accept them as a function argument.

    let task = SerializeTask {
        data: Arc::new(data),
        options: Arc::new(options),
    };
    let callback = cx.argument::<JsFunction>(1)?;
    task.schedule(callback);
    Ok(cx.undefined())
}

struct DeserializeTask {
    serialized: String,
    options: Arc<Options>,
}

impl Task for DeserializeTask {
    type Output = Data;
    type Error = String;
    type JsValue = JsValue;

    fn perform(&self) -> Result<Self::Output, Self::Error> {
        let compressed_data = base64::decode(&self.serialized).map_err(|e| e.to_string())?;
        let decompressed_data = decompress_data(&compressed_data)?;
        let deserialized = match self.options.format {
            SerializationFormat::Json => serde_json::from_slice(&decompressed_data),
            SerializationFormat::MessagePack => Ok({
                let mut de = Deserializer::new(&decompressed_data[..]);
                Deserialize::deserialize(&mut de)
            }),
            SerializationFormat::Bson => serde_helpers::from_bson(
                serde_helpers::deserialize_bson(&decompressed_data).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?,
        };
        deserialized.map_err(|e| e.to_string())
    }

    fn complete(
        self,
        mut cx: TaskContext,
        result: Result<Self::Output, Self::Error>,
    ) -> JsResult<Self::JsValue> {
        let data = result.map_err(|e| cx.string(e))?;
        let object = neon_serde::to_value(&mut cx, &data)?;
        Ok(object)
    }
}
fn deserialize(mut cx: FunctionContext) -> JsResult<JsUndefined> {
    let serialized = cx.argument::<JsString>(0)?.value(&mut cx);
    let options = Options::default(); // You can customize the options here or accept them as a function argument.

    let task = DeserializeTask {
        serialized,
        options: Arc::new(options),
    };
    let callback = cx.argument::<JsFunction>(1)?;
    task.schedule(callback);

    Ok(cx.undefined())
}
register_module!(mut cx, {
    cx.export_function("serialize", serialize)?;
    cx.export_function("deserialize", deserialize)?;
    Ok(())
});
