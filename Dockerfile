FROM rust:1.88-alpine AS builder

RUN apk add --no-cache musl-dev
WORKDIR /app
COPY . .
RUN cargo build --release --locked

FROM alpine:3.20

RUN apk add --no-cache ca-certificates tzdata
WORKDIR /app
COPY --from=builder /app/target/release/sub_manager_lite .
RUN mkdir -p /data
ENV DATABASE_URL="sqlite:////data/sub_manager.db"
EXPOSE 8000
CMD ["./sub_manager_lite"]
