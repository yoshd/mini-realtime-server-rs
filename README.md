![test](https://github.com/yoshd/mini-realtime-server-rs/workflows/Test/badge.svg)

# mini-realtime-server-rs

This is a small real-time server implementation supporting multiple network protocols, written in Rust.

[API Definition](./protobuf/app.proto)

```
mini-realtime-server 0.1.0

Usage: mini-realtime-server [OPTIONS]

Options:
  -p, --protocol <PROTOCOL>                      [default: websocket]
  -a, --addr <ADDRESS>                           [default: 127.0.0.1:8000]
      --enable-auth-bearer <ENABLE_AUTH_BEARER>  [default: true] [possible values: true, false]
      --auth-bearer <AUTH_BEARER>                [default: test]
      --enable-tls <ENABLE_TLS>                  [default: false] [possible values: true, false]
      --tls-cert-file-path <TLS_CERT_FILE_PATH>  [default: ./server.crt]
      --tls-key-file-path <TLS_KEY_FILE_PATH>    [default: ./server.key]
  -h, --help                                     Print help information
  -V, --version                                  Print version information
```

# Articles

- TODO
