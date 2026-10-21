# Mobile Development

zxySSH uses the shared Tauri application for desktop, iOS, and Android. The
mobile runtime supports foreground SSH, terminal input, host-key verification,
remote SFTP browsing, and pasted private keys. Desktop-only local shells,
updater/process plugins, service IPC and directory transfer are disabled by the
backend capability contract and mobile ACL. The 2.0 narrowing also removed the
tunnel and coding-agent surfaces, so they are not expected to reappear here.

## Toolchain Rule

Use the rustup-managed toolchain for cross-target commands. A separately
installed Homebrew `cargo` cannot see targets installed by rustup.

```bash
rustup run stable cargo check \
  --manifest-path src-tauri/Cargo.toml \
  --target aarch64-apple-ios-sim
```

## iOS

The Xcode project is not committed; releases ship Linux desktop and CLI
artifacts only. Generate it locally when iOS work is actually needed:

```bash
npx tauri ios init
```

Required Rust targets:

```bash
rustup target add \
  aarch64-apple-ios \
  aarch64-apple-ios-sim \
  x86_64-apple-ios
```

Required host tools are Xcode, XcodeGen, CocoaPods, and libimobiledevice. A
simulator build also requires at least one iOS Simulator runtime installed in
Xcode. `xcodebuild -showsdks` showing an iOS SDK is not sufficient; verify the
runtime separately:

```bash
xcrun simctl list runtimes
npx tauri ios build --debug --target aarch64-sim --ci
```

A development team and signing certificate are required for a physical device
or archive, but not for the Rust cross-target check.

## Android

Required Rust targets:

```bash
rustup target add \
  aarch64-linux-android \
  armv7-linux-androideabi \
  i686-linux-android \
  x86_64-linux-android
```

Tauri 2.10 currently requests NDK `29.0.13846066` during project
initialization. Configure a user-owned Android SDK and accept the Google SDK
licenses yourself before running initialization:

```bash
export ANDROID_HOME="$HOME/Library/Android/sdk"
export NDK_HOME="$ANDROID_HOME/ndk/29.0.13846066"
export ANDROID_NDK_HOME="$NDK_HOME"
export JAVA_HOME="/path/to/jdk"

npx tauri android init --ci
npx tauri android build --debug --target aarch64
```

Do not bypass the NDK version check by relabeling an older NDK. The generated
Android project belongs under `src-tauri/gen/android` and is intentionally not
ignored by Git.

## Release Boundaries

- Mobile SSH sessions are foreground-only. The app does not promise that a
  socket survives operating-system suspension.
- Private keys can be pasted on mobile. Native document import/export remains
  a later adapter and path-based upload/download stays hidden.
- Saved credentials are opt-in, device-local, and excluded from cloud sync.
- Real-device release testing still needs iOS signing and an Android SDK whose
  licenses were accepted by the developer running the build.
