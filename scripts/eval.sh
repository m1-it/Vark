#!/usr/bin/env bash
set -euo pipefail

cargo run --locked -p vark-eval -- --json --check-baseline
