#!/bin/sh

set -euxo pipefail

: "${LIBNAME:=libciteproc_rs}"
: "${TARGET:=../../target}"
: "${OUTPATH:=$TARGET/xcframework}"
: "${OUTNAME:=CiteprocRs}"
# needs PR https://github.com/rust-lang/rust/pull/87699
# install with `rustup install $TOOLCHAIN && rustup component add rust-src --toolchain $TOOLCHAIN`
# : "${TOOLCHAIN:=nightly-2021-10-07}"
: "${CONFIGURATION:=test}"

export IPHONEOS_DEPLOYMENT_TARGET=13.0
export MACOSX_DEPLOYMENT_TARGET=10.10

rustup show
env

PLATFORMS="
apple-darwin
apple-ios
apple-ios-sim
"

# test is an alias for debug, but it adds --features testability
if [ "$CONFIGURATION" = "test" ]; then
  CONFIGURATION=debug
  : "${FEATURES:=--features testability}"
  PLATFORMS="apple-darwin apple-ios"
else
  : "${FEATURES:=}"
fi

: "${PROFDIR:=$CONFIGURATION}"

# PLATFORMS="apple-darwin apple-ios"
# PLATFORMS="apple-ios"

suffixes=$(mktemp -d)
echo "macos" > $suffixes/apple-darwin
echo "ios" > $suffixes/apple-ios
echo "ios-simulator" > $suffixes/apple-ios-sim
echo "ios-macabi" > $suffixes/apple-ios-macabi

# lipo doesn't like x86-64 on ios. x86_64 is only relevant to the apple-ios-sim platform.
targets=$(mktemp -d)
echo "aarch64-apple-ios" > $targets/apple-ios
echo "aarch64-apple-ios-sim x86_64-apple-ios" > $targets/apple-ios-sim
echo "aarch64-apple-darwin x86_64-apple-darwin" > $targets/apple-darwin
echo "aarch64-apple-ios-macabi x86_64-apple-ios-macabi" > $targets/apple-ios-macabi

ARCHS="
aarch64
x86_64
"
subarchs=$(mktemp -d)
echo "arm64v8" > $subarchs/aarch64
echo "x86_64" > $subarchs/x86_64

trap "rm -rf -- $suffixes $targets $subarchs" EXIT

mkdir -p "$OUTPATH/$CONFIGURATION"
rm -rf "$OUTPATH/$CONFIGURATION/$OUTNAME.xcframework"

xc_args=""
for PLATFORM in $PLATFORMS
do
  lipo_args=""
  for TRIPLE in $(< $targets/$PLATFORM)
  do
    BUILD_STD=""
    RELEASE=""
    rustup target add $TRIPLE || BUILD_STD="-Z unstable-options -Z build-std"
    if [ "$CONFIGURATION" = "release" ]; then
      RELEASE="--release"
    fi

    cargo build -p citeproc-ffi $RELEASE \
        --target "$TRIPLE" \
        $FEATURES $BUILD_STD

    larch=$(< $subarchs/$(echo $TRIPLE | cut -d - -f 1))
    lipo_args="$lipo_args -arch $larch ../../target/$TRIPLE/$PROFDIR/$LIBNAME.a"
  done

  suffix=$(< $suffixes/$PLATFORM)
  lipo_output="$OUTPATH/$CONFIGURATION/$LIBNAME-$suffix.a"
  rm -f $lipo_output
  lipo -create $lipo_args -output "$lipo_output"

  xc_args="$xc_args -library $lipo_output"
  xc_args="$xc_args -headers ../ffi/modules/swift/include"
done

xcodebuild -create-xcframework $xc_args -output "$OUTPATH/$CONFIGURATION/$OUTNAME.xcframework"

