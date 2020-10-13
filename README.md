# BazaarRealmClient

A Rust DLL that handles making requests to the `BazaarRealmAPI` web server
for the [`BazaarRealmPlugin`](https://github.com/thallada/BazaarRealmPlugin),
part of the Bazaar Realm Skyrim mod.

This project is still a bit of a mess at the moment. But, essentially it uses
[`reqwest`](https://crates.io/crates/reqwest) to make requests to the API,
deserializes the data with [serde](https://crates.io/crates/serde), and saves
the responses to files in the Skyrim data directory to use as a local cache
when the API server is unavailable.

[cbindgen](https://crates.io/crates/cbindgen) automatically generates the
header file needed for the `BazaarRealmPlugin` (written in C++) to call into
this DLL.