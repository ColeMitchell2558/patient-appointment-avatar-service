use patient_avatar_service::{
    appointment_avatar::{router, AvatarState},
    infrai_client::InfraiClient,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = AvatarState {
        infrai: InfraiClient::from_env()?,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
    println!("patient avatar service listening on http://127.0.0.1:8080");
    axum::serve(listener, router(state)).await?;
    Ok(())
}

