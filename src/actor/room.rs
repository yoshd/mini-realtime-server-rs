use std::collections::HashMap;

use log::{debug, warn};
use once_cell::sync::Lazy;
use tokio::sync::{mpsc, RwLock};

use super::event::*;
use crate::entity;

static ROOM_CHANNELS: Lazy<RwLock<HashMap<entity::RoomId, mpsc::UnboundedSender<InputEvent>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

pub async fn get_room_channel(id: &entity::RoomId) -> Option<mpsc::UnboundedSender<InputEvent>> {
    let room_channels = ROOM_CHANNELS.read().await;
    Some(room_channels.get(id)?.clone())
}

pub async fn get_or_create_room_channel(
    id: &entity::RoomId,
    config: entity::RoomConfig,
) -> mpsc::UnboundedSender<InputEvent> {
    let mut room_channels = ROOM_CHANNELS.write().await;
    let tx = room_channels.get(id);
    match tx {
        Some(tx) => tx.clone(),
        None => {
            let (tx, rx) = mpsc::unbounded_channel();
            let room_id = id.clone();
            {
                // moveされるのでここでcloneしておく。
                let room_id = id.clone();
                tokio::spawn(async move {
                    let mut room_runner = Room::new(entity::Room::new(room_id, config), rx);
                    room_runner.run().await
                });
            }
            room_channels.insert(room_id, tx.clone());
            tx
        }
    }
}

async fn remove_room_from_channels(id: &entity::RoomId) {
    let mut room_channels = ROOM_CHANNELS.write().await;
    room_channels.remove(id);
}

pub struct Room {
    room: entity::Room<OutputEvent>,
    room_rx: mpsc::UnboundedReceiver<InputEvent>,
}

impl Room {
    pub fn new(
        room: entity::Room<OutputEvent>,
        room_rx: mpsc::UnboundedReceiver<InputEvent>,
    ) -> Self {
        Self { room, room_rx }
    }

    pub async fn run(&mut self) {
        debug!("Start Room. room_id={}", self.room.id);
        while let Some(event) = self.room_rx.recv().await {
            match event {
                InputEvent::Join(event) => {
                    debug!("Receive InputJoinEvent");
                    self.handle_join_event(event);
                }
                InputEvent::Leave(event) => {
                    debug!("Receive InputLeaveEvent");
                    self.handle_leave_event(event);
                }
                InputEvent::Message(event) => {
                    debug!("Receive InputMessageEvent");
                    self.handle_message_event(event);
                }
            }

            if self.room.num_players() == 0 {
                debug!("Stop Room. room_id={}", self.room.id);
                // TODO: rx側のcloseを基本として、Player actor側でroom_tx.sendの結果をエラーハンドリングするという手もある。
                // どちらからのcloseを基本とするかは一考の余地があるが、 Roomが空になるかは基本的にはLeave次第(Player側に主導権があるもの)なので、
                // tx側からのcloseの方がgracefulかも。
                remove_room_from_channels(&self.room.id).await;
                break;
            }
        }
    }

    fn handle_join_event(&mut self, mut event: InputJoinEvent) {
        match self
            .room
            .add_player(event.player.clone(), &event.room_config)
        {
            Ok(_) => {
                let output_event = OutputEvent::Join(
                    Ok(OutputJoinEvent {
                        room_id: self.room.id.clone(),
                        player_id: event.player.id.clone(),
                        room_player_ids: self.room.players.keys().cloned().collect(),
                        room_config: self.room.config.clone(),
                    }),
                );

                self.room.broadcast(output_event);
            }
            Err(err) => {
                if event
                    .player
                    .send(OutputEvent::Join(Err(err)))
                    .is_err()
                {
                    // Joinに失敗かつプレイヤーが切断した場合なので特にハンドリングは不要。
                    warn!("Join failed and player disconnected");
                }
            }
        }
    }

    fn handle_leave_event(&mut self, event: InputLeaveEvent) {
        if self.room.is_joined(&event.player_id) {
            let output_event = OutputEvent::Leave(
                Ok(OutputLeaveEvent {
                    room_id: self.room.id.clone(),
                    player_id: event.player_id.clone(),
                }),
            );
            self.room.broadcast(output_event);
            let ok = self.room.remove_player(&event.player_id);
            debug_assert!(ok);
        } else {
            // 二重LeaveかRoomに所属していなかった。
            // このように呼び出されない想定なのでここでは何もしない。
            warn!("Double leave or not joining the room");
        }
    }

    fn handle_message_event(&mut self, event: InputMessageEvent) {
        let output_event = OutputEvent::Message(OutputMessageEvent {
            room_id: self.room.id.clone(),
            body: event.body,
            sender_player_id: event.sender_player_id,
        });

        if event.target_ids.is_empty() {
            self.room.broadcast(output_event);
        } else {
            event.target_ids.iter().for_each(|id| {
                if self.room.is_joined(id) {
                    self.room.send(id, output_event.clone());
                } else {
                    // ターゲットにRoomに参加していないプレイヤーが含まれていた。
                    // 現状メッセージはベストエフォート想定なので何もしない。
                    warn!("The targets contained players who didn't join");
                }
            });
        }
    }
}
