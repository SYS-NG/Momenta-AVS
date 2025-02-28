# <h1 align="center"> Momenta AVS - Audio Deepfake Detection on EigenLayer 🎵🔍 </h1>

A decentralized source of truth for audio deepfake detection powered by EigenLayer & Tangle Network 

## 📚 Overview

Momenta AVS is a decentralized platform built on EigenLayer that provides a trusted source of truth for audio deepfake detection. By leveraging the security and economic guarantees of EigenLayer's restaking mechanism, Momenta creates a reliable, tamper-resistant environment for detecting AI-generated audio content.

The service enables users to submit audio files for analysis, which are then processed through specialized machine learning models to determine authenticity. The results are recorded on-chain, creating an immutable record of the analysis that can be referenced by applications, platforms, or legal entities requiring trusted verification.

### 💼 Submodules

All submodules/source code for EigenGames2025 can be accessed in this repository: [EigenGames2025\_Momenta-AVS](https://github.com/SYS-NG/EigenGames2025_Momenta-AVS).

- **Audio-Checker-Docker**: The audio-check-docker source code is available here: [Audio-Checker-Docker](https://github.com/SYS-NG/Audio-Checker-Docker).
- **Momenta-Inference-Docker**: The Momenta-inference-docker source code is available here: [Momenta-Inference-Docker](https://github.com/SYS-NG/Momenta-Inference-Docker).
- **Audio-Upload-UI-Docker**: The UI implementation as a docker container is available here: [Audio-Upload-UI-Docker](https://github.com/SYS-NG/Audio-Upload-UI-Docker).

## 🏗️ Technical Architecture

![Momenta AVS Technical Architecture](momenta-avs-technical-architecture.png)

Momenta AVS consists of two primary components:

### On-chain Components (Solidity)
- Smart contracts for task management, result verification, and coordination
- ECDSA for Operator signature
- Recording inference results with confidence scores and signed by Operators

### Off-chain Components (Rust)
- Event listeners and task processors
- Docker container management for inference workloads
- Communication with EigenLayer middleware

### Docker Containers

The AVS utilizes two specialized Docker containers:

#### Audio Checking Container (audio-checking-docker)
- Serves as an API endpoint for receiving and processing audio files
- Located at port 5009 by default
- Responsible for file validation, metadata extraction, and queue management
- Accessible from external sources for file submission

#### PyTorch Audio Inference Container (pytorch-audio-inference)
- Runs the ML models for deepfake detection
- Located at port 5000 by default
- Processes audio files and generates confidence scores
- Isolated for security and performance reasons

Both containers are automatically pulled and configured by the AVS on startup, with dynamic port mapping to avoid conflicts.

## 🔬 How It Works

### Task Creation
- The task creation mechanism is used to poll the UI for newly uploaded  audio file
- A task is created with a unique identifier
- Operators are notified of the new task via events

### Inference Processing
- The AVS picks up the task and forwards it to the audio checking container
- Audio is processed through the inference container for deepfake detection
- Results include a prediction (real/fake) and a confidence score

### Result Recording
- Inference results are submitted on-chain via the `recordInferenceResult` function
- Results include the file reference, prediction, and confidence score
- An event is emitted with the complete results for external consumption

### Operator Consensus
- Multiple operators perform the same analysis and submit their signatures (Validation through Redundancy)
- ECDSA signatures are used to sign stored inference results to the chain
- Optimistic with the option to challenge recorded inference result

## 🚀 Getting Started

### Prerequisites
Before you can run this project, you will need to have the following software installed:
- Rust
- Forge
- Docker (for running the inference containers)

You will also need to install `cargo-tangle`, the CLI tool for creating and deploying Blueprints:

### Network Setup
Momenta relies on a Docker network for container communication. Create it with:

### Configuration
Check the `settings.env` file for the necessary contract addresses

### Deployment
Deploy the Momenta AVS to a local devnet with:

```bash:main:Deployment
cargo tangle blueprint deploy eigenlayer \
    --devnet \
    --ordered-deployment
```

This command:
- Compiles the Solidity contracts
- Deploys them in the correct order
- Sets up the AVS registry
- Configures the necessary permissions

The deployment will output the addresses of the deployed contracts, including the crucial `TASK_MANAGER_ADDRESS`.

### Running the Operator
To start an operator node, run:

```bash:main:Running the Operator
TASK_MANAGER_ADDRESS=<ADDRESS_FROM_OUTPUT> cargo tangle blueprint run \
    -p eigenlayer \
    -u <URL_FROM_DEPLOYMENT_OUTPUT> \
    --keystore-path ./test-keystore
```

This command:
- Connects to the specified RPC endpoint
- Pulls and starts the Docker containers for inference
- Begins listening for incoming tasks
- Processes any new tasks using the inference containers
- Submits the results back to the blockchain

### Operator Initialization Process
When an operator starts, it performs the following steps:

#### Keystore Loading:
- Validates key ownership and permissions

#### Docker Container Initialization:
- Pulls the required Docker images if not already present
- Creates and starts containers with appropriate network configuration
- Maps container ports to host system for external access

#### EigenLayer Registration:
- Registers with the EigenLayer middleware
- Prepares for task processing with the specified quorums

#### Event Monitoring:
- Begins listening for `NewTaskCreated` events from the `TangleTaskManager` contract
- Processes tasks as they are created

## 💻 Code Architecture

### Solidity Contracts
- **TangleTaskManager.sol**: The main contract responsible for:
  - Creating and tracking tasks (`createNewTask`)
  - Recording responses from operators
  - Storing inference results (`recordInferenceResult`)
  - Managing challenges and disputes
- **TangleServiceManager.sol**: Handles AVS registration with EigenLayer:
  - Interfaces with EigenLayer's AVS Directory
  - Manages rewards distribution
  - Enforces operator slashing rules

### Rust Components
- **src/main.rs**: Entry point with task manager initialization and event watching
- **src/lib.rs**: Core implementation with:
  - Event handling for inference tasks
  - Task pre-processing
  - Response submission to blockchain
  - ECDSA signature handling
- **src/context.rs**: Manages Docker containers and AVS context:
  - Initializes and configures Docker containers
  - Handles dynamic port mapping
  - Manages container lifecycle
  - Provides shared context for the AVS

## 📖 Smart Contract Events

The system emits several key events:
- **NewTaskCreated**: When a new audio file is submitted for analysis
- **TaskResponded**: When operators submit their responses to a task
- **TaskCompleted**: When a task reaches consensus and is finalized
- **InferenceResultRecorded**: When an inference result is recorded with its prediction and confidence

## 🔄 Docker Container Integration

The AVS manages Docker containers automatically:

### Container Configuration:
- Containers are pulled from the Docker Hub if not available locally
- Each container is assigned to the "eigenavs" Docker network
- Ports are dynamically assigned to avoid conflicts

### Container Communication:
- The AVS communicates with the checking container via HTTP
- The checking container forwards requests to the inference container
- Results are returned to the AVS via HTTP responses

### Failure Handling:
- Container failures are detected and handled
- Automatic cleanup on AVS shutdown prevents resource leaks

## 🔒 Security and Trust Model

Momenta AVS leverages EigenLayer's economic security for trust:
- **Operator Staking**: Operators stake ETH via EigenLayer, creating economic alignment
- **ECDSA Signatures**: Cryptographic signatures ensure result integrity
- **Challenge Mechanism**: Results can be challenged if fraud is suspected

## 📬 Feedback and Contributions

We welcome feedback and contributions to improve Momenta AVS. Please open an issue or submit a pull request on our GitHub repository.

## 📜 License
This project is licensed under the [MIT License](LICENSE).

