use momenta_avs as blueprint;
use blueprint::{TangleTaskManager, TASK_MANAGER_ADDRESS};
use blueprint_sdk::alloy::primitives::{address, Address, U256};
use blueprint_sdk::logging::{info, warn};
use blueprint_sdk::macros::main;
use blueprint_sdk::runners::core::runner::BlueprintRunner;
use blueprint_sdk::runners::eigenlayer::bls::EigenlayerBLSConfig;
use blueprint_sdk::utils::evm::get_provider_http;
use blueprint::context::{DockerManager, MomentaAvsContext};
use std::sync::Arc;

#[main(env)]
async fn main() {
    // Initialize Docker manager
    let docker_manager = Arc::new(
        DockerManager::new()
            .await
            .expect("Failed to initialize Docker manager")
    );

    // Create your service context
    let context = MomentaAvsContext {
        config: env.clone(),
        docker_manager: docker_manager.clone(),
    };

    // Get the provider
    let rpc_endpoint = env.http_rpc_endpoint.clone();
    let provider = get_provider_http(&rpc_endpoint);

    // Create an instance of your task manager
    let contract = TangleTaskManager::new(*TASK_MANAGER_ADDRESS, provider);

    // Create the event handler from the job
    let inference_job = blueprint::InferenceFromFilepathEventHandler::new(contract, context.clone());

    // Optional: Replace the test task creation with a mechanism to handle file path requests
    // This could be a REST API, a command-line interface, or another mechanism
    // For this example, we'll create a simple task with a filepath embedded in the task data
    info!("Spawning a task to create inference tasks when needed...");
    blueprint_sdk::tokio::spawn(async move {
        let provider = get_provider_http(&rpc_endpoint);
        let contract = TangleTaskManager::new(*TASK_MANAGER_ADDRESS, provider);
        
        // Example file paths to process - in a real application, these would come from user requests
        let file_paths = vec![
            "/home/szeyu/code/momenta-avs/p270_306.wav",
            "/home/szeyu/code/momenta-avs/p227_387.wav",
            "/home/szeyu/code/momenta-avs/natural_p227_387.wav"
        ];
        
        for (index, file_path) in file_paths.iter().enumerate() {
            blueprint_sdk::tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            
            // Convert the file path to bytes for storage
            let filepath_bytes = file_path.as_bytes().to_vec();
            
            // Sample quorum numbers - typically used for consensus
            // In a real implementation, this would be properly configured
            let quorum_numbers = vec![0]; // Example quorum numbers
            
            // Log what we're sending
            info!("Creating task with filepath: {}", file_path);
            
            // Create a task with the file path as a separate parameter
            let task = contract
                .createNewTask(
                    U256::from(index),                // task identifier
                    100u32,                          // quorum threshold percentage
                    quorum_numbers.into(),           // actual quorum numbers
                    filepath_bytes.into()            // dedicated filepath parameter
                )
                .from(address!("15d34AAf54267DB7D7c367839AAf71A00a2C6A65"));
                
            let receipt = task.send().await.unwrap().get_receipt().await.unwrap();
            if receipt.status() {
                info!("Inference task created for file: {}", file_path);
            } else {
                warn!("Inference task creation failed for file: {}", file_path);
            }
        }
    });

    info!("Starting the event watcher for inference tasks...");
    let eigen_config = EigenlayerBLSConfig::new(Address::default(), Address::default());
    BlueprintRunner::new(eigen_config, env)
        .job(inference_job)
        .run()
        .await?;

    // Cleanup Docker container on shutdown
    blueprint_sdk::tokio::spawn(async move {
        if let Err(e) = docker_manager.cleanup().await {
            warn!("Failed to cleanup Docker container: {}", e);
        }
    });

    info!("Exiting...");
    Ok(())
}
