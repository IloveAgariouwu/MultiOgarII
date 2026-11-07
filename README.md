# MultiOgarII
MultiOgarII is a FOSS agar.io server implementation that is based upon the [work done](https://github.com/Barbosik/MultiOgar) by [Barbosik](https://github.com/Barbosik/).

This is an unofficial Rust rewrite of [MultiOgarII](https://github.com/m-byte918/MultiOgarII) by Megabyte918 and contributors. It behaves like the JavaScript server, with a faster spatial index, multithreaded networking and multibox added.

## Quick Start
Install [Rust](https://rustup.rs/) 1.88 or newer, then run these from the project directory:

```BASH
# build and start the server
$ cargo run --release

# or build once and run the binary from this directory
$ cargo build --release
$ ./target/release/multiogarii
```

The server reads `config.toml` from the working directory, or the path given as the first argument. Every setting keeps its name from the old `config.js`, and any setting left out keeps its default.

The word filter, skins, IP bans and accounts live in `data/`. Change the passwords in `data/userRoles.json` before you open your server to anyone.

The default `serverStatsPort` is 88, as it always was. That port needs root on Linux and is often taken on Windows; the server only logs a message and keeps running if it cannot bind it. Pick another port, or 0 to turn the stats server off.

Behind a reverse proxy on the same machine, every player appears as `127.0.0.1`. They then share one `serverIpLimit`, ignore the ban list, and can log in to accounts limited to that address, so change or remove those accounts first.

## Benchmark
Tick time against the last JavaScript release (`2fa3f12`, MultiOgarII 1.6.3), FFA with only bots connected. Both servers time the same work per tick and report it as `update_time` on the stats port; each run samples it once per second for 60 seconds after a 15-second warm-up. Measured on an AMD Ryzen 7 9800X3D, Windows 11, Node.js 22.16.0.

| Bots | JavaScript median / p95 (ms) | Rust median / p95 (ms) | Speed-up (median) | Peak RSS JS / Rust (MiB) |
|---:|---:|---:|---:|---:|
| 100 | 0.538 / 0.702 | 0.136 / 0.215 | 4.0x | 62 / 14 |
| 300 | 1.507 / 1.926 | 0.256 / 0.347 | 5.9x | 81 / 15 |
| 600 | 3.390 / 4.102 | 0.623 / 0.806 | 5.4x | 88 / 17 |

Bots have no real socket, so this measures the game logic and leaves out network traffic. To run it yourself (needs Node.js and npm; the JavaScript server is unpacked into `target/bench`):

```BASH
$ node bench/compare.mjs --bots 100,300,600 --duration 60
```

### Questions / Modifications
If you have a question about how to add a feature, or how to change a behavior of the game, ask for help on the [Agar.io Private Servers Discord](https://discord.gg/XcKgShT)
Do not create a GitHub issue for a question.

### Commands
Once started, the server's functionality can be altered via the use of commands. These commands can be listed, and documented, by running `help`.

Players can use chat commands, starting with `/help`. Some of them need a login with `/login`.

### Gamemodes
The server offers a small set of gamemodes, these include:
- Free For All
- Experimental
- Teams
- Rainbow

All the gamemodes are standard, aside from rainbow.

### Multibox
With `serverMultibox` on, a client can control two sets of cells and swap between them. It works on every protocol version: the client sends the single byte `26` to switch, and the server answers with `0x14` ClearOwned followed by one `0x20` AddNode per cell of the other set, packets every client already understands. Clients that never send `26` see no difference.

Clients can instead switch with xingkong's Tab packet, the single byte `44`. The server then sends owned cells with their unit, `0x20` AddNode for unit 0 and `0x21` for unit 1, for both units, and announces the active unit with `0x22` followed by the unit byte whenever it changes: on a switch, when the active unit dies and control moves to the other one, and on respawn. It no longer sends `0x14` on switches.

For every client, mouse, split, eject and switch packets are applied in the order they arrived, so a split sent before a switch splits the unit that was active. Splits are counted per unit and each unit splits at most once per tick.

## Protocols
This server implementation supports the majority of the early Agar protocols. This includes protocols ranging from `v4` to `v18`. This means that clients running protocols outside of that range *may malfunction or not work at all.* It is suggested that you use a client such as [Cigar](https://github.com/Luka967/Cigar), or its [Cigar2](https://github.com/Cigar2/Cigar2) variant - both of these use protocol v6 and are compatible with the server.

## Web client
[client/](./client) holds a web client for this server, built with Rust, WebAssembly and Svelte. It speaks protocol v6 and supports multibox with the Tab key. It can also run this server inside the browser for offline play.

Building it needs Node.js, the Rust target `wasm32-unknown-unknown` and `wasm-bindgen-cli` 0.2.129:

```BASH
$ rustup target add wasm32-unknown-unknown
$ cargo install wasm-bindgen-cli --version 0.2.129 --locked
$ sh client/build-web.sh
$ cd client/ui
$ npm ci
$ npm run build
```

The page in `client/ui/dist` starts on Solo, which runs this server in the browser with 20 bots. The server picker has a Solo server for each gamemode (FFA, Teams, Experimental and Rainbow); they share one WebAssembly build and each runs in its own shared worker. Local is port 8080 of the host that served the page; other servers can be added in the server picker or linked with `?ip=host:port`.

## TLS
In order to use protocols such as HTTPS with a webocket connection you need to enable TLS. This is documented in [keys/README.md](./keys/README.md).
