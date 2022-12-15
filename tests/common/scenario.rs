use uuid::Uuid;

extern crate mini_realtime_server;

use super::*;

pub async fn run_e2e_all<C>()
where
    C: Client,
{
    e2e_normal(generate_random_id(), &mut C::generate(), &mut C::generate()).await;
    e2e_double_login(&mut C::generate(), &mut C::generate()).await;
    e2e_multi_room(
        generate_random_id(),
        generate_random_id(),
        &mut C::generate(),
        &mut C::generate(),
    )
    .await;
}

fn generate_random_id() -> String {
    Uuid::new_v4().to_string()
}

async fn e2e_normal(room_id: String, p1: &mut impl Client, p2: &mut impl Client) {
    let p1_id = generate_random_id();
    let p2_id = generate_random_id();
    p1.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::LoginRequest(
            protobuf::app::LoginRequest {
                player_id: p1_id.clone(),
                auth_config: Some(protobuf::app::login_request::AuthConfig::Bearer(
                    protobuf::app::AuthConfigBearer {
                        token: "bearer".to_string(),
                    },
                )),
            },
        )),
    })
    .unwrap();

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::LoginResponse(res) = data {
        assert_eq!(
            protobuf::app::ErrorCode::None as i32,
            res.error.unwrap().code
        );
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    p1.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::JoinRequest(
            protobuf::app::JoinRequest {
                room_id: room_id.clone(),
                room_config: Some(protobuf::app::RoomConfig {
                    max_players: 2,
                }),
            },
        )),
    })
    .unwrap();

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::JoinResponse(res) = data {
        assert_eq!(room_id, res.room_id);
        assert_eq!(
            protobuf::app::ErrorCode::None as i32,
            res.error.unwrap().code
        );
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    p2.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::LoginRequest(
            protobuf::app::LoginRequest {
                player_id: p2_id.clone(),
                auth_config: Some(protobuf::app::login_request::AuthConfig::Bearer(
                    protobuf::app::AuthConfigBearer {
                        token: "bearer".to_string(),
                    },
                )),
            },
        )),
    })
    .unwrap();

    let data = p2.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::LoginResponse(res) = data {
        assert_eq!(
            protobuf::app::ErrorCode::None as i32,
            res.error.unwrap().code
        );
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    p2.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::JoinRequest(
            protobuf::app::JoinRequest {
                room_id: room_id.clone(),
                room_config: Some(protobuf::app::RoomConfig {
                    max_players: 2,
                }),
            },
        )),
    })
    .unwrap();

    let data = p2.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::JoinResponse(res) = data {
        assert_eq!(room_id, res.room_id);
        assert_eq!(
            protobuf::app::ErrorCode::None as i32,
            res.error.unwrap().code
        );
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::JoinNotification(notification) = data {
        assert_eq!(room_id, notification.room_id);
        assert_eq!(p2_id, notification.player_id);
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    let p1_broadcast_msg = "p1_broadcast_msg".as_bytes().to_vec();
    p1.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::SendMessage(
            protobuf::app::SendMessage {
                room_id: room_id.clone(),
                target_ids: Vec::new(),
                body: p1_broadcast_msg.clone(),
            },
        )),
    })
    .unwrap();

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::MessageNotification(notification) = data {
        assert_eq!(room_id, notification.room_id);
        assert_eq!(p1_id, notification.sender_id);
        assert_eq!(p1_broadcast_msg, notification.body);
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    let data = p2.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::MessageNotification(notification) = data {
        assert_eq!(room_id, notification.room_id);
        assert_eq!(p1_id, notification.sender_id);
        assert_eq!(p1_broadcast_msg, notification.body);
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    let p2_to_p1_msg = "p2_to_p1_msg".as_bytes().to_vec();
    p2.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::SendMessage(
            protobuf::app::SendMessage {
                room_id: room_id.clone(),
                target_ids: vec![p1_id.clone()],
                body: p2_to_p1_msg.clone(),
            },
        )),
    })
    .unwrap();

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::MessageNotification(notification) = data {
        assert_eq!(room_id, notification.room_id);
        assert_eq!(p2_id, notification.sender_id);
        assert_eq!(p2_to_p1_msg, notification.body);
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    // p2にはp2_to_p1_msgは届かない。
    // 別のbroadcastメッセージを送って確認。
    let p2_broadcast_msg = "p2_broadcast_msg".as_bytes().to_vec();
    p2.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::SendMessage(
            protobuf::app::SendMessage {
                room_id: room_id.clone(),
                target_ids: Vec::new(),
                body: p2_broadcast_msg.clone(),
            },
        )),
    })
    .unwrap();

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::MessageNotification(notification) = data {
        assert_eq!(room_id, notification.room_id);
        assert_eq!(p2_id, notification.sender_id);
        assert_eq!(p2_broadcast_msg, notification.body);
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    let data = p2.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::MessageNotification(notification) = data {
        assert_eq!(room_id, notification.room_id);
        assert_eq!(p2_id, notification.sender_id);
        assert_eq!(p2_broadcast_msg, notification.body);
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    p2.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::LeaveRequest(
            protobuf::app::LeaveRequest {
                room_id: room_id.clone(),
            },
        )),
    })
    .unwrap();

    let data = p2.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::LeaveResponse(res) = data {
        assert_eq!(room_id, res.room_id);
        assert_eq!(
            protobuf::app::ErrorCode::None as i32,
            res.error.unwrap().code
        );
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::LeaveNotification(notification) = data {
        assert_eq!(room_id, notification.room_id);
        assert_eq!(p2_id, notification.player_id);
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    p1.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::LeaveRequest(
            protobuf::app::LeaveRequest {
                room_id: room_id.clone(),
            },
        )),
    })
    .unwrap();

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::LeaveResponse(res) = data {
        assert_eq!(room_id, res.room_id);
        assert_eq!(
            protobuf::app::ErrorCode::None as i32,
            res.error.unwrap().code
        );
    } else {
        panic!("Unexpected message. {:?}", data);
    }
}

async fn e2e_double_login(p1: &mut impl Client, p2: &mut impl Client) {
    let p1_id = generate_random_id();
    let p2_id = p1_id.clone();
    p1.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::LoginRequest(
            protobuf::app::LoginRequest {
                player_id: p1_id.clone(),
                auth_config: Some(protobuf::app::login_request::AuthConfig::Bearer(
                    protobuf::app::AuthConfigBearer {
                        token: "bearer".to_string(),
                    },
                )),
            },
        )),
    })
    .unwrap();

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::LoginResponse(res) = data {
        assert_eq!(
            protobuf::app::ErrorCode::None as i32,
            res.error.unwrap().code
        );
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    p2.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::LoginRequest(
            protobuf::app::LoginRequest {
                player_id: p2_id.clone(),
                auth_config: Some(protobuf::app::login_request::AuthConfig::Bearer(
                    protobuf::app::AuthConfigBearer {
                        token: "bearer".to_string(),
                    },
                )),
            },
        )),
    })
    .unwrap();

    let data = p2.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::LoginResponse(res) = data {
        assert_eq!(
            protobuf::app::ErrorCode::AlreadyLoggedIn as i32,
            res.error.unwrap().code
        );
    } else {
        panic!("Unexpected message. {:?}", data);
    }
}

async fn e2e_multi_room(
    room_id_1: String,
    room_id_2: String,
    p1: &mut impl Client,
    p2: &mut impl Client,
) {
    let p1_id = generate_random_id();
    let p2_id = generate_random_id();
    p1.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::LoginRequest(
            protobuf::app::LoginRequest {
                player_id: p1_id.clone(),
                auth_config: Some(protobuf::app::login_request::AuthConfig::Bearer(
                    protobuf::app::AuthConfigBearer {
                        token: "bearer".to_string(),
                    },
                )),
            },
        )),
    })
    .unwrap();

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::LoginResponse(res) = data {
        assert_eq!(
            protobuf::app::ErrorCode::None as i32,
            res.error.unwrap().code
        );
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    p1.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::JoinRequest(
            protobuf::app::JoinRequest {
                room_id: room_id_1.clone(),
                room_config: Some(protobuf::app::RoomConfig {
                    max_players: 2,
                }),
            },
        )),
    })
    .unwrap();

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::JoinResponse(res) = data {
        assert_eq!(room_id_1, res.room_id);
        assert_eq!(
            protobuf::app::ErrorCode::None as i32,
            res.error.unwrap().code
        );
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    p1.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::JoinRequest(
            protobuf::app::JoinRequest {
                room_id: room_id_2.clone(),
                room_config: Some(protobuf::app::RoomConfig {
                    max_players: 2,
                }),
            },
        )),
    })
    .unwrap();

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::JoinResponse(res) = data {
        assert_eq!(room_id_2, res.room_id);
        assert_eq!(
            protobuf::app::ErrorCode::None as i32,
            res.error.unwrap().code
        );
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    p2.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::LoginRequest(
            protobuf::app::LoginRequest {
                player_id: p2_id.clone(),
                auth_config: Some(protobuf::app::login_request::AuthConfig::Bearer(
                    protobuf::app::AuthConfigBearer {
                        token: "bearer".to_string(),
                    },
                )),
            },
        )),
    })
    .unwrap();

    let data = p2.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::LoginResponse(res) = data {
        assert_eq!(
            protobuf::app::ErrorCode::None as i32,
            res.error.unwrap().code
        );
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    p2.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::JoinRequest(
            protobuf::app::JoinRequest {
                room_id: room_id_1.clone(),
                room_config: Some(protobuf::app::RoomConfig {
                    max_players: 2,
                }),
            },
        )),
    })
    .unwrap();

    let data = p2.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::JoinResponse(res) = data {
        assert_eq!(room_id_1, res.room_id);
        assert_eq!(
            protobuf::app::ErrorCode::None as i32,
            res.error.unwrap().code
        );
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    p2.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::JoinRequest(
            protobuf::app::JoinRequest {
                room_id: room_id_2.clone(),
                room_config: Some(protobuf::app::RoomConfig {
                    max_players: 2,
                }),
            },
        )),
    })
    .unwrap();

    let data = p2.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::JoinResponse(res) = data {
        assert_eq!(room_id_2, res.room_id);
        assert_eq!(
            protobuf::app::ErrorCode::None as i32,
            res.error.unwrap().code
        );
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::JoinNotification(notification) = data {
        assert_eq!(room_id_1, notification.room_id);
        assert_eq!(p2_id, notification.player_id);
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::JoinNotification(notification) = data {
        assert_eq!(room_id_2, notification.room_id);
        assert_eq!(p2_id, notification.player_id);
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    // room_id_1へのメッセージ
    let p1_broadcast_msg = "p1_broadcast_msg_room1".as_bytes().to_vec();
    p1.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::SendMessage(
            protobuf::app::SendMessage {
                room_id: room_id_1.clone(),
                target_ids: Vec::new(),
                body: p1_broadcast_msg.clone(),
            },
        )),
    })
    .unwrap();

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::MessageNotification(notification) = data {
        assert_eq!(room_id_1, notification.room_id);
        assert_eq!(p1_id, notification.sender_id);
        assert_eq!(p1_broadcast_msg, notification.body);
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    let data = p2.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::MessageNotification(notification) = data {
        assert_eq!(room_id_1, notification.room_id);
        assert_eq!(p1_id, notification.sender_id);
        assert_eq!(p1_broadcast_msg, notification.body);
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    // room_id_2へのメッセージ
    let p1_broadcast_msg = "p1_broadcast_msg_room2".as_bytes().to_vec();
    p1.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::SendMessage(
            protobuf::app::SendMessage {
                room_id: room_id_2.clone(),
                target_ids: Vec::new(),
                body: p1_broadcast_msg.clone(),
            },
        )),
    })
    .unwrap();

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::MessageNotification(notification) = data {
        assert_eq!(room_id_2, notification.room_id);
        assert_eq!(p1_id, notification.sender_id);
        assert_eq!(p1_broadcast_msg, notification.body);
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    let data = p2.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::MessageNotification(notification) = data {
        assert_eq!(room_id_2, notification.room_id);
        assert_eq!(p1_id, notification.sender_id);
        assert_eq!(p1_broadcast_msg, notification.body);
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    // p2をroom_id_1からLeaveさせる
    p2.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::LeaveRequest(
            protobuf::app::LeaveRequest {
                room_id: room_id_1.clone(),
            },
        )),
    })
    .unwrap();

    let data = p2.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::LeaveResponse(res) = data {
        assert_eq!(room_id_1, res.room_id);
        assert_eq!(
            protobuf::app::ErrorCode::None as i32,
            res.error.unwrap().code
        );
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::LeaveNotification(notification) = data {
        assert_eq!(room_id_1, notification.room_id);
        assert_eq!(p2_id, notification.player_id);
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    // 再びroom_id_2へのメッセージ。Leaveしていないので両プレイヤーに届くはず。
    let p1_broadcast_msg = "p1_broadcast_msg_room2".as_bytes().to_vec();
    p1.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::SendMessage(
            protobuf::app::SendMessage {
                room_id: room_id_2.clone(),
                target_ids: Vec::new(),
                body: p1_broadcast_msg.clone(),
            },
        )),
    })
    .unwrap();

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::MessageNotification(notification) = data {
        assert_eq!(room_id_2, notification.room_id);
        assert_eq!(p1_id, notification.sender_id);
        assert_eq!(p1_broadcast_msg, notification.body);
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    let data = p2.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::MessageNotification(notification) = data {
        assert_eq!(room_id_2, notification.room_id);
        assert_eq!(p1_id, notification.sender_id);
        assert_eq!(p1_broadcast_msg, notification.body);
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    // あとは全RoomからLeaveする。
    p1.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::LeaveRequest(
            protobuf::app::LeaveRequest {
                room_id: room_id_1.clone(),
            },
        )),
    })
    .unwrap();

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::LeaveResponse(res) = data {
        assert_eq!(room_id_1, res.room_id);
        assert_eq!(
            protobuf::app::ErrorCode::None as i32,
            res.error.unwrap().code
        );
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    p2.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::LeaveRequest(
            protobuf::app::LeaveRequest {
                room_id: room_id_2.clone(),
            },
        )),
    })
    .unwrap();

    let data = p2.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::LeaveResponse(res) = data {
        assert_eq!(room_id_2, res.room_id);
        assert_eq!(
            protobuf::app::ErrorCode::None as i32,
            res.error.unwrap().code
        );
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::LeaveNotification(notification) = data {
        assert_eq!(room_id_2, notification.room_id);
        assert_eq!(p2_id, notification.player_id);
    } else {
        panic!("Unexpected message. {:?}", data);
    }

    p1.send(protobuf::app::ClientMessage {
        data: Some(protobuf::app::client_message::Data::LeaveRequest(
            protobuf::app::LeaveRequest {
                room_id: room_id_2.clone(),
            },
        )),
    })
    .unwrap();

    let data = p1.recv().await.unwrap().data.unwrap();
    if let protobuf::app::server_message::Data::LeaveResponse(res) = data {
        assert_eq!(room_id_2, res.room_id);
        assert_eq!(
            protobuf::app::ErrorCode::None as i32,
            res.error.unwrap().code
        );
    } else {
        panic!("Unexpected message. {:?}", data);
    }
}
