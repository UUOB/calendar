#!/usr/bin/env bash
# On a Mac (GitHub's Mac runner, see .github/workflows/mac.yml): build
# Calendar.app for Apple silicon, then the .dmg (first install) and
# the .app.tar.gz (what the Update button installs), in out/.
# Needs Rust with the aarch64-apple-darwin target, and Xcode.
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
build=mac/build gen=mac/build/gen app=mac/build/Calendar.app
rm -rf "$build" && mkdir -p "$gen" out

# 1. The core (with UUOB's SDK), for Apple silicon, as one library.
MACOSX_DEPLOYMENT_TARGET=14.0 cargo build --release --lib -p calendar-core --target aarch64-apple-darwin
cp target/aarch64-apple-darwin/release/libcalendar_core.dylib "$build/libcalendar_core.dylib"
install_name_tool -id @rpath/libcalendar_core.dylib "$build/libcalendar_core.dylib"

# 2. Its Swift bindings (UniFFI): Calendar's core and UUOB's SDK.
cargo run -q -p calendar-core --features bindgen --bin uniffi-bindgen -- \
  generate --library target/aarch64-apple-darwin/release/libcalendar_core.dylib --language swift --out-dir "$gen"

# 3. The app, for Apple silicon: its screens, UUOB's kit for apps, the bindings.
maps=(); for m in "$gen"/*.modulemap; do maps+=(-Xcc "-fmodule-map-file=$m"); done
for arch in arm64; do
  swiftc -O -parse-as-library -swift-version 5 -target "$arch-apple-macos14.0" -module-name CalendarApp \
    -I "$gen" "${maps[@]}" \
    mac/Sources/*.swift sdk/kit/mac/*.swift "$gen"/*.swift \
    -L "$build" -lcalendar_core -Xlinker -rpath -Xlinker @executable_path/../Frameworks \
    -o "$build/Calendar-$arch"
done

# 4. The "Up next" widget (mac/Widget), an extension inside the app.
ext="$app/Contents/PlugIns/UpNext.appex"
mkdir -p "$ext/Contents/MacOS"
swiftc -O -parse-as-library -application-extension -swift-version 5 -target arm64-apple-macos14.0 -module-name UpNext \
  mac/Widget/UpNext.swift -framework WidgetKit -Xlinker -e -Xlinker _NSExtensionMain -o "$ext/Contents/MacOS/UpNext"
sed "s/VERSION/$version/g" mac/Widget/Info.plist > "$ext/Contents/Info.plist"

# 5. Calendar.app, signed for this Mac only (no paid Apple account: macOS asks once, "Open Anyway").
# The widget is signed first with its sandbox (macOS runs widgets only sandboxed), then the rest.
mkdir -p "$app/Contents/MacOS" "$app/Contents/Frameworks" "$app/Contents/Resources"
cp "$build/Calendar-arm64" "$app/Contents/MacOS/Calendar"
cp "$build/libcalendar_core.dylib" "$app/Contents/Frameworks/"
cp mac/Calendar.icns "$app/Contents/Resources/"
sed "s/VERSION/$version/g" mac/Info.plist > "$app/Contents/Info.plist"
codesign --force --sign - "$app/Contents/Frameworks/libcalendar_core.dylib"
codesign --force --sign - --entitlements mac/Widget/UpNext.entitlements "$ext"
codesign --force --sign - "$app"

# 6. What gets published (the same names as before, so updates find them).
mkdir -p "$build/dmg" && cp -R "$app" "$build/dmg/" && ln -s /Applications "$build/dmg/Applications"
hdiutil create -volname Calendar -srcfolder "$build/dmg" -ov -format UDZO "out/Calendar_${version}_arm64.dmg"
(cd "$build" && COPYFILE_DISABLE=1 tar -czf "../../out/Calendar.app.tar.gz" Calendar.app)
ls -la out
