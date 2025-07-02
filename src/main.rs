
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	// let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3000));
	// let hello_service = MyHelloService::default();

    // tokio::spawn(async move {
    //     let mut client = HelloServiceClient::connect("https://localhost:3000").await.unwrap();
        
        
    //     loop {
    //         sleep(Duration::from_secs(1)).await;
    //         let request = tonic::Request::new(HelloRequest {
    //             name: "World".into(),
    //         });

    //         let response = client.say_hello(request).await.unwrap();

    //         println!("Response={response:?}");

    //     }
    // });

	// Server::builder()
	// 	.add_service(HelloServiceServer::new(hello_service))
	// 	.serve(addr)
	// 	.await?;

	Ok(())
}
