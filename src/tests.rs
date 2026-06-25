#[cfg(test)]
mod tests {
    use crate::models::{
        InputContent, InputMessage, MessageRequest, MessageResponse, MessageRole,
        OutputContentBlock, TokenUsage, ToolDefinition,
    };
    use crate::provider::{
        apply_extensions, resolve_auth, route_model, ProviderKind, AuthHeader
    };
    use crate::sse::{
        AccumulatorState, DeltaPayload, MessageDeltaPayload, SSEEvent, parse_sse_event,
    };
    use crate::config::{
        load_merged_config, Config, PermissionRules, get_config_paths
    };
    use crate::config::precedence::resolve_model;
    use crate::config::aliases::resolve_alias;
    use crate::path_scope::{
        tokenize_payload, extract_paths, validate_path, ValidationResult, is_path_like, expand_env_vars
    };
    use crate::usage::{
        calculate_cost, get_model_rates, UsageTracker, HAIKU_RATES, SONNET_RATES, UNKNOWN_RATES
    };
    use std::collections::{HashMap, HashSet};
    use std::path::{Path, PathBuf};

    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn set_env_var(key: &str, value: &str) {
        unsafe {
            std::env::set_var(key, value);
        }
    }

    fn remove_env_var(key: &str) {
        unsafe {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn test_routing_cascade() {
        // Rule 1: contains claude
        assert_eq!(route_model("claude-3-5-sonnet"), ProviderKind::Anthropic);
        assert_eq!(route_model("Claude-Opus"), ProviderKind::Anthropic);

        // Rule 2: contains grok
        assert_eq!(route_model("grok-3-mini"), ProviderKind::XAI);
        assert_eq!(route_model("Grok-Beta"), ProviderKind::XAI);

        // Rule 3: starts with openai/, local/, or gpt-
        assert_eq!(route_model("openai/gpt-4o"), ProviderKind::OpenAICompat);
        assert_eq!(route_model("local/llama3"), ProviderKind::OpenAICompat);
        assert_eq!(route_model("gpt-3.5-turbo"), ProviderKind::OpenAICompat);

        // Rule 4: starts with qwen/, qwen-, kimi/, or kimi-
        assert_eq!(route_model("qwen-max"), ProviderKind::DashScope);
        assert_eq!(route_model("kimi-k2.5"), ProviderKind::DashScope);
        assert_eq!(route_model("qwen/coder"), ProviderKind::DashScope);
    }

    #[test]
    fn test_credential_fallback() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // Rule 6 fallback
        // Clean environment
        remove_env_var("ANTHROPIC_API_KEY");
        remove_env_var("ANTHROPIC_AUTH_TOKEN");
        remove_env_var("OPENAI_API_KEY");
        remove_env_var("XAI_API_KEY");
        remove_env_var("OLLAMA_HOST");

        // Set only XAI_API_KEY
        set_env_var("XAI_API_KEY", "xai-test-key-123");

        // Route unknown model name
        assert_eq!(route_model("my-custom-unmatched-model"), ProviderKind::XAI);

        remove_env_var("XAI_API_KEY");
    }

    #[test]
    fn test_auth_header_resolution() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // Clean environment
        remove_env_var("ANTHROPIC_API_KEY");
        remove_env_var("ANTHROPIC_AUTH_TOKEN");

        // Set valid sk-ant- key in ANTHROPIC_API_KEY
        set_env_var("ANTHROPIC_API_KEY", "sk-ant-key123");
        let auth = resolve_auth(ProviderKind::Anthropic).unwrap().unwrap();
        assert_eq!(auth.name, "x-api-key");
        assert_eq!(auth.value, "sk-ant-key123");

        // Set invalid variable warning (sk-ant- in auth token)
        remove_env_var("ANTHROPIC_API_KEY");
        set_env_var("ANTHROPIC_AUTH_TOKEN", "sk-ant-key456");
        let auth2 = resolve_auth(ProviderKind::Anthropic).unwrap().unwrap();
        assert_eq!(auth2.name, "Authorization");
        assert_eq!(auth2.value, "Bearer sk-ant-key456");

        remove_env_var("ANTHROPIC_AUTH_TOKEN");
    }

    #[test]
    fn test_apply_extensions() {
        let mut base = serde_json::json!({
            "model": "claude-sonnet-4-6",
            "messages": [],
            "stream": false,
            "custom_val": 10
        });

        let mut extensions = serde_json::Map::new();
        // Trying to override protected core fields
        extensions.insert("model".to_string(), serde_json::json!("overridden-model"));
        extensions.insert("stream".to_string(), serde_json::json!(true));
        // Valid custom key
        extensions.insert("max_completion_tokens".to_string(), serde_json::json!(4096));

        apply_extensions(&mut base, &Some(extensions));

        // Assert core fields are protected
        assert_eq!(base["model"].as_str().unwrap(), "claude-sonnet-4-6");
        assert_eq!(base["stream"].as_bool().unwrap(), false);
        // Assert custom field is merged
        assert_eq!(base["max_completion_tokens"].as_u64().unwrap(), 4096);
    }

    #[test]
    fn test_sse_accumulation() {
        let mut state = AccumulatorState::new();

        let initial_resp = MessageResponse {
            response_id: "resp_123".to_string(),
            role: "assistant".to_string(),
            content_blocks: Vec::new(),
            model_used: "claude-sonnet-4-6".to_string(),
            stop_reason: None,
            token_usage: TokenUsage::default(),
        };

        // 1. SessionStart
        state.transition(SSEEvent::SessionStart { response: initial_resp });
        
        // 2. ContentBlockBegin (text at index 0)
        state.transition(SSEEvent::ContentBlockBegin {
            index: 0,
            block: OutputContentBlock::TextContent { text: String::new() },
        });

        // 3. ContentBlockDelta (text fragment)
        state.transition(SSEEvent::ContentBlockDelta {
            index: 0,
            delta: DeltaPayload::Text { text: "hello".to_string() },
        });

        // 4. ContentBlockBegin (tool at index 1)
        state.transition(SSEEvent::ContentBlockBegin {
            index: 1,
            block: OutputContentBlock::ToolInvocation {
                id: "tool_123".to_string(),
                name: "FileRead".to_string(),
                input: serde_json::Value::Null,
            },
        });

        // 5. ContentBlockDelta (tool input JSON segment)
        state.transition(SSEEvent::ContentBlockDelta {
            index: 1,
            delta: DeltaPayload::Json { json: "{\"path\":".to_string() },
        });

        // 6. ContentBlockDelta (text fragment 2)
        state.transition(SSEEvent::ContentBlockDelta {
            index: 0,
            delta: DeltaPayload::Text { text: " world".to_string() },
        });

        // 7. ContentBlockDelta (tool input JSON segment 2)
        state.transition(SSEEvent::ContentBlockDelta {
            index: 1,
            delta: DeltaPayload::Json { json: "\"/tmp/test.txt\"}".to_string() },
        });

        // 8. ContentBlockEnd index 0
        state.transition(SSEEvent::ContentBlockEnd { index: 0 });

        // 9. ContentBlockEnd index 1 (should parse JSON)
        state.transition(SSEEvent::ContentBlockEnd { index: 1 });

        // 10. MessageDelta
        state.transition(SSEEvent::MessageDelta {
            delta: MessageDeltaPayload {
                stop_reason: Some("tool_use".to_string()),
                token_usage: Some(TokenUsage {
                    input_tokens: 100,
                    output_tokens: 50,
                    cache_creation_tokens: 0,
                    cache_read_tokens: 0,
                }),
            },
        });

        // 11. SessionEnd
        state.transition(SSEEvent::SessionEnd);

        // Verify final state
        if let AccumulatorState::Complete(resp) = state {
            assert_eq!(resp.response_id, "resp_123");
            assert_eq!(resp.stop_reason, Some("tool_use".to_string()));
            assert_eq!(resp.token_usage.input_tokens, 100);
            assert_eq!(resp.token_usage.output_tokens, 50);
            
            assert_eq!(resp.content_blocks.len(), 2);
            
            // Text block check
            if let OutputContentBlock::TextContent { text } = &resp.content_blocks[0] {
                assert_eq!(text, "hello world");
            } else {
                panic!("Block 0 is not text");
            }

            // Tool invocation block check
            if let OutputContentBlock::ToolInvocation { id, name, input } = &resp.content_blocks[1] {
                assert_eq!(id, "tool_123");
                assert_eq!(name, "FileRead");
                assert_eq!(input["path"].as_str().unwrap(), "/tmp/test.txt");
            } else {
                panic!("Block 1 is not tool invocation");
            }
        } else {
            panic!("AccumulatorState is not Complete");
        }
    }

    #[test]
    fn test_sse_interruption() {
        let mut state = AccumulatorState::new();

        let initial_resp = MessageResponse {
            response_id: "resp_123".to_string(),
            role: "assistant".to_string(),
            content_blocks: Vec::new(),
            model_used: "claude-sonnet-4-6".to_string(),
            stop_reason: None,
            token_usage: TokenUsage::default(),
        };

        state.transition(SSEEvent::SessionStart { response: initial_resp });
        state.transition(SSEEvent::ContentBlockBegin {
            index: 0,
            block: OutputContentBlock::TextContent { text: String::new() },
        });
        state.transition(SSEEvent::ContentBlockDelta {
            index: 0,
            delta: DeltaPayload::Text { text: "partial content".to_string() },
        });

        // Force connection error
        state.force_error("connection reset by peer");

        let partial = state.get_partial_response().unwrap();
        assert_eq!(partial.response_id, "resp_123");
        if let OutputContentBlock::TextContent { text } = &partial.content_blocks[0] {
            assert_eq!(text, "partial content");
        } else {
            panic!("Missing text block");
        }
    }

    #[test]
    fn test_model_precedence() {
        let _guard = ENV_MUTEX.lock().unwrap();
        // Set env vars
        set_env_var("NAKAMA_MODEL", "model-env-nakama");
        set_env_var("ANTHROPIC_MODEL", "model-env-anthropic");

        let config = Config {
            model: Some("model-config".to_string()),
            ..Default::default()
        };

        // 1. CLI flag wins
        let model1 = resolve_model(Some("model-cli"), &config);
        assert_eq!(model1, "model-cli");

        // 2. NAKAMA_MODEL wins when CLI flag is missing
        let model2 = resolve_model(None, &config);
        assert_eq!(model2, "model-env-nakama");

        remove_env_var("NAKAMA_MODEL");

        // 3. ANTHROPIC_MODEL wins when CLI and NAKAMA_MODEL are missing
        let model3 = resolve_model(None, &config);
        assert_eq!(model3, "model-env-anthropic");

        remove_env_var("ANTHROPIC_MODEL");

        // 4. Config file wins when all env vars are missing
        let model4 = resolve_model(None, &config);
        assert_eq!(model4, "model-config");

        // 5. Default fallback
        let empty_config = Config::default();
        let model5 = resolve_model(None, &empty_config);
        assert_eq!(model5, "claude-sonnet-4-6");
    }

    #[test]
    fn test_model_alias_resolution() {
        let config = Config {
            model_aliases: Some(vec![
                ("opus".to_string(), "my-custom-opus-v2".to_string())
            ].into_iter().collect()),
            ..Default::default()
        };

        // Config overrides built-in alias
        assert_eq!(resolve_alias("opus", &config), "my-custom-opus-v2");

        // Built-in alias maps correctly
        assert_eq!(resolve_alias("sonnet", &config), "claude-sonnet-4-6");
        assert_eq!(resolve_alias("haiku", &config), "claude-haiku-4-5-20251213");
        assert_eq!(resolve_alias("grok", &config), "grok-3");

        // Case-insensitivity check for built-in
        assert_eq!(resolve_alias("Sonnet", &config), "claude-sonnet-4-6");

        // Pass-through unmatched
        assert_eq!(resolve_alias("gpt-4o", &config), "gpt-4o");
    }

    #[test]
    fn test_path_tokenizer_and_extraction() {
        // Test basic shlex split
        let tokens = tokenize_payload("echo 'hello world' > output.txt");
        assert_eq!(tokens, vec!["echo", "hello world", ">", "output.txt"]);

        // Test fallback quote stripping on unmatched quotes
        let fallback_tokens = tokenize_payload("echo \"hello 'world");
        assert_eq!(fallback_tokens, vec!["echo", "\"hello", "'world"]); // quote stripping is performed on matched external quotes only

        // Redirection extraction and flag filters
        let paths = extract_paths(&vec![
            "grep".to_string(),
            "-i".to_string(),
            "pattern".to_string(),
            "/workspace/src/".to_string(),
            ">".to_string(),
            "out.log".to_string(),
        ]);
        // "-i" is filtered out as flag.
        // "out.log" is extracted from redirection.
        // "/workspace/src/" is path-like.
        assert_eq!(paths, vec!["/workspace/src/", "out.log"]);
    }

    #[test]
    fn test_path_scope_validation() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let roots = vec![PathBuf::from("/home/user/workspace")];
        let cwd = Path::new("/home/user/workspace");

        // Test normal containment
        let res1 = validate_path("./src/main.rs", &roots, cwd);
        if let ValidationResult::Allowed { .. } = res1 {
            // pass
        } else {
            panic!("Expected allowed path");
        }

        // Test directory traversal escape
        let res2 = validate_path("../escape/file.txt", &roots, cwd);
        if let ValidationResult::Denied { .. } = res2 {
            // pass
        } else {
            panic!("Expected denied path");
        }

        // Test env variable expansion
        set_env_var("MY_DIR", "src");
        let res3 = validate_path("./$MY_DIR/main.rs", &roots, cwd);
        if let ValidationResult::Allowed { .. } = res3 {
            // pass
        } else {
            panic!("Expected allowed path with env var");
        }
        remove_env_var("MY_DIR");
    }

    #[test]
    fn test_usage_tracking_and_cost() {
        let rates = get_model_rates("claude-haiku-4-5");
        assert_eq!(rates, HAIKU_RATES);

        let usage = TokenUsage {
            input_tokens: 10000,          // $1.00 per M -> $0.0100
            output_tokens: 2000,         // $5.00 per M -> $0.0100
            cache_creation_tokens: 1000, // $1.25 per M -> $0.00125
            cache_read_tokens: 5000,     // $0.10 per M -> $0.0005
        };

        let cost = calculate_cost(usage, rates);
        // total: 0.0100 + 0.0100 + 0.00125 + 0.0005 = 0.02175
        assert!((cost - 0.02175).abs() < 1e-6);

        // Unknown class test
        let unknown_rates = get_model_rates("gpt-4o");
        assert_eq!(unknown_rates, UNKNOWN_RATES); // decoupled pricing constant
    }

    #[test]
    fn test_usage_tracker_reconstruction() {
        let mut tracker = UsageTracker::new();
        
        let messages = vec![
            MessageResponse {
                response_id: "1".to_string(),
                role: "assistant".to_string(),
                content_blocks: Vec::new(),
                model_used: "model".to_string(),
                stop_reason: None,
                token_usage: TokenUsage {
                    input_tokens: 10,
                    output_tokens: 5,
                    cache_creation_tokens: 0,
                    cache_read_tokens: 0,
                },
            },
            MessageResponse {
                // message without usage metadata (all zeros) should be skipped
                response_id: "2".to_string(),
                role: "assistant".to_string(),
                content_blocks: Vec::new(),
                model_used: "model".to_string(),
                stop_reason: None,
                token_usage: TokenUsage::default(),
            },
            MessageResponse {
                response_id: "3".to_string(),
                role: "assistant".to_string(),
                content_blocks: Vec::new(),
                model_used: "model".to_string(),
                stop_reason: None,
                token_usage: TokenUsage {
                    input_tokens: 20,
                    output_tokens: 10,
                    cache_creation_tokens: 2,
                    cache_read_tokens: 1,
                },
            },
        ];

        tracker.reconstruct_from_messages(&messages);
        
        assert_eq!(tracker.turn_count, 2);
        assert_eq!(tracker.cumulative_usage.input_tokens, 30);
        assert_eq!(tracker.cumulative_usage.output_tokens, 15);
        assert_eq!(tracker.cumulative_usage.cache_creation_tokens, 2);
        assert_eq!(tracker.cumulative_usage.cache_read_tokens, 1);
        
        // Latest turn usage should equal the last usage-bearing message
        assert_eq!(tracker.latest_turn_usage.input_tokens, 20);
        assert_eq!(tracker.latest_turn_usage.output_tokens, 10);
    }

    #[test]
    fn test_nim_accumulator() {
        use crate::nim_accumulator::NimAccumulator;

        let mut accumulator = NimAccumulator::new();
        assert!(!accumulator.is_done());

        // Chunk 1: "Hello"
        let data1 = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;
        let delta1 = accumulator.process_line(data1);
        assert_eq!(delta1, Some("Hello".to_string()));
        assert!(!accumulator.is_done());

        // Chunk 2: " world"
        let data2 = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}"#;
        let delta2 = accumulator.process_line(data2);
        assert_eq!(delta2, Some(" world".to_string()));
        assert!(!accumulator.is_done());

        // Chunk 3: Empty/Null delta content
        let data3 = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","choices":[{"index":0,"delta":{},"finish_reason":null}]}"#;
        let delta3 = accumulator.process_line(data3);
        assert_eq!(delta3, None);
        assert!(!accumulator.is_done());

        // Chunk 4: Stop reason & Usage
        let data4 = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":15,"completion_tokens":25}}"#;
        let delta4 = accumulator.process_line(data4);
        assert_eq!(delta4, None);
        assert!(!accumulator.is_done());

        // Chunk 5: [DONE] sentinel
        let delta5 = accumulator.process_line("[DONE]");
        assert_eq!(delta5, None);
        assert!(accumulator.is_done());

        // Verify accumulated results
        let (text, usage, stop_reason) = accumulator.into_result();
        assert_eq!(text, "Hello world");
        assert_eq!(stop_reason, Some("stop".to_string()));
        assert_eq!(usage.input_tokens, 15);
        assert_eq!(usage.output_tokens, 25);
    }
}

