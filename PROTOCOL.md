# Compact protocol

The server speaks the usual Ogar protocols 1 to 18. It also speaks a compact
protocol, version 1, for clients that ask for it. The compact protocol is
protocol 6 with smaller world updates, and the web client in `client/` uses it.

## Asking for it

The client sends the protocol 6 handshake, with `MOG1` as the key:

| Bytes | Meaning |
|---|---|
| `FE 06 00 00 00` | protocol 6 |
| `FF 4D 4F 47 31` | key `"MOG1"` (`0x3147_4F4D` as a little-endian u32) |

Protocol 6 servers that do not know the compact protocol ignore the key and
speak protocol 6. A server that knows it sends a hello as its very first frame,
so the first frame tells the client which protocol it got.

## What changes

All messages from the client stay as in protocol 6. From the server:

* World updates come in **world** frames (`0x52`) instead of UpdateNodes (`0x10`).
* The camera position is part of the world frame, so there is no `0x11`.
* Owned cells are marked in the world frame, together with their unit, so there
  is no `0x20` or `0x21`. The server sends the active unit (`0x22`) from the
  start, as it does for protocol 6 clients after their first `44`.
* Each player's name and skin come once in a **player** frame (`0x51`), and cells
  refer to the player by pid.
* The leaderboard (`0x31` or `0x32`, as in protocol 6) is sent only when it
  differs from the last one sent.

Every other message is as in protocol 6: `0x12` clear all, `0x14` clear owned,
`0x22` active unit, `0x40` border, `0x63` chat and `0xFE` stats.

## Encoding

Fixed-size numbers are little-endian. Text is UTF-8 ending in a zero byte.

* **varint**: an unsigned LEB128 number. Each byte holds seven bits, lowest bits
  first, and the high bit is set on every byte except the last.
* **signed**: a zigzag varint. 0, -1, 1, -2, 2 and so on are written as 0, 1, 2,
  3, 4 and so on.
* **id lists**: ids in a list are written in increasing order. Each id is the
  difference from the one before it, modulo 2³². The first difference is from 0.

Ids, positions and sizes are the ones protocol 6 would send. Positions and
sizes are truncated to integers, and positions are offset by the scramble.
Scrambled ids are `id + scramble` (wrapping) rather than `id ^ scramble`, so a
sorted list stays sorted and its differences stay small.

## Messages

### `0x50` hello

| Field | Type |
|---|---|
| version | u8, 1 |

### `0x51` player

Sent before the first world frame that adds a cell of this player, and again
when the player's name or skin changes. The client keeps what it was told
across `0x12`.

| Field | Type |
|---|---|
| pid | varint |
| name | text |
| skin | text, empty for none |

### `0x52` world

The camera, then four sections in this order. Each section is a varint count
followed by that many records.

| Field | Type |
|---|---|
| camera x, y | signed, signed |
| eats | count, records |
| updates | count, records |
| adds | count, records |
| removals | count, records |

The camera is where protocol 6 would put `0x11`. Like `0x11`, it only matters
while the client has no cells.

**Eat**: a cell the client knew was eaten. The prey ids form an id list.

| Field | Type |
|---|---|
| prey | varint, id list |
| hunter | varint, the scrambled id |

**Update**: a known cell changed. The record starts with a head, which is
`(id difference << 4) | changed`; the ids form an id list. `changed` says
which fields follow, and each one is the difference from the value last sent:

| Bit | Field | Type |
|---|---|---|
| `0x1` | x | signed |
| `0x2` | y | signed |
| `0x4` | size | signed |
| `0x8` | color | r, g, b as u8 |

The server leaves out cells whose fields did not change. Protocol 6 resends
every player cell in view on every tick. A client that relies on that, for
example to see that a cell has stopped, has to repeat the player cells that
are missing from a frame itself.

**Add**: a cell the client did not know. The record starts with a head, which
is `(id difference << 3) | kind`; the ids form an id list.

| Field | Type |
|---|---|
| head | varint |
| x, y | signed, signed, relative to the camera |
| size | varint |
| color | r, g, b as u8 |
| pid | varint, for kinds 4 to 6 only |

| Kind | Cell |
|---|---|
| 0 | food |
| 1 | ejected mass |
| 2 | virus |
| 3 | mother cell |
| 4 | another player's cell |
| 5 | the client's own cell, unit 0 |
| 6 | the client's own cell, unit 1 |

**Removal**: a known cell left the view. The ids form an id list. Unlike
protocol 6, eaten cells are not listed again here.

## Size

`tests/view_sync.rs` plays the same world through both protocols, while
splitting, ejecting and switching units. On the server side, protocol 6 used
5.66 MB in that run and the compact protocol used 1.34 MB.

The client's `compact_protocol_builds_the_same_scene_as_protocol_6` test
decodes both streams of one recorded game and checks that they build the same
scene after every tick. The game is recorded by
`cargo run --release --example protocol_fixture`.
