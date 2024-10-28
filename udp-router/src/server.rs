use std::net::SocketAddr;

use crate::bpf_actor::BpfActorHandle;
use udp_router_protobuf::management::router_service_server::{RouterService, RouterServiceServer};
use udp_router_protobuf::management::{
    GetStatsRequest, GetStatsResponse, SetLocalNetAndMaskRequest, SetBackendNetAndMaskRequest
};
use tokio::net::TcpListener;
use tonic::{transport::Server, Request, Response, Status};

#[derive(Debug, Clone)]
pub struct ManagementServer {
    bpf: BpfActorHandle,
    bind_address: String,
    port: u32,
}

impl ManagementServer {
    pub fn new(bpf: BpfActorHandle, bind_address: &str, port: u32) -> Self {
        ManagementServer {
            bpf,
            bind_address: bind_address.to_string(),
            port,
        }
    }

    pub async fn start(&self) {
        let addr: SocketAddr = format!("{}:{}", self.bind_address, self.port).parse().unwrap();
        let sock = socket2::Socket::new(
            match addr {
                SocketAddr::V4(_) => socket2::Domain::IPV4,
                SocketAddr::V6(_) => socket2::Domain::IPV6,
            },
            socket2::Type::STREAM,
            None,
        )
        .unwrap();

        sock.set_reuse_address(true).unwrap();
        sock.set_reuse_port(true).unwrap();
        sock.set_nonblocking(true).unwrap();
        sock.bind(&addr.into()).unwrap();
        sock.listen(1024).unwrap();

        let incoming = tokio_stream::wrappers::TcpListenerStream::new(TcpListener::from_std(sock.into()).unwrap());

        Server::builder()
            .add_service(RouterServiceServer::new(self.clone()))
            .serve_with_incoming(incoming)
            .await
            .unwrap();
    }
}

#[tonic::async_trait]
impl RouterService for ManagementServer {
    async fn get_stats(&self, _req: Request<GetStatsRequest>) -> Result<Response<GetStatsResponse>, Status> {
        let stats = self.bpf.get_router_stats().await;

        Ok(Response::new(GetStatsResponse {
            total_packets: stats.total_packets,
            total_client_to_server: stats.total_client_to_server_packets,
            total_server_to_client: stats.total_server_to_client_packets,
        }))
    }

    async fn set_local_net_and_mask(&self, req: Request<SetLocalNetAndMaskRequest>) -> Result<Response<()>, Status> {
        let req = req.into_inner();
        self.bpf.set_local_net_mask(req.net, req.mask).await;
        Ok(Response::new(()))
    }

    async fn set_backend_net_and_mask(&self, req: Request<SetBackendNetAndMaskRequest>) -> Result<Response<()>, Status> {
        let req = req.into_inner();
        self.bpf.set_backend_net_mask(req.net, req.mask).await;
        Ok(Response::new(()))
    }
}
