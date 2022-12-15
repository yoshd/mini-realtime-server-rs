use std::fmt::Debug;

use bytes::Bytes;

use crate::entity;

type Result<T> = std::result::Result<T, entity::RoomError>;

#[derive(Clone, Debug)]
pub struct InputJoinEvent {
    pub player: entity::Player<OutputEvent>,
    pub room_config: entity::RoomConfig,
}

#[derive(Clone, Debug)]
pub struct InputLeaveEvent {
    pub player_id: entity::PlayerId,
}

#[derive(Clone, Debug)]
pub struct InputMessageEvent {
    pub sender_player_id: entity::PlayerId,
    pub target_ids: Vec<entity::PlayerId>,
    pub body: Bytes,
}

#[derive(Clone, Debug)]
pub enum InputEvent {
    Join(InputJoinEvent),
    Leave(InputLeaveEvent),
    Message(InputMessageEvent),
}

#[derive(Clone, Debug)]
pub struct OutputJoinEvent {
    pub room_id: entity::RoomId,
    pub player_id: entity::PlayerId,
    pub room_player_ids: Vec<entity::PlayerId>,
    pub room_config: entity::RoomConfig,
}

#[derive(Clone, Debug)]
pub struct OutputLeaveEvent {
    pub room_id: entity::RoomId,
    pub player_id: entity::PlayerId,
}

#[derive(Clone, Debug)]
pub struct OutputMessageEvent {
    pub room_id: entity::RoomId,
    pub sender_player_id: entity::PlayerId,
    pub body: Bytes,
}

#[derive(Clone, Debug)]
pub enum OutputEvent {
    Join(Result<OutputJoinEvent>),
    Leave(Result<OutputLeaveEvent>),
    Message(OutputMessageEvent),
}
