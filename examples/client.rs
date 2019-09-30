#![feature(type_ascription, async_closure)]

use std::{error::Error, env};
use tokio::{self, net::TcpStream};
use futures::{sink::SinkExt};
use protociolla::{self, Packet, format};
use serde::{Serialize, Deserialize};

/// Normally this type would be shared between server and client binaries, but
/// I'd need an example only section in the crate or something.
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct Foo {
	pub a: u32,
	pub b: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let stream = TcpStream::connect(&env::args().nth(1).expect("no host")).await?;
	let mut packets = protociolla::mi::<format::MessagePack, _>(stream);

	packets.send(Packet::oneshot(&Foo { a: 32, b: false })?).await?;

	loop {
		::std::thread::sleep(::std::time::Duration::from_secs(10_000));
	}
}
