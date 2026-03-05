use tokio::task::spawn_blocking;
use crate::Error;

pub async fn chain(buffer: Vec<u8>, operations: Vec<saulimages::Operation>) -> Result<Vec<u8>, Error> {
    spawn_blocking(move || {
        saulimages::chain(buffer, operations).map_err(|e| e.into())
    }).await?
}

pub async fn caption(buffer: Vec<u8>, caption: String) -> Result<Vec<u8>, Error> {
    spawn_blocking(move || {
        saulimages::caption(buffer, &caption).map_err(|e| e.into())
    }).await?
}

pub async fn pugsley(buffer: Vec<u8>) -> Result<Vec<u8>, Error> {
    spawn_blocking(move || {
        saulimages::pugsley(buffer).map_err(|e| e.into())
    }).await?
}

pub async fn rio_de_janeiro(buffer: Vec<u8>) -> Result<Vec<u8>, Error> {
    spawn_blocking(move || {
        saulimages::rio_de_janeiro(buffer).map_err(|e| e.into())
    }).await?
}

pub async fn papyrus(buffer: Vec<u8>, caption: String) -> Result<Vec<u8>, Error> {
    spawn_blocking(move || {
        saulimages::papyrus(buffer, &caption).map_err(|e| e.into())
    }).await?
}

pub async fn burn(buffer: Vec<u8>) -> Result<Vec<u8>, Error> {
    spawn_blocking(move || {
        saulimages::burn(buffer).map_err(|e| e.into())
    }).await?
}