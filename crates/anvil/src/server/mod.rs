//! Contains the code to launch an Ethereum RPC server.

use crate::EthApi;
use anvil_server::{ipc::IpcEndpoint, ServerConfig};
use axum::Router;
use futures::StreamExt;
use handler::{HttpEthRpcHandler, PubSubEthRpcHandler};
use std::{
    future::{Future, IntoFuture},
    net::SocketAddr,
    pin::pin,
};
use tokio::{io, task::JoinHandle};

pub mod error;
mod handler;

/// Configures a server that handles [`EthApi`] related JSON-RPC calls via HTTP and WS.
///
/// The returned future creates a new server, binding it to the given address, which returns another
/// future that runs it.
pub async fn serve(
    addr: SocketAddr,
    api: EthApi,
    config: ServerConfig,
) -> io::Result<impl Future<Output = io::Result<()>>> {
    tokio::net::TcpListener::bind(addr).await.map(|tcp_listener| {
        axum::serve(tcp_listener, router(api, config).into_make_service()).into_future()
    })
}

/// Configures an [`axum::Router`] that handles [`EthApi`] related JSON-RPC calls via HTTP and WS.
pub fn router(api: EthApi, config: ServerConfig) -> Router {
    let http = HttpEthRpcHandler::new(api.clone());
    let ws = PubSubEthRpcHandler::new(api);
    anvil_server::http_ws_router(config, http, ws)
}

/// Launches an ipc server at the given path in a new task
///
/// # Panics
///
/// Panics if setting up the IPC connection was unsuccessful.
#[track_caller]
pub fn spawn_ipc(api: EthApi, path: String) -> JoinHandle<io::Result<()>> {
    try_spawn_ipc(api, path).expect("failed to establish ipc connection")
}

/// Launches an ipc server at the given path in a new task.
pub fn try_spawn_ipc(api: EthApi, path: String) -> io::Result<JoinHandle<io::Result<()>>> {
    let handler = PubSubEthRpcHandler::new(api);
    let ipc = IpcEndpoint::new(handler, path);
    let incoming = ipc.incoming()?;

    let task = tokio::task::spawn(async move {
        let mut incoming = pin!(incoming);
        while let Some(stream) = incoming.next().await {
            trace!(target: "ipc", "new ipc connection");
            tokio::task::spawn(stream);
        }
        Ok(())
    });

    Ok(task)
}
