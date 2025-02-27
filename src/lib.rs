pub mod context;

use blueprint_sdk::alloy::primitives::{address, Address};
use blueprint_sdk::alloy::rpc::types::Log;
use blueprint_sdk::alloy::sol;
use blueprint_sdk::config::GadgetConfiguration;
use blueprint_sdk::event_listeners::evm::EvmContractEventListener;
use blueprint_sdk::job;
use blueprint_sdk::macros::load_abi;
use blueprint_sdk::std::convert::Infallible;
use blueprint_sdk::std::sync::LazyLock;
use serde::{Deserialize, Serialize};
use bollard::Docker;
use bollard::container::{CreateContainerOptions, Config, StartContainerOptions, RemoveContainerOptions};
use rand;
use reqwest::multipart::{Form, Part};
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use std::sync::Arc;
use crate::context::{DockerManager, MomentaAvsContext};

type ProcessorError =
    blueprint_sdk::event_listeners::core::Error<blueprint_sdk::event_listeners::evm::error::Error>;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    #[derive(Debug, Serialize, Deserialize)]
    TangleTaskManager,
    "contracts/out/TangleTaskManager.sol/TangleTaskManager.json"
);

load_abi!(
    TANGLE_TASK_MANAGER_ABI_STRING,
    "contracts/out/TangleTaskManager.sol/TangleTaskManager.json"
);

pub static TASK_MANAGER_ADDRESS: LazyLock<Address> = LazyLock::new(|| {
    std::env::var("TASK_MANAGER_ADDRESS")
        .map(|addr| addr.parse().expect("Invalid TASK_MANAGER_ADDRESS"))
        .unwrap_or_else(|_| address!("0000000000000000000000000000000000000000"))
});

// #[derive(Deserialize)]
// struct InferenceResponse {
//     file: String,
//     prediction: String,
//     confidence: f64,
//     error: Option<String>,
// }

#[derive(Deserialize)]
struct InferenceResponse {
    processed_files: u32,
    results: Vec<InferenceResultItem>,
}

#[derive(Deserialize)]
struct InferenceResultItem {
    url: String,
    status: String,
    message: Option<String>,
}

#[job(
    id = 0,
    params(filepath),
    event_listener(
        listener = EvmContractEventListener<MomentaAvsContext, TangleTaskManager::NewTaskCreated>,
        instance = TangleTaskManager,
        abi = TANGLE_TASK_MANAGER_ABI_STRING,
        pre_processor = task_pre_processor,
    ),
)]
pub async fn inference_from_filepath(
    context: MomentaAvsContext,
    filepath: String,
) -> Result<String, Box<dyn std::error::Error>> {
    blueprint_sdk::logging::info!("Starting audio inference for file: {}", filepath);

    // Read the WAV file from the provided filepath.
    let file_path = Path::new(&filepath);
    blueprint_sdk::logging::info!("Opening file: {:?}", file_path);
    let mut file = File::open(file_path).await?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await?;
    blueprint_sdk::logging::info!("File read successfully: {} bytes", buffer.len());

    // Create multipart form.
    let part = Part::bytes(buffer)
        .file_name(file_path.file_name().unwrap().to_string_lossy().to_string())
        .mime_str("audio/wav")?;
    let form = Form::new().part("file", part);

    // Use the dynamically assigned host port for the inference container.
    let host_port = &context.docker_manager.checker_container.host_port;
    let url = format!("http://localhost:{}/process-audio", host_port);
    blueprint_sdk::logging::info!("Sending GET request to: {}", url);

    // Send GET request to the container.
    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;
    blueprint_sdk::logging::info!("After GET request");

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_else(|_| "Unable to read body".to_string());
        blueprint_sdk::logging::error!("Request failed: {}. Body: {}", status, body);
        return Err(format!("Request failed with status: {}", status).into());
    }

    // Read and log the raw response.
    let bytes = response.bytes().await?;
    let body = String::from_utf8_lossy(&bytes);
    blueprint_sdk::logging::info!("Response Body: {}", body);

    // Deserialize JSON into the new InferenceResponse structure.
    let inference_response: InferenceResponse = serde_json::from_slice(&bytes)?;
    blueprint_sdk::logging::info!("Processed files: {}", inference_response.processed_files);

    // Iterate through the results and handle any error statuses.
    for result_item in &inference_response.results {
        blueprint_sdk::logging::info!(
            "Inference result for {}: Status: {}, Message: {}",
            result_item.url,
            result_item.status,
            result_item.message.as_deref().unwrap_or("None")
        );
        if result_item.status.to_lowercase() == "error" {
            blueprint_sdk::logging::error!(
                "Inference error for {}: {}",
                result_item.url,
                result_item.message.as_deref().unwrap_or("Unknown error")
            );
            return Err(format!(
                "Inference error for {}: {}",
                result_item.url,
                result_item.message.as_deref().unwrap_or("Unknown error")
            )
            .into());
        }
    }

    // If no inference results are returned, handle gracefully.
    if inference_response.results.is_empty() {
        blueprint_sdk::logging::info!("No files to process");
        return Ok(format!("No files to process"));
    }

    // For demonstration, we use the first result to generate the final message.
    let first_result = inference_response.results.first().unwrap();
    Ok(format!(
        "Processed files: {}",
        inference_response.processed_files,
    ))
}

// #[job(
//     id = 0,
//     params(filepath),
//     event_listener(
//         listener = EvmContractEventListener<MomentaAvsContext, TangleTaskManager::NewTaskCreated>,
//         instance = TangleTaskManager,
//         abi = TANGLE_TASK_MANAGER_ABI_STRING,
//         pre_processor = task_pre_processor,
//     ),
// )]
// pub async fn inference_from_filepath(context: MomentaAvsContext, filepath: String) -> Result<String, Box<dyn std::error::Error>> {
//     blueprint_sdk::logging::info!("Processing audio inference for file: {}", filepath);
    
//     // Read the WAV file from the provided filepath
//     let file_path = Path::new(&filepath);
//     let mut file = File::open(file_path).await?;
//     let mut buffer = Vec::new();
//     file.read_to_end(&mut buffer).await?;
    
//     // Create multipart form
//     let part = Part::bytes(buffer)
//         .file_name(file_path.file_name().unwrap().to_string_lossy().to_string())
//         .mime_str("audio/wav")?;
//     let form = Form::new().part("file", part);
    
//     // Use the dynamically assigned host port for the inference container
//     let host_port = &context.docker_manager.inference_container.host_port;
    
//     // Send POST request to the container using the dynamic port
//     let client = reqwest::Client::new();
//     let response = client
//         .post(&format!("http://localhost:{}/infer", host_port))
//         .multipart(form)
//         .send()
//         .await?;
    
//     if !response.status().is_success() {
//         return Err(format!("Request failed with status: {}", response.status()).into());
//     }
    
//     let inference_result: InferenceResponse = response.json().await?;
    
//     blueprint_sdk::logging::info!(
//         "Inference result - File: {}, Prediction: {}, Confidence: {}", 
//         inference_result.file, 
//         inference_result.prediction, 
//         inference_result.confidence
//     );
    
//     if let Some(error) = inference_result.error {
//         return Err(format!("Inference error: {}", error).into());
//     }
    
//     Ok(format!(
//         "Processed file {}. Result: {} (confidence: {:.2})",
//         file_path.file_name().unwrap().to_string_lossy(),
//         inference_result.prediction,
//         inference_result.confidence
//     ))
// }

/// Pre-processor for handling inbound events and extracting file paths
async fn task_pre_processor(
    (event, _log): (TangleTaskManager::NewTaskCreated, Log),
) -> Result<Option<(String,)>, ProcessorError> {
    // Debug print to see what fields are available
    blueprint_sdk::logging::info!("Processing task: {:?}", event);
    
    // Extract filepath from the dedicated filepath field instead of quorum_numbers
    let filepath = match String::from_utf8(event.task.filepath.to_vec()) {
        Ok(path) => {
            blueprint_sdk::logging::info!("Successfully extracted filepath: {}", path);
            path
        },
        Err(e) => {
            blueprint_sdk::logging::warn!("Failed to decode filepath from bytes: {}. Using default path.", e);
            "/home/szeyu/code/momenta-avs/p270_306.wav".to_string()
        }
    };
    
    Ok(Some((filepath,)))
}

// async fn cleanup_container(docker: &Docker, container_id: &str) -> Result<(), Box<dyn std::error::Error>> {
//     docker.remove_container(
//         container_id,
//         Some(RemoveContainerOptions {
//             force: true,
//             ..Default::default()
//         }),
//     ).await?;
//     Ok(())
// }

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn it_works() {
        let config = GadgetConfiguration::default();
        let docker_manager = Arc::new(
            DockerManager::new()
                .await
                .expect("Failed to initialize Docker manager")
        );
        let context = MomentaAvsContext { config, docker_manager };
        let result = inference_from_filepath(context, "/home/szeyu/code/momenta-avs/p270_306.wav".into()).await.unwrap();
        assert!(result.contains("Result:"));
    }
}
