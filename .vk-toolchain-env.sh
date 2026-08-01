# Toolchain bootstrap for this sandbox (not part of the repo's normal setup).
#
# The sandbox ships node + bun but no pnpm, no Rust, and no C compiler on PATH.
# All three are obtainable: pnpm from npm, Rust from static.rust-lang.org, and
# gcc/binutils already exist in the Nix store but are not on PATH.
#
# Usage:  source .vk-toolchain-env.sh
export PATH="$HOME/.local/bin:$PATH"                     # pnpm 10.13.1
export RUSTUP_HOME="$HOME/.rustup" CARGO_HOME="$HOME/.cargo"
export PATH="$CARGO_HOME/bin:$PATH"                      # nightly-2025-12-04
export PATH="/nix/store/788mx070y81zjlg5ipcl0cra3afviw9k-gcc-wrapper-15.2.0/bin:$PATH"
export PATH="/nix/store/kfwagnh6i1mysf7vxq679rzh30z9zj3g-binutils-wrapper-2.46/bin:$PATH"
export PATH="/nix/store/8i2h8b6nykwi6rj3ya5x4sysljg8zmg4-pkg-config-0.29.2/bin:$PATH"
export PATH="/nix/store/0afwlg1nchhabaxdcrpgaib0w2zrngcr-perl-5.42.0-env/bin:$PATH"

# openssl-sys otherwise tries to build vendored OpenSSL from source. Point it at
# the store's prebuilt copy; the -dev output carries the headers and .pc files,
# the plain output the shared libraries.
export PKG_CONFIG_PATH="/nix/store/dy64cxaygvmjfznysgxk501yds8jij6s-openssl-3.6.1-dev/lib/pkgconfig"
export OPENSSL_NO_VENDOR=1

# libsqlite3-sys runs bindgen, which dlopens libclang at build time.
export LIBCLANG_PATH="/nix/store/jdgw7h0g0l8clmcasaspxnx6v62jz1il-clang-21.1.8-lib/lib"
