#!/bin/bash
set -e

echo "Condition 1:"
echo -e "say hello\n" | NAKAMA_MODEL="claude-3-5-sonnet-20241022" ANTHROPIC_API_KEY="fake" cargo run --bin nakama --quiet | head -n 15

echo "Condition 2:"
echo -e "say hello\n" | NAKAMA_MODEL="moonshotai/kimi-k2-5" NVIDIA_API_KEY="fake" cargo run --bin nakama --quiet | head -n 15

echo "Condition 3:"
echo -e "say hello\n" | NAKAMA_MODEL="grok-2" cargo run --bin nakama --quiet || true

echo "Condition 4:"
echo -e "say hello\n" | env -u NAKAMA_MODEL ANTHROPIC_API_KEY="fake" cargo run --bin nakama --quiet | head -n 15

echo "Condition 5:"
echo -e "say hello\n" | NAKAMA_PROVIDER="nim" NAKAMA_MODEL="claude-3-5-sonnet" NVIDIA_API_KEY="fake" cargo run --bin nakama --quiet | head -n 15
