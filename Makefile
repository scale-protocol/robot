localmint:
	spl-token create-token --decimals 6 -- local_mint.json
	spl-token create-account -- local_mint.json
	spl-token mint 3xJL46KjjDQbPUDg54nEzSC1Ejs49xFHwQJyEMPq7H7g 60000000000000
transfer:
	spl-token transfer --fund-recipient 3xJL46KjjDQbPUDg54nEzSC1Ejs49xFHwQJyEMPq7H7g 10000000000 6imhP9ec6sNXy7Dn19wq4hjL1oUtthtGHUEwtuCTGNL8
initvault:
	cargo run init_vault
initmarket:
	cargo run init_market -p 'BTC/USD' -s 100.0 -y 'HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J' \
	-t 'CzZQBrJCLqjXRfMjRN3fhbxur2QYHUzkpaRwkWsiPqbz'
	cargo run init_market -p 'ETH/USD' -s 10.0 -y 'EdVCmQ9FSPcVe5YySXDPCRmc8aDQLKJ9xvYBMZPie1Vw' \
	-t '2ypeVyYnZaW2TNYXXTaZq9YhYvnqcjCiifW1C6n8b7Go'
	cargo run init_market -p 'SOL/USD' -s 5.0 -y 'J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix' \
	-t 'HgTtcbcmp5BeThax5AU8vg4VwK79qAvAKKFMs8txMLW6'
inituser:
	cargo run init_user
deposit:
	cargo run deposit -a 100000000000
investment:
	cargo run investment -p 'BTC/USD' -a 10000000000000
	cargo run investment -p 'ETH/USD' -a 10000000000000
	cargo run investment -p 'SOL/USD' -a 10000000000000
divestment:
	cargo run divestment -p 'BTC/USD' -a 10000
	cargo run divestment -p 'ETH/USD' -a 5000
	cargo run divestment -p 'SOL/USD' -a 5000
openposition:
	cargo run open_position -p 'BTC/USD' -s 0.0001 -l 1 -t 1 -d 1
closeposition:
	cargo run close_position -a GJKBaC3sBdPf5YJ3K2KRNY7MwY6yJPqsvQ1LczADGsih
bot:
	# export RUST_LOG=robot::bot::machine=debug && cargo run -- bot
	# export RUST_LOG=robot::http::service=debug && cargo run -- bot
	export RUST_LOG=debug && cargo run -- bot
dockerbuild:
	export DOCKER_BUILDKIT=1 && docker build --ssh default=~/.ssh/id_rsa -t tttlkkkl/scale:latest .
gitset:
	export CARGO_NET_GIT_FETCH_WITH_CLI=true
	git config --global url."git@github.com:".insteadOf "https://github.com/"
one:
	spl-token create-token --decimals 6 -- local_mint.json
	spl-token create-account -- local_mint.json
	spl-token mint 3xJL46KjjDQbPUDg54nEzSC1Ejs49xFHwQJyEMPq7H7g 1000000000000
	cargo run init_vault
	cargo run init_market -p 'BTC/USD' -s 0.01 -y 'HovQMDrbAgAYPCmHVSrezcSmkMtXSSUsLDFANExrZh2J' \
	-t 'CzZQBrJCLqjXRfMjRN3fhbxur2QYHUzkpaRwkWsiPqbz'
	cargo run init_market -p 'ETH/USD' -s 0.05 -y 'EdVCmQ9FSPcVe5YySXDPCRmc8aDQLKJ9xvYBMZPie1Vw' \
	-t '2ypeVyYnZaW2TNYXXTaZq9YhYvnqcjCiifW1C6n8b7Go'
	cargo run init_market -p 'SOL/USD' -s 0.05 -y 'J83w4HKfqxwcq3BEMMkPFSppX3gqekLyLJBexebFVkix' \
	-t 'HgTtcbcmp5BeThax5AU8vg4VwK79qAvAKKFMs8txMLW6'
	cargo run init_user
	cargo run deposit -a 1000000000
	cargo run investment -p 'BTC/USD' -a 100000000000
	cargo run investment -p 'ETH/USD' -a 100000000
	cargo run investment -p 'SOL/USD' -a 100000000
	cargo run divestment -p 'BTC/USD' -a 10000
	cargo run divestment -p 'ETH/USD' -a 5000
	cargo run divestment -p 'SOL/USD' -a 5000
	cargo run open_position -p 'BTC/USD' -s 1.1 -l 20 -t 1 -d 1
	cargo run close_position -a GJKBaC3sBdPf5YJ3K2KRNY7MwY6yJPqsvQ1LczADGsih