use std::{io, usize, marker::PhantomData};
use tokio::{self, codec::{Decoder, Encoder}, sync::mpsc::{unbounded_channel}};
use bytes::{BufMut, Bytes, BytesMut, ByteOrder, BigEndian};
use futures::{stream::{StreamExt}, sink::{SinkExt}};
use t1ha::T1haHashMap as HashMap;
use crate::{Format, reframe::{self, Reframe, Source}, packet::{self, Packet}, Session};

/// `tokio::{Decoder, Encoder}` to transform a `Stream + Sink` of bytes to one
/// of header and payload.
pub struct Codec;

impl Default for Codec {
	fn default() -> Self {
		Self
	}
}

impl Decoder for Codec {
	type Item = (packet::Header, Bytes);
	type Error = io::Error;

	fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<(packet::Header, Bytes)>, io::Error> {
		if buf.len() < 4 {
			return Ok(None);
		}

		let header = packet::Header {
			cookie: BigEndian::read_u16(&buf[0..]),
			length: BigEndian::read_u16(&buf[2..])
		};

		if buf.len() - 4 < usize::from(header.length()) {
			return Ok(None);
		}

		let payload = buf.split_off(4);

		Ok(Some((header, payload.freeze())))
	}
}

impl Encoder for Codec {
	type Item = (packet::Header, Bytes);
	type Error = io::Error;

	fn encode(&mut self, (header, payload): (packet::Header, Bytes), buf: &mut BytesMut) -> Result<(), io::Error> {
		buf.reserve(4 + payload.len());

		buf.put_u16_be(header.cookie);
		buf.put_u16_be(header.length);
		buf.put_slice(&payload);

		Ok(())
	}
}

/// Reframe a `Codec` into a `Packet`.
#[derive(Copy, Clone, Debug)]
pub struct Packets<F = ()> {
	_marker: PhantomData<F>
}

impl<F: Format> Reframe for Packets<F> {
	type StreamFrom = (packet::Header, Bytes);
	type StreamInto = Packet<F>;

	type SinkFrom = (packet::Header, Bytes);
	type SinkInto = Packet<F>;

	type Error = io::Error;

	fn reframe(source: Source<Self::StreamFrom, Self::SinkFrom, Self::Error>) -> Source<Self::StreamInto, Self::SinkInto, Self::Error> {
		let Source { mut stream, mut sink } = source;

		Source::new(
			reframe::stream(|mut out| async move {
				macro_rules! next {
					($body:expr) => (
						if let Some(value) = stream.next().await {
							match value {
								Ok(value) => value,

								Err(error) => {
									out.send(Err(error)).await.unwrap();
									return;
								}
							}
						}
						else {
							return;
						}
					);
				}

				loop {
					let mut payload = BytesMut::new();

					let mut packet = next!(stream);
					payload.extend_from_slice(&packet.1);

					while packet.0.has_more_payload() {
						packet = next!(stream);
						payload.extend_from_slice(&packet.1);
					}

					out.send(Ok(if let Some(cookie) = packet.0.cookie() {
						if packet.0.has_more_packets() {
							Packet::<F>::new(packet::Cookie::Stream(cookie), payload.freeze())
						}
						else {
							Packet::<F>::new(packet::Cookie::Single(cookie), payload.freeze())
						}
					}
					else {
						Packet::<F>::new(packet::Cookie::Oneshot, payload.freeze())
					})).await.unwrap();
				}
			}),

			reframe::sink(|mut rx| async move {
				while let Some(packet) = rx.next().await : Option<Packet<F>> {
					let mut chunks = (0 ..= packet.bytes().len() / 0xfffe).peekable();

					while let Some(chunk) = chunks.next() {
						let is_last = chunks.peek().is_none();
						let payload = packet.bytes().slice(chunk * 0xfffe, packet.bytes().len() - (chunk * 0xfffe));
						let length  = if is_last { Some(payload.len()) } else { None };
						let header  = match packet.cookie() {
							packet::Cookie::Oneshot =>
								packet::Header::oneshot(length),

							packet::Cookie::Single(cookie) =>
								packet::Header::single(cookie, length),

							packet::Cookie::Stream(cookie) =>
								packet::Header::stream(cookie, length),
						};

						sink.send((header, payload)).await.unwrap();
					}
				}
			}).sink_map_err(|err| io::Error::new(io::ErrorKind::Interrupted, err)))
	}
}

/// Reframe packets into multiple streams (one per cookie).
#[derive(Copy, Clone, Debug)]
pub struct Sessions<F = ()> {
	_marker: PhantomData<F>,
}

impl<F: Format> Reframe for Sessions<F> {
	type Error = io::Error;

	type StreamFrom = Packet<F>;
	type StreamInto = Session<F>;

	type SinkFrom = Packet<F>;
	type SinkInto = Packet<F>;

	fn reframe(source: Source<Self::StreamFrom, Self::SinkFrom, Self::Error>) -> Source<Self::StreamInto, Self::SinkInto, Self::Error> {
		let reframe::Source { mut stream, mut sink } = source;
		let sink = {
			let (tx, mut rx) = unbounded_channel();

			tokio::spawn(async move {
				while let Some(value) = rx.next().await : Option<Packet<F>> {
					sink.send(value).await.unwrap();
				}
			});

			tx
		};

		let sunk = sink.clone();

		reframe::Source::new(
			reframe::stream(|mut out| async move {
				macro_rules! next {
					($body:expr) => (
						if let Some(value) = stream.next().await {
							match value {
								Ok(value) => value,

								Err(error) => {
									out.send(Err(error)).await.unwrap();
									return;
								}
							}
						}
						else {
							return;
						}
					);
				}

				let mut channels = HashMap::<u16, _>::default();

				loop {
					let packet = next!(stream);

					match packet.cookie() {
						packet::Cookie::Oneshot => {
							out.send(Ok(Session::no_reply(packet))).await.unwrap();
						}

						packet::Cookie::Stream(cookie) => {
							if !channels.contains_key(&cookie) {
								let session = Session::new(cookie, sink.clone());
								let sender = session.sender();

								out.send(Ok(session)).await.unwrap();
								channels.insert(cookie, sender);
							}

							channels.get_mut(&cookie).unwrap().send(packet.into()).await.unwrap();
						}

						packet::Cookie::Single(cookie) => {
							if channels.contains_key(&cookie) {
								channels.get_mut(&cookie).unwrap().send(packet.into()).await.unwrap();
								channels.remove(&cookie);
							}
							else {
								let session = Session::new(cookie, sink.clone());
								let mut sender = session.sender();

								out.send(Ok(session)).await.unwrap();
								sender.send(packet.into()).await.unwrap();
							}
						}
					}
				}
			}),

			sunk.sink_map_err(|err| io::Error::new(io::ErrorKind::Interrupted, err)))
	}
}
