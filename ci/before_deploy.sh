# This script takes care of building your crate and packaging it for release

set -ex

main() {
    local src=$(pwd) \
          stage=

    case $TRAVIS_OS_NAME in
        linux)
            stage=$(mktemp -d)
            ;;
        osx)
            stage=$(mktemp -d -t tmp)
            ;;
    esac

    test -f Cargo.lock || cargo generate-lockfile

    # TODO Update this to build the artifacts that matter to you
    cargo build --release 

    # TODO Update this to package the right artifacts
    cp target/release/cargo-tarpaulin $stage/

    cd $stage
    tar czf $src/$CRATE_NAME-$TRAVIS_TAG-travis.tar.gz *
    cd $src

    rm -rf $stage
}

main
