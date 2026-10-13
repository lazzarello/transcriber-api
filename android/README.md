# PTT Client

Minimal Kotlin Android scaffold for Phase 2.

- Launcher icon: tongue emoji.
- Screen: light pink background with a centered tongue emoji.
- Tapping the emoji changes the on-screen icon to water drops.

## Build and install

Open `android/` in Android Studio once to install/configure an Android SDK, or install SDK Platform 35 and Build Tools 35 manually. Then, from the repository root:

```sh
devenv shell
cd android
./gradlew assembleDebug
./gradlew installDebug
```

With a USB-connected, authorized developer device, check the connection with:

```sh
adb devices
```
