# IC-SFT

A SFT (Semi-Fungible Token, implemented [ICRC-7](https://github.com/dfinity/ICRC/tree/main/ICRCs/ICRC-7) and [ICRC-37](https://github.com/dfinity/ICRC/tree/main/ICRCs/ICRC-7)) canister smart contract on the Internet Computer.

## Running the project locally

If you want to test your project locally, you can use the following commands:

```bash
# Starts the replica
dfx start

# Deploys your canisters to the replica and generates your candid interface
dfx deploy --argument '(record {symbol="SFT"; name="Semi-Fungible Token";})' ic_sft_canister
```

Once the job completes, your application will be available at `http://localhost:4943?canisterId={asset_canister_id}`.
