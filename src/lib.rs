#![feature(type_ascription, async_closure)]

pub mod reframe;
pub use crate::reframe::{Reframe, Reframed};

pub mod format;
pub use crate::format::Format;

mod packet;
pub use crate::packet::{Header, Cookie, Packet};

mod codec;
pub use crate::codec::{Codec, Packets, Streams};

use std::marker::Unpin;
use tokio::{codec::Framed, io::{AsyncRead, AsyncWrite}};

pub fn mi<F, S>(socket: S) -> Reframed<Streams<F>>
  where F: Format,
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static
{
  let packets = Framed::new(socket, Codec);
  let packets = Reframed::<Packets<F>>::new(packets);
  let packets = Reframed::<Streams<F>>::new(packets);

  packets
}
