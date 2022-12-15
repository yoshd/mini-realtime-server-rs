//! 各プロトコルから利用される共通処理。
//! actorを通じでクライアントからのメッセージと内部ロジックのやり取りを行う。

mod event;
mod player;
mod room;

pub use event::*;
pub use player::*;
pub use room::*;
