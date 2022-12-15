use std::{collections::HashMap, fmt::Debug};

use log::warn;
use thiserror::Error;

use super::player::*;

pub type RoomId = String;

type Result<T> = std::result::Result<T, RoomError>;

#[derive(Error, Clone, Debug, PartialEq)]
pub enum RoomError {
    #[error("the player has already joined the room. roomId={0}, playerId={1}")]
    AlreadyJoinedRoom(RoomId, PlayerId),
    #[error("the room config does not match. roomId={0}, playerId={1}")]
    RoomConfigDoesNotMatch(RoomId, PlayerId),
    #[error("the room is full. roomId={0}, playerId={1}")]
    RoomIsFull(RoomId, PlayerId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoomConfig {
    pub max_players: u32,
}

impl RoomConfig {
    pub fn default() -> Self {
        RoomConfig {
            max_players: 2,
        }
    }
}

#[derive(Debug)]
pub struct Room<OutputMessageT> {
    pub id: RoomId,
    pub config: RoomConfig,
    pub players: HashMap<PlayerId, Player<OutputMessageT>>,
}

impl<OutputMessageT> Room<OutputMessageT> {
    pub fn new(id: RoomId, config: RoomConfig) -> Self {
        Self {
            id,
            config,
            players: HashMap::new(),
        }
    }

    pub fn add_player(
        &mut self,
        player: Player<OutputMessageT>,
        config: &RoomConfig,
    ) -> Result<()> {
        if self.config != *config {
            return Err(RoomError::RoomConfigDoesNotMatch(
                self.id.clone(),
                player.id,
            ));
        }

        if self.players.contains_key(&player.id) {
            return Err(RoomError::AlreadyJoinedRoom(self.id.clone(), player.id));
        }

        if self.num_players() >= self.config.max_players {
            return Err(RoomError::RoomIsFull(self.id.clone(), player.id));
        }


        self.players.insert(player.id.clone(), player);

        Ok(())
    }

    pub fn remove_player(&mut self, player_id: &PlayerId) -> bool {
        self.players.remove(player_id).is_some()
    }

    pub fn num_players(&self) -> u32 {
        self.players.len() as u32
    }

    pub fn broadcast(&mut self, event: OutputMessageT)
    where
        OutputMessageT: Clone,
    {
        self.players.iter_mut().for_each(|(_, player)| {
            if let Err(err) = player.send(event.clone()) {
                // 切断によって送信できなかったケース。
                // このプレイヤーをRoomから削除する等はRoomを利用する側の責務として、ここではログだけ出しておく。
                warn!(
                    "failed to send message. player_id={}, error={}",
                    player.id,
                    err.to_string(),
                );
            }
        });
    }

    pub fn send(&mut self, player_id: &PlayerId, event: OutputMessageT) {
        if let Some(player) = self.players.get_mut(player_id) {
            if let Err(err) = player.send(event) {
                // 切断によって送信できなかったケース。
                // このプレイヤーをRoomから削除する等はRoomを利用する側の責務として、ここではログだけ出しておく。
                warn!(
                    "failed to send message. player_id={}, error={}",
                    player.id,
                    err.to_string()
                );
            }
        }
    }

    pub fn is_joined(&self, player_id: &PlayerId) -> bool {
        self.players.contains_key(player_id)
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn join_room_and_send_message_normal() {
        let mut room = Room::new(
            "test".to_string(),
            RoomConfig {
                max_players: 2,
            },
        );

        let (p1_tx, mut p1_rx) = mpsc::unbounded_channel();
        let p1_id = "p1".to_string();
        let p1 = Player::new(p1_id.clone(), p1_tx);

        let (p2_tx, mut p2_rx) = mpsc::unbounded_channel();
        let p2 = Player::new("p2".to_string(), p2_tx);

        let result = room.add_player(
            p1,
            &RoomConfig {
                max_players: 2,
            },
        );
        assert!(result.is_ok());
        let result = room.add_player(
            p2,
            &RoomConfig {
                max_players: 2,
            },
        );
        assert!(result.is_ok());
        let msg_to_p1 = "message to p1";
        room.send(&p1_id, msg_to_p1);
        let p1_msg = p1_rx.recv().await;
        assert_eq!(msg_to_p1, p1_msg.unwrap());

        let broadcast_msg = "broadcast";
        room.broadcast(broadcast_msg);
        let p1_msg = p1_rx.recv().await;
        assert_eq!(broadcast_msg, p1_msg.unwrap());
        let p2_msg = p2_rx.recv().await;
        assert_eq!(broadcast_msg, p2_msg.unwrap());
    }

    #[test]
    fn failed_to_join_room_over_capacity() {
        let room_config = RoomConfig {
            max_players: 1,
        };
        let mut room = Room::new("test".to_string(), room_config.clone());

        let (tx, _) = mpsc::unbounded_channel::<()>();
        let p1 = Player::new("p1".to_string(), tx.clone());
        let p2_id = "p2".to_string();
        let p2 = Player::new(p2_id.clone(), tx);

        let result = room.add_player(p1, &room_config);
        assert!(result.is_ok());
        let result = room.add_player(p2, &room_config);
        assert_eq!(RoomError::RoomIsFull(room.id, p2_id), result.err().unwrap());
    }

    #[test]
    fn failed_to_join_room_config_does_not_match() {
        let mut room = Room::new(
            "test".to_string(),
            RoomConfig {
                max_players: 2,
            },
        );

        let (tx, _) = mpsc::unbounded_channel::<()>();
        let p1_id = "p1".to_string();
        let p1 = Player::new(p1_id.clone(), tx);

        let result = room.add_player(
            p1,
            &RoomConfig {
                max_players: 1,
            },
        );
        assert_eq!(
            RoomError::RoomConfigDoesNotMatch(room.id, p1_id),
            result.err().unwrap(),
        );
    }

    #[test]
    fn leave_room_and_num_players_normal() {
        let room_config = RoomConfig {
            max_players: 2,
        };
        let mut room = Room::new("test".to_string(), room_config.clone());

        let (tx, _) = mpsc::unbounded_channel::<()>();
        let p1 = Player::new("p1".to_string(), tx.clone());
        let p2_id = "p2".to_string();
        let p2 = Player::new(p2_id.clone(), tx);

        room.add_player(p1, &room_config).unwrap();
        room.add_player(p2, &room_config).unwrap();

        assert_eq!(2, room.num_players());
        assert!(room.remove_player(&p2_id));
        assert_eq!(1, room.num_players());
    }
}
