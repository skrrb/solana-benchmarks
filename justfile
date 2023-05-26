DUMP := "~/.local/share/solana/install/active_release/bin/sdk/bpf/scripts/dump.sh"

default:
    bash run_and_process_log.sh

dump:
    cargo build-sbf && {{ DUMP }} ./target/sbf-solana-solana/release/openbook_v2_cu.so dump.txt
