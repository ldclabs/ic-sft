BUILD_ENV := rust

.PHONY: build-wasm build-did

lint:
	@cargo fmt
	@cargo clippy --all-targets --all-features

fix:
	@cargo clippy --fix --workspace --tests

# cargo install twiggy
twiggy:
	twiggy top -n 12 target/wasm32-unknown-unknown/release/ic_sft_canister.wasm

# cargo install ic-wasm
build-wasm:
	cargo build --release --target wasm32-unknown-unknown --package ic_sft_canister

shrink-wasm:
	ic-wasm -o target/wasm32-unknown-unknown/release/ic_sft_canister_optimized.wasm target/wasm32-unknown-unknown/release/ic_sft_canister.wasm shrink

# cargo install candid-extractor
build-did:
	candid-extractor target/wasm32-unknown-unknown/release/ic_sft_canister.wasm > src/ic_sft_canister/ic_sft_canister.did
