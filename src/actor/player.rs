use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use log::{debug, error};
use once_cell::sync::Lazy;

use tokio::sync::{mpsc, RwLock};

use super::event::*;
use super::room::*;
use crate::config;
use crate::entity;
use crate::protobuf;

static PLAYERS: Lazy<RwLock<HashSet<entity::PlayerId>>> = Lazy::new(|| RwLock::new(HashSet::new()));

async fn register_player(id: entity::PlayerId) -> bool {
    let mut players = PLAYERS.write().await;
    players.insert(id)
}

async fn unregister_player(id: &entity::PlayerId) {
    let mut players = PLAYERS.write().await;
    players.remove(id);
}

pub struct Player {
    input_tx: mpsc::UnboundedSender<protobuf::app::ClientMessage>,
    output_rx: mpsc::UnboundedReceiver<protobuf::app::ServerMessage>,
}

impl Player {
    pub fn new(config: Arc<config::Config>) -> Self {
        let (input_tx, mut input_rx) = mpsc::unbounded_channel::<protobuf::app::ClientMessage>();
        let (output_tx, output_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            debug!("Start player actor task");
            let (player_tx, mut player_rx) = mpsc::unbounded_channel();
            let player_id = match Self::wait_login(&mut input_rx, &output_tx, &config).await {
                Some(player_id) => player_id,
                None => {
                    drop(output_tx);
                    return;
                }
            };

            let player = entity::Player::new(player_id, player_tx);
            let mut joined_rooms: HashMap<entity::RoomId, mpsc::UnboundedSender<InputEvent>> =
                HashMap::new();
            loop {
                tokio::select! {
                    message = input_rx.recv() => {
                        Self::on_client_message(message, &output_tx, &player).await;
                    }
                    event = player_rx.recv() => {
                        Self::on_output_event(event, &output_tx, &player.id, &mut joined_rooms).await;
                    }
                    _ = output_tx.closed() => {
                        // 切断した場合。
                        // JoinしているルームにLeaveイベントを投げる。
                        // output_tx/rxがcloseしているのでレスポンスだけ返るということも無い。
                        for (_, room_tx) in joined_rooms.iter() {
                            let result = room_tx.send(InputEvent::Leave(
                                Box::new(InputLeaveEvent {
                                    player_id: player.id.clone(),
                                }),
                            ));

                            // RoomがDropしている場合(プレイヤー数が0)。ここではハンドリングしない。
                            if result.is_err() {
                                debug!("Room has been dropped");
                            }
                        }

                        unregister_player(&player.id).await;
                        debug!("Finish player actor task");
                        return;
                    }
                };
            }
        });

        Self {
            input_tx,
            output_rx,
        }
    }

    pub fn send(
        &self,
        message: protobuf::app::ClientMessage,
    ) -> Result<(), mpsc::error::SendError<protobuf::app::ClientMessage>> {
        self.input_tx.send(message)
    }

    pub async fn recv(&mut self) -> Option<protobuf::app::ServerMessage> {
        self.output_rx.recv().await
    }

    async fn wait_login(
        input_rx: &mut mpsc::UnboundedReceiver<protobuf::app::ClientMessage>,
        output_tx: &mpsc::UnboundedSender<protobuf::app::ServerMessage>,
        config: &config::Config,
    ) -> Option<entity::PlayerId> {
        while let Some(message) = input_rx.recv().await {
            if let Some(data) = message.data {
                match data {
                    protobuf::app::client_message::Data::LoginRequest(req) => {
                        let ok = register_player(req.player_id.clone()).await;
                        if !ok {
                            // すでにログインしていた場合。
                            Self::send_login_error(
                                &req.player_id,
                                protobuf::app::ErrorCode::AlreadyLoggedIn,
                                "Already logged in".to_string(),
                                output_tx,
                            )
                            .await;
                            return None;
                        }

                        match req.auth_config {
                            Some(auth_config) => match auth_config {
                                protobuf::app::login_request::AuthConfig::Bearer(bearer) => {
                                    if bearer.token != config.auth.bearer {
                                        Self::send_login_error(
                                            &req.player_id,
                                            protobuf::app::ErrorCode::Unauthorized,
                                            "Unauthorized".to_string(),
                                            output_tx,
                                        )
                                        .await;
                                        return None;
                                    }

                                    Self::send_login_ok(output_tx);
                                    return Some(req.player_id);
                                }
                            },
                            None => {
                                Self::send_login_error(
                                    &req.player_id,
                                    protobuf::app::ErrorCode::Unauthorized,
                                    "Unauthorized".to_string(),
                                    output_tx,
                                )
                                .await;
                                return None;
                            }
                        }
                    }
                    // ひとまずLogin以外がきたら切断にしてしまう。
                    _ => return None,
                };
            }
        }

        None
    }

    fn send_login_ok(tx: &mpsc::UnboundedSender<protobuf::app::ServerMessage>) {
        let result = tx.send(protobuf::app::ServerMessage {
            data: Some(protobuf::app::server_message::Data::LoginResponse(
                protobuf::app::LoginResponse {
                    error: Some(protobuf::app::Error {
                        code: protobuf::app::ErrorCode::None as i32,
                        message: String::new(),
                    }),
                },
            )),
        });

        // レスポンスを返す前に切断した場合。
        // 別途切断処理でハンドリングされるので、ここではハンドリング不要。
        if result.is_err() {
            debug!("Player disconnected before sending response");
        }
    }

    async fn send_login_error(
        player_id: &entity::PlayerId,
        code: protobuf::app::ErrorCode,
        message: String,
        tx: &mpsc::UnboundedSender<protobuf::app::ServerMessage>,
    ) {
        let result = tx.send(protobuf::app::ServerMessage {
            data: Some(protobuf::app::server_message::Data::LoginResponse(
                protobuf::app::LoginResponse {
                    error: Some(protobuf::app::Error {
                        code: code as i32,
                        message,
                    }),
                },
            )),
        });

        // レスポンスを返す前に切断した場合。
        // 別途切断処理でハンドリングされるので、ここではハンドリング不要。
        if result.is_err() {
            debug!("Player disconnected before sending response");
        }

        unregister_player(player_id).await;
    }

    async fn on_client_message(
        message: Option<protobuf::app::ClientMessage>,
        output_tx: &mpsc::UnboundedSender<protobuf::app::ServerMessage>,
        player: &entity::Player<OutputEvent>,
    ) {
        if let Some(client_message) = message {
            debug!("ClientMessage: {:?}", client_message);
            // output_tx.sendのエラー(output_rxがDrop or closeされている状態)は呼び出し元の次回ループでハンドリングされるので、この関数内ではハンドリングしない。
            if let Some(data) = client_message.data {
                match data {
                    protobuf::app::client_message::Data::LoginRequest(_) => {
                        Self::try_to_send_output_message(
                            output_tx,
                            protobuf::app::ServerMessage {
                                data: Some(protobuf::app::server_message::Data::LoginResponse(
                                    protobuf::app::LoginResponse {
                                        error: Some(protobuf::app::Error {
                                            code: protobuf::app::ErrorCode::AlreadyLoggedIn as i32,
                                            message: "Already logged in".to_string(),
                                        }),
                                    },
                                )),
                            },
                        );
                    }
                    protobuf::app::client_message::Data::JoinRequest(req) => {
                        let room_config = match req.room_config {
                            Some(req_room_config) => entity::RoomConfig {
                                max_players: req_room_config.max_players,
                            },
                            None => entity::RoomConfig::default(),
                        };

                        let room_tx =
                            get_or_create_room_channel(&req.room_id, room_config.clone()).await;
                        // room_tx.sendが失敗するのはRoomがDropした場合。
                        // タイミング次第でこのJoin前に別プレイヤーがLeaveしてRoomの人数が0になればあり得なくもない。
                        // ひとまひとまずこの場合はエラーを返す。
                        let result = room_tx.send(InputEvent::Join(Box::new(InputJoinEvent {
                            player: player.clone(),
                            room_config,
                        })));

                        if result.is_err() {
                            Self::try_to_send_output_message(
                                output_tx,
                                protobuf::app::ServerMessage {
                                    data: Some(protobuf::app::server_message::Data::JoinResponse(
                                        protobuf::app::JoinResponse {
                                            room_id: req.room_id,
                                            current_players: Vec::new(),
                                            room_config: None,
                                            error: Some(protobuf::app::Error {
                                                code: protobuf::app::ErrorCode::RoomNotFound as i32,
                                                message: "Room was removed during Join processing"
                                                    .to_string(),
                                            }),
                                        },
                                    )),
                                },
                            );
                        }
                    }
                    protobuf::app::client_message::Data::LeaveRequest(req) => {
                        let room_tx = get_room_channel(&req.room_id).await;
                        match room_tx {
                            Some(room_tx) => {
                                let result =
                                    room_tx.send(InputEvent::Leave(Box::new(InputLeaveEvent {
                                        player_id: player.id.clone(),
                                    })));

                                // RoomがDropしていた場合。
                                // プレイヤー切断タイミング次第ではエラーになることはあり得なくもなさそう。
                                if result.is_err() {
                                    debug!("Player disconnected before leave processing");
                                }
                            }
                            None => {
                                // 存在しないRoomにLeaveを送ったケース。
                                Self::try_to_send_output_message(
                                    output_tx,
                                    protobuf::app::ServerMessage {
                                        data: Some(protobuf::app::server_message::Data::LeaveResponse(
                                            protobuf::app::LeaveResponse {
                                                room_id: req.room_id,
                                                error: Some(protobuf::app::Error {
                                                    code: protobuf::app::ErrorCode::FailedPrecondition
                                                        as i32,
                                                    message: "You have not joined the room or it does not exist".to_string(),
                                                }),
                                            },
                                        )),
                                    }
                                );
                            }
                        };
                    }
                    protobuf::app::client_message::Data::SendMessage(send_message) => {
                        debug!("SendMessage: {:?}", send_message);
                        let event = Box::new(InputMessageEvent {
                            sender_player_id: player.id.clone(),
                            target_ids: send_message.target_ids,
                            body: send_message.body.into(),
                        });
                        let room_tx = get_room_channel(&send_message.room_id).await;
                        match room_tx {
                            Some(room_tx) => {
                                let result = room_tx.send(InputEvent::Message(event));
                                // RoomがDropしていた場合。
                                // プレイヤー切断タイミング次第ではエラーになることはあり得なくもなさそう。
                                if result.is_err() {
                                    debug!("Player disconnected before leave processing");
                                }
                            }
                            None => {
                                // 存在しないRoomにMessageを送ったケース。
                                // 現状SendMessageにはレスポンスを返さずベストエフォートな想定なので無視する。
                                error!("Attempted to send a message to a room that does not exist or is not joined");
                            }
                        };
                    }
                };
            }
        }
    }

    async fn on_output_event(
        event: Option<OutputEvent>,
        output_tx: &mpsc::UnboundedSender<protobuf::app::ServerMessage>,
        player_id: &entity::PlayerId,
        joined_rooms: &mut HashMap<entity::RoomId, mpsc::UnboundedSender<InputEvent>>,
    ) {
        if let Some(event) = event {
            debug!("OutputEvent: {:?}", event);
            match event {
                OutputEvent::Join(event) => {
                    match event {
                        Ok(ev) => {
                            if &ev.player_id == player_id {
                                let room_tx = get_room_channel(&ev.room_id).await;
                                match room_tx {
                                    Some(room_tx) => {
                                        joined_rooms.insert(ev.room_id.clone(), room_tx);
                                        Self::try_to_send_output_message(
                                            output_tx,
                                            protobuf::app::ServerMessage {
                                                data: Some(
                                                    protobuf::app::server_message::Data::JoinResponse(
                                                        protobuf::app::JoinResponse {
                                                            room_id: ev.room_id.clone(),
                                                            current_players: ev.room_player_ids.clone(),
                                                            room_config: Some(protobuf::app::RoomConfig {
                                                                max_players: ev.room_config.max_players,
                                                            }),
                                                            error: Some(protobuf::app::Error {
                                                                code: protobuf::app::ErrorCode::None
                                                                    as i32,
                                                                message: String::new(),
                                                            }),
                                                        },
                                                    ),
                                                ),
                                            },
                                        );
                                    }
                                    None => {
                                        // Joinできた直後にRoomがDropした。
                                        // 切断タイミング次第ではあり得るか。
                                        Self::try_to_send_output_message(
                                            output_tx,
                                            protobuf::app::ServerMessage {
                                                data: Some(
                                                    protobuf::app::server_message::Data::JoinResponse(
                                                        protobuf::app::JoinResponse {
                                                            room_id: ev.room_id.clone(),
                                                            current_players: Vec::new(),
                                                            room_config: None,
                                                            error: Some(protobuf::app::Error {
                                                                code: protobuf::app::ErrorCode::RoomNotFound
                                                                    as i32,
                                                                message: "Room was deleted during Join processing".to_string(),
                                                            }),
                                                        },
                                                    ),
                                                ),
                                            },
                                        );
                                    }
                                };
                            } else {
                                Self::try_to_send_output_message(
                                    output_tx,
                                    protobuf::app::ServerMessage {
                                        data: Some(
                                            protobuf::app::server_message::Data::JoinNotification(
                                                protobuf::app::JoinNotification {
                                                    room_id: ev.room_id.clone(),
                                                    player_id: ev.player_id.clone(),
                                                },
                                            ),
                                        ),
                                    },
                                );
                            }
                        }
                        Err(err) => match err {
                            entity::RoomError::AlreadyJoinedRoom(room_id, _player_id) => {
                                Self::try_to_send_output_message(
                                    output_tx,
                                    protobuf::app::ServerMessage {
                                        data: Some(protobuf::app::server_message::Data::JoinResponse(
                                            protobuf::app::JoinResponse {
                                                room_id,
                                                current_players: Vec::new(),
                                                room_config: None,
                                                error: Some(protobuf::app::Error {
                                                    code: protobuf::app::ErrorCode::AlreadyJoinedTheRoom
                                                        as i32,
                                                    message: "Already joined the room".to_string(),
                                                }),
                                            },
                                        )),
                                    },
                                );
                            }
                            entity::RoomError::RoomConfigDoesNotMatch(room_id, _player_id) => {
                                Self::try_to_send_output_message(
                                    output_tx,
                                    protobuf::app::ServerMessage {
                                        data: Some(
                                            protobuf::app::server_message::Data::JoinResponse(
                                                protobuf::app::JoinResponse {
                                                    room_id,
                                                    current_players: Vec::new(),
                                                    room_config: None,
                                                    error: Some(protobuf::app::Error {
                                                        code: protobuf::app::ErrorCode::RoomConfigDoesNotMatch
                                                            as i32,
                                                        message: "Room config does not match".to_string(),
                                                    }),
                                                },
                                            ),
                                        ),
                                    },
                                );
                            }
                            _ => {
                                unreachable!("invalid error type for OutputEvent::Join");
                            }
                        },
                    }
                }
                OutputEvent::Leave(event) => {
                    if let Ok(ev) = event {
                        if &ev.player_id == player_id {
                            joined_rooms.remove(&ev.room_id);

                            Self::try_to_send_output_message(
                                output_tx,
                                protobuf::app::ServerMessage {
                                    data: Some(protobuf::app::server_message::Data::LeaveResponse(
                                        protobuf::app::LeaveResponse {
                                            room_id: ev.room_id.clone(),
                                            error: Some(protobuf::app::Error {
                                                code: protobuf::app::ErrorCode::None as i32,
                                                message: String::new(),
                                            }),
                                        },
                                    )),
                                },
                            );
                        } else {
                            Self::try_to_send_output_message(
                                output_tx,
                                protobuf::app::ServerMessage {
                                    data: Some(
                                        protobuf::app::server_message::Data::LeaveNotification(
                                            protobuf::app::LeaveNotification {
                                                room_id: ev.room_id.clone(),
                                                player_id: ev.player_id.clone(),
                                            },
                                        ),
                                    ),
                                },
                            );
                        }
                    } else {
                        unreachable!("invalid error type for OutputEvent::Leave");
                    }
                }
                OutputEvent::Message(event) => {
                    Self::try_to_send_output_message(
                        output_tx,
                        protobuf::app::ServerMessage {
                            data: Some(protobuf::app::server_message::Data::MessageNotification(
                                protobuf::app::MessageNotification {
                                    sender_id: event.sender_player_id.clone(),
                                    room_id: event.room_id.clone(),
                                    body: event.body.clone().into(),
                                },
                            )),
                        },
                    );
                }
            }
        }
    }

    fn try_to_send_output_message(
        output_tx: &mpsc::UnboundedSender<protobuf::app::ServerMessage>,
        message: protobuf::app::ServerMessage,
    ) -> bool {
        // output_tx.sendのエラー(output_rxがDrop or closeされている状態)は呼び出し元の次回ループでハンドリングされるので、Resultのハンドリングはしない。
        output_tx.send(message).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use crate::protobuf::app;

    fn default_config() -> Arc<config::Config> {
        Arc::new(config::Config {
            auth: config::Auth {
                enable_bearer: true,
                bearer: "bearer".to_string(),
            },
            tls: config::Tls {
                enable: false,
                cert_file_path: "".to_string(),
                key_file_path: "".to_string(),
            },
        })
    }

    #[tokio::test]
    async fn join_and_room_to_be_removed_after_leave() {
        let config = default_config();
        let room_id = "test".to_string();

        let p1_id = "p1".to_string();
        let p2_id = "p2".to_string();
        let mut p1 = Player::new(config.clone());
        let mut p2 = Player::new(config);

        p1.send(app::ClientMessage {
            data: Some(app::client_message::Data::LoginRequest(app::LoginRequest {
                player_id: p1_id.clone(),
                auth_config: Some(app::login_request::AuthConfig::Bearer(
                    app::AuthConfigBearer {
                        token: "bearer".to_string(),
                    },
                )),
            })),
        })
        .unwrap();

        let data = p1.recv().await.unwrap().data.unwrap();
        if let app::server_message::Data::LoginResponse(res) = data {
            assert_eq!(app::ErrorCode::None as i32, res.error.unwrap().code);
        } else {
            panic!("Unexpected message. {:?}", data);
        }

        p1.send(app::ClientMessage {
            data: Some(app::client_message::Data::JoinRequest(app::JoinRequest {
                room_id: room_id.clone(),
                room_config: Some(app::RoomConfig { max_players: 2 }),
            })),
        })
        .unwrap();

        let data = p1.recv().await.unwrap().data.unwrap();
        if let app::server_message::Data::JoinResponse(res) = data {
            assert_eq!(room_id, res.room_id);
            assert_eq!(app::ErrorCode::None as i32, res.error.unwrap().code);
        } else {
            panic!("Unexpected message. {:?}", data);
        }

        p2.send(app::ClientMessage {
            data: Some(app::client_message::Data::LoginRequest(app::LoginRequest {
                player_id: p2_id.clone(),
                auth_config: Some(app::login_request::AuthConfig::Bearer(
                    app::AuthConfigBearer {
                        token: "bearer".to_string(),
                    },
                )),
            })),
        })
        .unwrap();

        let data = p2.recv().await.unwrap().data.unwrap();
        if let app::server_message::Data::LoginResponse(res) = data {
            assert_eq!(app::ErrorCode::None as i32, res.error.unwrap().code);
        } else {
            panic!("Unexpected message. {:?}", data);
        }

        p2.send(app::ClientMessage {
            data: Some(app::client_message::Data::JoinRequest(app::JoinRequest {
                room_id: room_id.clone(),
                room_config: Some(app::RoomConfig { max_players: 2 }),
            })),
        })
        .unwrap();

        let data = p2.recv().await.unwrap().data.unwrap();
        if let app::server_message::Data::JoinResponse(res) = data {
            assert_eq!(room_id, res.room_id);
            assert_eq!(app::ErrorCode::None as i32, res.error.unwrap().code);
        } else {
            panic!("Unexpected message. {:?}", data);
        }

        let data = p1.recv().await.unwrap().data.unwrap();
        if let app::server_message::Data::JoinNotification(notification) = data {
            assert_eq!(room_id, notification.room_id);
            assert_eq!(p2_id, notification.player_id);
        } else {
            panic!("Unexpected message. {:?}", data);
        }

        p2.send(app::ClientMessage {
            data: Some(app::client_message::Data::LeaveRequest(app::LeaveRequest {
                room_id: room_id.clone(),
            })),
        })
        .unwrap();

        let data = p2.recv().await.unwrap().data.unwrap();
        if let app::server_message::Data::LeaveResponse(res) = data {
            assert_eq!(room_id, res.room_id);
            assert_eq!(app::ErrorCode::None as i32, res.error.unwrap().code);
        } else {
            panic!("Unexpected message. {:?}", data);
        }

        let data = p1.recv().await.unwrap().data.unwrap();
        if let app::server_message::Data::LeaveNotification(notification) = data {
            assert_eq!(room_id, notification.room_id);
            assert_eq!(p2_id, notification.player_id);
        } else {
            panic!("Unexpected message. {:?}", data);
        }

        p1.send(app::ClientMessage {
            data: Some(app::client_message::Data::LeaveRequest(app::LeaveRequest {
                room_id: room_id.clone(),
            })),
        })
        .unwrap();

        let data = p1.recv().await.unwrap().data.unwrap();
        if let app::server_message::Data::LeaveResponse(res) = data {
            assert_eq!(room_id, res.room_id);
            assert_eq!(app::ErrorCode::None as i32, res.error.unwrap().code);
        } else {
            panic!("Unexpected message. {:?}", data);
        }

        // Roomが掃除されていることを確認しておく。
        let room_tx = get_room_channel(&room_id).await;
        assert!(room_tx.is_none());
    }
}
