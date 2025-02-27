use blueprint_sdk::config::GadgetConfiguration;
use bollard::Docker;
use bollard::container::{CreateContainerOptions, Config, StartContainerOptions, RemoveContainerOptions};
use bollard::image::CreateImageOptions;
use bollard::models::{HostConfig, PortBinding};
use futures_util::stream::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use uuid;

pub struct ContainerInfo {
    pub container_id: String,
    pub host_port: String,
}

pub struct DockerManager {
    pub docker: Docker,
    pub inference_container: ContainerInfo,
    pub checker_container: ContainerInfo,
}

impl DockerManager {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let docker = Docker::connect_with_local_defaults()?;
        
        // Pull and start the inference container
        let inference_container = Self::setup_container(
            &docker,
            "stevenmomenta/pytorch-audio-inference:latest",
            "5000",
            "avs-inference"
        ).await?;

        // Pull and start the checker container
        let checker_container = Self::setup_container(
            &docker,
            "stevenmomenta/audio-checking-docker:latest",
            "5009",
            "avs-checker"
        ).await?;

        Ok(Self {
            docker,
            inference_container,
            checker_container,
        })
    }

    async fn setup_container(
        docker: &Docker,
        image: &str,
        container_port: &str,
        prefix: &str,
    ) -> Result<ContainerInfo, Box<dyn std::error::Error>> {
        blueprint_sdk::logging::info!("Attempting to pull the Docker image: {}", image);
        
        let mut create_image_stream = docker.create_image(
            Some(CreateImageOptions {
                from_image: image,
                ..Default::default()
            }),
            None,
            None
        );

        while let Some(pull_result) = create_image_stream.next().await {
            pull_result?;
            blueprint_sdk::logging::info!("Image pull successful for {}", image);
        }

        blueprint_sdk::logging::info!("Setting up port mapping for container");
        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            format!("{}/tcp", container_port),
            Some(vec![PortBinding {
                host_ip: Some("0.0.0.0".to_string()),
                host_port: Some("0".to_string()),
            }]),
        );

        let host_config = HostConfig {
            port_bindings: Some(port_bindings),
            network_mode: Some("eigenavs".to_string()), // Attach to the "eigenavs" network
            ..Default::default()
        };

        let mut exposed_ports = HashMap::new();
        let string_value = format!("{}/tcp", container_port); // String
        let str_value: &str = string_value.as_str(); // Convert &String to &str
        exposed_ports.insert(str_value, HashMap::new());

        blueprint_sdk::logging::info!("Creating container with unique name");
        let container = docker.create_container(
            Some(CreateContainerOptions {
                name: format!("{}-{}", prefix, uuid::Uuid::new_v4()),
                ..Default::default()
            }),
            Config {
                image: Some(image),
                exposed_ports: Some(exposed_ports),
                host_config: Some(host_config),
                ..Default::default()
            },
        ).await?;

        blueprint_sdk::logging::info!("Starting container");
        docker.start_container(&container.id, None::<StartContainerOptions<String>>).await?;
        
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        
        let inspect = docker.inspect_container(&container.id, None).await?;
        let ports = &inspect.network_settings.as_ref().unwrap().ports.as_ref().unwrap();
        let binding = ports.get(&format!("{}/tcp", container_port)).unwrap().as_ref().unwrap().get(0).unwrap();
        let host_port = binding.host_port.as_ref().unwrap().clone();
        
        blueprint_sdk::logging::info!(
            "Docker container initialized and started - ID: {}, mapped to host port: {}", 
            container.id, 
            host_port
        );

        Ok(ContainerInfo {
            container_id: container.id,
            host_port,
        })
    }

    pub async fn cleanup(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Clean up both containers
        for container in [&self.inference_container, &self.checker_container] {
            blueprint_sdk::logging::info!("Attempting to remove container {}", container.container_id);
            self.docker.remove_container(
                &container.container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            ).await?;
            blueprint_sdk::logging::info!("Container {} removed successfully", container.container_id);
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct MomentaAvsContext {
    pub config: GadgetConfiguration,
    pub docker_manager: Arc<DockerManager>,
}
