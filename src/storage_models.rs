/// Models for volume related requests and responses

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ErrorResponse {
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VolumeCreateRequest {
    pub size: i32, // in MB
    pub parent: Option<Uuid>,
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VolumeCreateResponse {
    pub volume: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VolumeDeleteRequest {
    pub volume: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VolumeDeleteResponse {
    pub volume: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VolumeAttachRequest {
    pub volume: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VolumeAttachResponse {
    pub volume: Uuid,
    pub device: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VolumeDetachRequest {
    pub volume: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VolumeDetachResponse {
    pub volume: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VolumeDetailRequest {
    pub volume: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SnapshotCreateRequest {
    pub volume: Uuid,
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct  SnapshotCreateResponse {
    pub volume: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SnapshotDeleteRequest {
    pub volume: Uuid,
    pub snapshot: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SnapshotListRequest {
    pub volume: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SnapshotListResponse {
    pub snapshots: Vec<Uuid>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SnapshotRollbackRequest {
    pub volume: Uuid,
    pub snapshot: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImagePublishRequest {
    pub volume: Uuid,
    pub snapshot: Uuid
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImagePublishResponse {
    pub volume: Uuid,
    pub snapshot: Uuid,
    pub image: Uuid,
}