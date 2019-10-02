use bytes::{BufMut, Bytes, BytesMut};
use serde::{ser::Serialize, de::DeserializeOwned};

/// Trait that conflates serialization and deserialization.
pub trait Format: Send + Sync + 'static {
	type SerializeError;
	type DeserializeError;

	/// Serialize a value to a buffer.
	fn serialize<T: Serialize>(value: &T, buffer: &mut BytesMut) -> Result<(), Self::SerializeError>;

	/// Deserialize a value from a buffer.
	fn deserialize<T: DeserializeOwned>(buffer: &Bytes) -> Result<T, Self::DeserializeError>;
}

impl Format for () {
	type SerializeError = ();
	type DeserializeError = ();

	fn serialize<T: Serialize>(_value: &T, _buffer: &mut BytesMut) -> Result<(), ()> {
		unreachable!("u wot");
	}

	fn deserialize<T: DeserializeOwned>(_buffer: &Bytes) -> Result<T, ()> {
		unreachable!("u wot");
	}
}

/// MessagePack integration.
#[cfg(feature = "msgpack")]
#[derive(Copy, Clone, Debug)]
pub struct MessagePack;

#[cfg(feature = "msgpack")]
impl Format for MessagePack {
	type SerializeError = msgpack::encode::Error;
	type DeserializeError = msgpack::decode::Error;

	fn serialize<T: Serialize>(value: &T, buffer: &mut BytesMut) -> Result<(), Self::SerializeError> {
		msgpack::encode::write_named(&mut buffer.writer(), value)
	}

	fn deserialize<T: DeserializeOwned>(buffer: &Bytes) -> Result<T, Self::DeserializeError> {
		msgpack::decode::from_slice(buffer)
	}
}
