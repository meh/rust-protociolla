#![feature(type_ascription, async_closure)]

pub mod reframe;
pub use crate::reframe::{Reframe, Reframed};

pub mod format;
pub use crate::format::Format;

pub mod packet;
pub use crate::packet::Packet;

mod message;
pub use crate::message::Message;

mod session;
pub use crate::session::Session;

mod codec;
pub use crate::codec::{Codec, Packets, Sessions};

use std::marker::Unpin;
use tokio::{codec::Framed, io::{AsyncRead, AsyncWrite}};

pub fn mi<F, S>(socket: S) -> Reframed<Sessions<F>>
  where F: Format,
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static
{
  let packets = Framed::new(socket, Codec);
  let packets = Reframed::<Packets<F>>::new(packets);
  let packets = Reframed::<Sessions<F>>::new(packets);

  packets
}
