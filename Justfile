client:
    #!/bin/bash
    export RUST_LOG=crystalorb=debug,orbgame_client=trace,orbgame_shared=trace
    cargo run --package orbgame-client

server:
    #!/bin/bash
    export RUST_LOG=crystalorb=debug,orbgame_client=trace,orbgame_server=trace,orbgame_shared=trace
    cargo run --package orbgame-server
