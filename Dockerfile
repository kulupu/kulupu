# Originally developed by Polkasource, licensed under Apache-2.

# ===== START FIRST STAGE ======
FROM phusion/baseimage:0.11 as builder
LABEL maintainer "hi@that.world"
LABEL description="Kulupu builder."

ARG PROFILE=release
WORKDIR /rustbuilder
COPY . /rustbuilder/kulupu

# PREPARE OPERATING SYSTEM & BUILDING ENVIRONMENT
RUN apt-get update && \
	apt-get upgrade -y && \
	apt-get install -y cmake pkg-config libssl-dev git clang libclang-dev

# UPDATE RUST DEPENDENCIES
ENV RUSTUP_HOME "/rustbuilder/.rustup"
ENV CARGO_HOME "/rustbuilder/.cargo"
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH "$PATH:/rustbuilder/.cargo/bin"
RUN rustup update nightly
RUN RUSTUP_TOOLCHAIN=stable cargo install --git https://github.com/alexcrichton/wasm-gc

# BUILD RUNTIME AND BINARY
RUN rustup target add wasm32-unknown-unknown --toolchain nightly
RUN cd /rustbuilder/kulupu && RUSTUP_TOOLCHAIN=stable RANDOMX_ARCH=default cargo build --$PROFILE
# ===== END FIRST STAGE ======

# ===== START SECOND STAGE ======
FROM phusion/baseimage:0.11
LABEL maintainer "hi@that.world"
LABEL description="Kulupu binary."
ARG PROFILE=release
COPY --from=builder /rustbuilder/kulupu/target/$PROFILE/kulupu /usr/local/bin

# REMOVE & CLEANUP
RUN mv /usr/share/ca* /tmp && \
	rm -rf /usr/share/*  && \
	mv /tmp/ca-certificates /usr/share/ && \
	rm -rf /usr/lib/python* && \
	mkdir -p /root/.local/share/kulupu && \
	ln -s /root/.local/share/kulupu /data
RUN	rm -rf /usr/bin /usr/sbin

# FINAL PREPARATIONS
EXPOSE 30333 9933 9944
VOLUME ["/data"]
#CMD ["/usr/local/bin/kulupu"]
WORKDIR /usr/local/bin
ENTRYPOINT ["kulupu"]
CMD ["--chain=kulupu"]
# ===== END SECOND STAGE ======
