# Nakama

![Build Status](https://img.shields.io/badge/build-passing-brightgreen)
![Version](https://img.shields.io/badge/version-0.1.0-blue)
![License](https://img.shields.io/badge/license-MIT-green)
![Rust](https://img.shields.io/badge/rust-edition%202024-orange)
![Python](https://img.shields.io/badge/python-3.10%2B-yellow)

## Table of Contents

- [About the Project](#about-the-project)
- [Features](#features)
- [Built With](#built-with)
- [Getting Started](#getting-started)
  - [Prerequisites](#prerequisites)
  - [Installation](#installation)
- [Usage](#usage)
- [Contributing](#contributing)
- [License](#license)
- [Contact](#contact)

---

## About the Project

**Nakama** is a high-performance, dual-architecture AI coding assistant and agent infrastructure designed to bridge the gap between fast systems programming and flexible scripting. The project solves the complexity of orchestrating autonomous AI agents by splitting responsibilities: a blistering-fast primary engine written in Rust handles real-time API streaming and strict sandboxed tool execution, while a companion Python workspace mirrors state, routes dynamic prompts, and audits for parity.

Whether you're looking to run local shell commands via AI, interface with OpenAI-compatible streaming endpoints (like NVIDIA NIM), or enforce strict filesystem permission boundaries, Nakama provides a rock-solid foundation for next-generation agentic workflows.

---

## Features

- **Dual-Architecture Engine:** High-performance Rust backend for state and streaming, paired with a Python companion for prompt routing and auditing.
- **Advanced Tool Dispatch System:** Built-in capabilities including shell execution, file I/O operations, and recursive grep searches, all bound by strict path-scope validations to prevent directory traversal escapes.
- **Interactive Permission Gates:** Granular control over AI actions with `Prompt` (interactive approval) and `Auto` (autonomous execution) modes.
- **Provider Interoperability:** Seamlessly routes between AI models and providers (e.g., Anthropic, OpenAI, xAI, DashScope, NVIDIA NIM) using a unified Server-Sent Events (SSE) streaming interface.
- **Intelligent Transcript Compaction:** Automatically summarizes lengthy conversation histories to stay within model token context limits.

---

## Built With

- **Rust** (Edition 2024) - Core runtime, async streaming (`tokio`, `reqwest`), CLI interface (`clap`), and tool dispatching.
- **Python** (3.10+) - Companion workspace, parity auditing, and query simulation (`python_companion`).
- **OpenAI-Compatible APIs** - Standardized integration for chat completions and tool/function calling logic.

---

## Getting Started

Follow these steps to set up the Nakama runtime and Python companion locally.

### Prerequisites

Ensure you have the following installed on your machine:
- [Rust & Cargo](https://rustup.rs/) (Edition 2024)
- [Python 3.10+](https://www.python.org/downloads/)
- Optional: API Keys for your preferred AI providers (e.g., `NVIDIA_API_KEY`, `ANTHROPIC_API_KEY`)

### Installation

1. **Clone the repository:**
   ```bash
   git clone https://github.com/TanKaizokuO/nakama.git
   cd nakama
   ```

2. **Configure Environment Variables:**
   Create a `.env` file in the root directory and add your API credentials:
   ```env
   NVIDIA_API_KEY=your_api_key_here
   URL=https://integrate.api.nvidia.com/v1
   NAKAMA_PERMISSION_MODE=prompt
   ```

3. **Build the Rust Core:**
   ```bash
   cargo build --release
   ```

4. **Run the Application:**
   ```bash
   cargo run
   ```

5. **(Optional) Setup Python Companion:**
   ```bash
   cd python_companion
   # Ensure you have your virtual environment set up and active
   python main.py setupreport
   ```

---

## Usage

Once Nakama is running, it will open an interactive REPL loop. You can issue commands to the AI directly.

*Example flow:*
```text
Nakama ready. Model: moonshotai/kimi-k2.6
> list the files in the src directory
[tool: list_files({"path": "src"})]
Allow tool call: list_files({"path": "src"})? [y/N] y
```

*(Add further screenshots or CLI gifs here to showcase the recursive tool loop in action!)*

---

## Contributing

Contributions make the open-source community an amazing place to learn, inspire, and create. Any contributions you make are **greatly appreciated**.

1. Fork the Project
2. Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3. Commit your Changes (`git commit -m 'feat: Add some AmazingFeature'`)
4. Push to the Branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

---

## License

Distributed under the MIT License. See `LICENSE` for more information.

---

## Contact

**TanKaizokuO**  
GitHub: [@TanKaizokuO](https://github.com/TanKaizokuO)  
Project Link: [https://github.com/TanKaizokuO/nakama](https://github.com/TanKaizokuO/nakama)
