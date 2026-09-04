# Steam2 Extractor

[English](README.md) | Русский

Извлекает содержимое из файлов .dat/.blob/.sim/.sid

## Сборка

```
cargo build --release
```

## Использование

```
extract depot version [options]
extract file.sim [file2.sim ...] [options]

depot             depot id
version           версия
file.sim          манифест .sim, по одному на диск, по порядку

--blob-dir dir    директория blob, скачивается автоматически (по умолчанию: steam2_cache/blobs)
--dat-dir dir     директория dat, скачивается автоматически (по умолчанию: steam2_cache/dats)
--blobcrc crc     crc blob, требуется только после сброса депо
--key key         ключ депо, hex или depot:hex, можно указывать несколько раз
--keys-file file  файл с парами "depot" "key", по одной на строку, можно указывать несколько раз
--filter regex    извлекать только пути, соответствующие regex
--out dir         директория для результата
--offline         офлайн, только локальные файлы

extract donate    показать адреса для поддержки проекта
```

## Донат

| | |
|---|---|
| Bitcoin | `bc1q2e60ws5yy6m5czv5wtc97z28vp8et3quvzx35c` |
| Solana | `HgqXV2YMWQggDJNDWzJ3rrczwhhy9AAUq7xZcziBqHbC` |
| ETH/Base/BNB | `0xff7b4d8be072dd36eb221c64cdc6ba48cce83b7e` |
