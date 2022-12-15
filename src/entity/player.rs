use std::fmt::Debug;

use tokio::sync::mpsc;

pub type PlayerId = String;

#[derive(Clone, Debug)]
pub struct Player<OutputMessageT> {
    pub id: PlayerId,
    pub sender: mpsc::UnboundedSender<OutputMessageT>,
}

impl<OutputMessageT> Player<OutputMessageT> {
    pub fn new(id: PlayerId, sender: mpsc::UnboundedSender<OutputMessageT>) -> Self {
        Self { id, sender }
    }

    pub fn send(
        &mut self,
        event: OutputMessageT,
    ) -> std::result::Result<(), mpsc::error::SendError<OutputMessageT>> {
        self.sender.send(event)
    }
}
