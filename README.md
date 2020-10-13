# BazaarRealmClient

A Rust DLL that handles making requests to the
[`BazaarRealmAPI`](https://github.com/thallada/BazaarRealmAPI) web server for
the [`BazaarRealmPlugin`](https://github.com/thallada/BazaarRealmPlugin),
part of the Bazaar Realm Skyrim mod.

This project is still a bit of a mess at the moment. But, essentially it uses
[`reqwest`](https://crates.io/crates/reqwest) to make requests to the API,
deserializes the data with [serde](https://crates.io/crates/serde), and saves
the responses to files in the Skyrim data directory to use as a local cache
when the API server is unavailable.

[cbindgen](https://crates.io/crates/cbindgen) automatically generates the
header file needed for the `BazaarRealmPlugin` (written in C++) to call into
this DLL.

Related projects:

* [`BazaarRealmAPI`](https://github.com/thallada/BazaarRealmAPI): API server
  for the mod that stores all shop data and what this client talks to
* [`BazaarRealmPlugin`](https://github.com/thallada/BazaarRealmPlugin): SKSE
  plugin for the mod that modifies data within the Skyrim game engine and calls
  the methods in this client
* [`BazaarRealmMod`](https://github.com/thallada/BazaarRealmMod): Papyrus
  scripts, ESP plugin, and all other resources for the mod