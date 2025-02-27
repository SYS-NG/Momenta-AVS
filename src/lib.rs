pub mod context;

use blueprint_sdk::alloy::primitives::{address, Address, U256};
use blueprint_sdk::alloy::rpc::types::Log;
use blueprint_sdk::alloy::sol;
use blueprint_sdk::config::GadgetConfiguration;
use blueprint_sdk::event_listeners::evm::EvmContractEventListener;
use blueprint_sdk::job;
use blueprint_sdk::macros::load_abi;
use blueprint_sdk::std::sync::LazyLock;
use serde::{Deserialize, Serialize};
use bollard::Docker;
use bollard::container::{CreateContainerOptions, Config, StartContainerOptions, RemoveContainerOptions};
use std::sync::Arc;
use crate::context::{DockerManager, MomentaAvsContext};
use blueprint_sdk::crypto::k256::K256Ecdsa;
use serde_json::Value;
use blueprint_sdk::contexts::keystore::KeystoreContext;
use blueprint_sdk::keystore::backends::Backend;
use blueprint_sdk::keystore::backends::eigenlayer::EigenlayerBackend;
use blueprint_sdk::utils::evm::get_wallet_provider_http;
use alloy_network::EthereumWallet;
use alloy_signer_local::LocalSigner;
use alloy_primitives::FixedBytes;

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
    inference_result: Option<String>,
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
pub async fn inference(
    context: MomentaAvsContext,
    filepath: String,
) -> Result<String, Box<dyn std::error::Error>> {

    // Use the dynamically assigned host port for the inference container.
    let host_port = &context.docker_manager.checker_container.host_port;
    let url = format!("http://localhost:{}/process-audio", host_port);
    blueprint_sdk::logging::debug!("Sending GET request to: {}", url);

    // Send GET request to the container.
    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;
    blueprint_sdk::logging::debug!("After GET request");

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_else(|_| "Unable to read body".to_string());
        blueprint_sdk::logging::error!("Request failed: {}. Body: {}", status, body);
        return Err(format!("Request failed with status: {}", status).into());
    }

    // Read and log the raw response.
    let bytes = response.bytes().await?;
    let body = String::from_utf8_lossy(&bytes);
    blueprint_sdk::logging::debug!("Response Body: {}", body);

    // Deserialize JSON into the new InferenceResponse structure.
    let inference_response: InferenceResponse = serde_json::from_slice(&bytes)?;

    // Iterate through the results and handle any error statuses.
    for result_item in &inference_response.results {
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
        blueprint_sdk::logging::debug!("No files to process");
        return Ok(format!("No files to process"));
    }

    // Process successful inference results and submit to blockchain
    for result_item in &inference_response.results {
        if result_item.status.to_lowercase() == "success" && result_item.inference_result.is_some() {
            // Parse the inference_result JSON string
            let inference_result: Value = serde_json::from_str(
                result_item.inference_result.as_ref().unwrap()
            )?;
            
            // Extract the prediction details
            let file = inference_result["file"].as_str().unwrap_or("unknown");
            let prediction = inference_result["prediction"].as_str().unwrap_or("unknown");
            let confidence = inference_result["confidence"].as_f64().unwrap_or(0.0);
            
            blueprint_sdk::logging::info!(
                "\n=== BLOCKCHAIN SUBMISSION ===\nFile: {}\nPrediction: {}\nConfidence: {:.4}%\n===========================",
                file, prediction, confidence * 100.0
            );
            
            // Get the ECDSA key and provider - using the correct API methods with traits in scope
            let keystore = context.config.keystore();
            let ecdsa_keys = keystore.list_local::<K256Ecdsa>().unwrap_or_default();
            
            if ecdsa_keys.is_empty() {
                blueprint_sdk::logging::error!("No ECDSA keys found in keystore");
                continue;
            }
            
            let ecdsa_pubkey = &ecdsa_keys[0];
            let signing_key = keystore.expose_ecdsa_secret(ecdsa_pubkey).unwrap().unwrap();
            
            // DEVELOPMENT ONLY: Create a mock private key
            // WARNING: Never use hardcoded private keys in production!
            
            // Create wallet and provider
            let wallet = EthereumWallet::new(LocalSigner::from_bytes(&FixedBytes::from([0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1])).unwrap());
            let provider = get_wallet_provider_http(&context.config.http_rpc_endpoint, wallet);
            
            // Create contract instance with the provider
            let contract = TangleTaskManager::new(*TASK_MANAGER_ADDRESS, provider);
            
            // Convert confidence to a uint256 value (multiply by 1e18 to preserve precision)
            let confidence_scaled = (confidence * 1_000_000_000_000_000_000.0) as u128;
            let confidence_u256 = U256::from(confidence_scaled);
            
            // Submit the result to the blockchain
            let tx = contract
                .recordInferenceResult(
                    file.to_string().into(),
                    prediction.to_string().into(),
                    confidence_u256
                );
                
            // Send the transaction and get the transaction hash
            let tx_hash = tx.send().await?;
            
            blueprint_sdk::logging::info!(
                // Keep this commented out
                // "Transaction submitted with hash: {:?}",
                // tx_hash // Print the PendingTransactionBuilderObject
                "✅ Transaction submitted: {:#x}",
                tx_hash.tx_hash()
            );
        }
    }

    Ok(format!(
        "Processed files: {}",
        inference_response.processed_files,
    ))
}

/// Pre-processor for handling inbound events and extracting file paths
async fn task_pre_processor(
    (event, _log): (TangleTaskManager::NewTaskCreated, Log),
) -> Result<Option<(String,)>, ProcessorError> {
    // Debug print to see what fields are available
    blueprint_sdk::logging::debug!("Processing task: {:?}", event);
    
    // Extract filepath from the dedicated filepath field instead of quorum_numbers
    let filepath = match String::from_utf8(event.task.filepath.to_vec()) {
        Ok(path) => {
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
        let result = inference(context, "".into()).await.unwrap();
        assert!(result.contains("Processed files:"));
    }
}
