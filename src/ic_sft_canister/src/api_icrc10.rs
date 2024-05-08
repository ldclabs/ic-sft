use ic_sft_types::SupportedStandard;

// Returns the list of standards this ledger implements.
#[ic_cdk::query]
pub fn icrc10_supported_standards() -> Vec<SupportedStandard> {
    vec![
        SupportedStandard {
            name: "ICRC-3".to_string(),
            url: "https://github.com/dfinity/ICRC-1/tree/main/standards/ICRC-3".to_string(),
        },
        SupportedStandard {
            name: "ICRC-7".into(),
            url: "https://github.com/dfinity/ICRC/tree/main/ICRCs/ICRC-7".into(),
        },
        SupportedStandard {
            name: "ICRC-10".into(),
            url: "https://github.com/dfinity/ICRC/tree/main/ICRCs/ICRC-10".into(),
        },
        SupportedStandard {
            name: "ICRC-37".into(),
            url: "https://github.com/dfinity/ICRC/tree/main/ICRCs/ICRC-37".into(),
        },
    ]
}
