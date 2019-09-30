#![feature(type_ascription, async_closure)]

use std::{error::Error, net::SocketAddr, env};
use tokio::{self, codec::Framed, net::{TcpListener, TcpStream}};
use futures::{stream::StreamExt};
use protociolla::{self, format, Reframed};
use serde::{Serialize, Deserialize};

/// Normally this type would be shared between server and client binaries, but
/// I'd need an example only section in the crate or something.
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct Foo {
	pub a: u32,
	pub b: bool,
}

async fn accept(stream: TcpStream, _addr: SocketAddr) -> Result<(), Box<dyn Error>> {
	let packets = Framed::new(stream, protociolla::Codec);
	let mut packets = Reframed::<protociolla::Packets<format::MessagePack>>::new(packets);

	while let Some(Ok(packet)) = packets.next().await {
		println!("{:?}", packet);
		println!("{:?}", packet.cast::<Foo>());
	}

	Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let mut listener = TcpListener::bind(&format!("0.0.0.0:{}", env::args().nth(1).expect("no port"))).await?;

	loop {
		let (stream, addr) = listener.accept().await?;
		println!("{:?} connected", addr);

		tokio::spawn(async move {
			if let Err(e) = accept(stream, addr).await {
				eprintln!("rip: {}", e);
			}
		});
	}
}
