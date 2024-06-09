# tinytracker

tinytracker provides a [tokio-util](https://crates.io/crates/tokio-util) codec for the BitTorrent UDP tracker protocol specified in [BEP-0015](https://www.bittorrent.org/beps/bep_0015.html).

Additionally tinytracker ships an implementation that always serves the same-preprogrammed peers for all requested info hashes.


## Usage

Please see `Dockerfile` and `docker-compose.yml` for using the tinytracker sample implementation.

