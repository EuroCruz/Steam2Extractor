# Steam2 Extractor

Extracts Steam2 depot content from .dat/.blob/.sim/.sid

## Build

```
cargo build --release
```

## Usage

```
extract depot version [options]
extract file.sim [file2.sim ...] [options]

depot             depot id
version           version
file.sim          .sim manifest, one per disk, in order

--blob-dir dir    blob directory, downloaded automatically (default: steam2_cache/blobs)
--dat-dir dir     dat directory, downloaded automatically (default: steam2_cache/dats)
--blobcrc crc     blob crc, only needed after a depot reset
--key key         depot key, hex or depot:hex, repeatable
--keys-file file  "depot" "key" pairs, one per line, repeatable
--filter regex    only extract matching paths
--out dir         output directory
--offline         no network, local files only

extract donate    show donation addresses
```

## Donate

| | |
|---|---|
| Bitcoin | `bc1q2e60ws5yy6m5czv5wtc97z28vp8et3quvzx35c` |
| Solana | `HgqXV2YMWQggDJNDWzJ3rrczwhhy9AAUq7xZcziBqHbC` |
| ETH/Base/BNB | `0xff7b4d8be072dd36eb221c64cdc6ba48cce83b7e` |
