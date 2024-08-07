<p align="center">
  <img src="hiisi.png" alt="Hiisi" width="200"/>
  <h1 align="center">Hiisi</h1>
</p>

<p align="center">
Execute SQL remotely on  libSQL/SQLite databases.
</p>

<p align="center">
  <a href="https://github.com/penberg/limbo/blob/main/LICENSE.md">
    <img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT" title="MIT License" />
  </a>
</p>

---

## Why Hiisi?

SQLite is a versatile database, but serverless apps, for example, don't have persistent state to have an in-process database. Hiisi is a database server for remote SQL execution on libSQL/SQLite databases written in Rust, but follows [similar architecture as TigerBeetle](ARCHITECTURE.md) to support deterministic simulation testing (DST).

_Hiisi is an experimental proof-of-concept and is not suitable for production use._

## Features

- Support for libSQL [wire protocol](https://github.com/tursodatabase/libsql/blob/main/docs/HRANA_2_SPEC.md)
- Designed for massive multitenancy
- Deterministic simulation testing (DST)

## Getting Started

Simulator:

```
cd simulator && cargo run
```

Server:

```
cd server && cargo run
```

## FAQ

### How is Hiisi different from libSQL?

Hiisi is a proof-of-concept alternative to the libSQL server, which
provides the same functionality for remote SQL execution for
libSQL/SQLite databases. There is no hard dependency between the two
projects. Of course, if Hiisi becomes widely successful, we might
consider merging with libSQL, but that is something that will be decided
in the future.
