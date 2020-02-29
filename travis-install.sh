#!/bin/bash
curl -sL https://github.com/xd009642/tarpaulin/releases/download/0.11.0/cargo-tarpaulin-0.11.0-travis.tar.gz | tar xvz -C $HOME/.cargo/bin
echo "WARNING This install method may fail if using nightly features like doctest coverage. Consider using docker installs instead"
