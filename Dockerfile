# Use Rust with MUSL
FROM rust:1.83 as builder

# Install MUSL tools for building static binaries
RUN apt-get update && apt-get install -y musl-tools

# Set the target directory for building
WORKDIR /app

# Copy the project files into the container
COPY . .

# Add the MUSL target to Rust
RUN rustup target add x86_64-unknown-linux-musl

# Build the example as a static binary targeting MUSL
RUN cargo build --release --example echo_server --target x86_64-unknown-linux-musl

# Use the smallest possible image as the runtime
FROM scratch

# Set the working directory in the runtime image
WORKDIR /app

# Copy the statically linked binary from the builder stage
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/examples/echo_server ./echo_server

# Expose the application's port
EXPOSE 9002

# Run the static binary
CMD ["./echo_server"]