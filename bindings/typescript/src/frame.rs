use napi::bindgen_prelude::Buffer;
use napi_derive::napi;

#[napi(object)]
pub struct JsFrame {
    pub channel: u16,
    pub payload: Buffer,
}
