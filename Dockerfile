FROM rust:1.65 as builder

WORKDIR /work

RUN wget https://github.com/protocolbuffers/protobuf/releases/download/v21.5/protoc-21.5-linux-x86_64.zip && \
  unzip protoc-21.5-linux-x86_64.zip && \
  mv ./bin/protoc /usr/local/bin/ && \
  rm -rf protoc-21.5-linux-x86_64.zip include readme.txt
COPY . ./

RUN cargo build --release

FROM gcr.io/distroless/cc

COPY --from=builder /work/target/release/mini-realtime-server .

CMD ["./mini-realtime-server"]
