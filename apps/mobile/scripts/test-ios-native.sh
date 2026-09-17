#!/bin/sh
# Compiles the UIKit-free iOS plugin logic together with its host-side tests using plain swiftc on macOS.
# No device, simulator or xcodebuild is involved; the real source file is compiled as is.
set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
sources="$root/plugins/remote/ios/Sources/VelaRemotePlugin"
out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT
swiftc -swift-version 5 -o "$out/ios-native-tests" \
  "$sources/TrustPromptCoordinator.swift" \
  "$root/scripts/ios-native-tests/main.swift"
"$out/ios-native-tests"
