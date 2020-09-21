FROM centos:7.8.2003 as builder
WORKDIR /build

# install tools and dependencies
# RUN apt-get update && \
#         apt-get install -y \
#         g++ \
#         build-essential \
#         curl \
#         git \
#         file \
#         binutils \
#         libssl-dev \
#         pkg-config \
#         libudev-dev

RUN yum update -y && \
  yum install -y \
  git \
  gcc-c++ \
  make \
  curl \
  file \
  openssl-devel
  
# install rustup
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# rustup directory
ENV PATH /root/.cargo/bin:$PATH

# show backtraces
ENV RUST_BACKTRACE 1

# show tools
RUN rustc -vV && \
cargo -V && \
gcc -v &&\
g++ -v

# build codechain
ADD . /build/codechain
RUN cd codechain && \
        cargo build --release --verbose && \
        ls /build/codechain/target/release/codechain && \
        strip /build/codechain/target/release/codechain

RUN file /build/codechain/target/release/codechain


FROM centos:7.8.2003
WORKDIR /app/codechain
RUN yum update -y && \
  yum install -y \
  openssl-libs
COPY --from=builder /build/codechain/target/release/codechain ./target/release/codechain
COPY --from=builder /build/codechain/codechain/config/presets/ ./codechain/config/presets

# show backtraces
ENV RUST_BACKTRACE 1

EXPOSE 3485 8080
ENTRYPOINT ["target/release/codechain"]
