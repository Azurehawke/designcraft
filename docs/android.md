# DesignCraft on Android (experimental)

DesignCraft's desktop interface running on Android tablets, meant for use with a keyboard, an
S Pen and, if you like, a mouse. Built from the `android` branch by
`.github/workflows/android.yml` for arm64 devices running Android 11 or later. The approach is
the one the PdfCraft, VectorCraft and GridCraft Android ports use (a NativeActivity APK built
without Gradle).

## Install

1. On the tablet, open the repository's **Releases** page and download `designcraft.apk` from
   the `android-latest` prerelease.
2. Open the downloaded file. Android asks to allow installs from your browser or Files app the
   first time; allow it, then install. If Play Protect warns about an unknown app, choose
   *More details ▸ Install anyway*.
3. Give DesignCraft file access. It is a special access, not one of the app's regular
   permissions: the first time you open, place or save a file, the file browser shows **Allow
   access…**, which opens the switch in Settings. Or turn it on yourself: **Settings ▸ Apps ▸ ⋮ ▸
   Special access ▸ All files access ▸ DesignCraft**. Without it DesignCraft can only open and save
   files in its own app folder.

Later builds install over the earlier one as long as they are signed with the same key.

## Updates

DesignCraft updates itself from the GitHub repository it was built from. At start-up, and at
most once an hour after that (when it's open or comes back to the front), it asks GitHub for the
`android-latest` release. When that is a newer build than the one installed, a card at the bottom
right offers it:

1. **Update** downloads the APK inside the app, with a progress bar.
2. The first time, Android asks you to allow DesignCraft to install apps: turn on **Allow from
   this source** in the settings that open, come back and tap **Install**.
3. Android shows its own install window: tap **Update**. DesignCraft closes and comes back on the
   new build, so save your documents first.

**Later** hides the card for that build until DesignCraft next starts. Being offline is fine: a
check that fails says nothing and tries again later. Android only installs an update signed with
the same key as the installed app, so a download that isn't one of this repository's builds is
refused.

## Open documents from other apps

DesignCraft appears in **Open with** for IDML and InDesign files, and for files Android has no
type for, which is how Files, My Files and Gmail send `.designcraft` documents. A document from a
file in your storage opens as that file, and Save writes it in place (with All files access). A
document that only exists in another app (an email attachment, a Drive file) is first saved as a
copy in **Documents/DesignCraft** (or the app folder, without All files access), and that copy is
what you edit and save.

## How it differs from the desktop app

- **Files:** Open, Place, Save As, Save a Copy, the exports (PDF, PNG, IDML, EPUB, HTML…),
  Package, Relink, Load Swatches, libraries, books and data merge sources use DesignCraft's own
  file browser, with shortcuts to Internal storage, Documents, Download, Pictures, SD cards and
  USB drives, and the app folder. A tap opens a folder; tap a file, then the button. A few panel
  buttons (in the Links, Libraries and Book panels, for example) can't be run again by the
  browser: after you choose the file, the status bar says so, and the button's next tap uses it.
- **Touch:** a finger tap clicks, a finger drag scrolls the page, and a finger held still for
  half a second right-clicks (context menus). Two fingers pan, and pinch to zoom. Fingers don't
  draw or move objects: use the S Pen or a mouse.
- **S Pen:** works like a mouse with the button held, with whichever tool is chosen: select and
  drag objects, draw frames and paths, set the text cursor and select text, use menus and panels.
  While the pen is on the screen, fingers and a resting palm are ignored.
- **Mouse:** a paired Bluetooth or USB mouse works as on a desktop (hover, right-click, wheel,
  Ctrl+wheel zoom), and so does a touchpad on a keyboard cover.
- **Keyboard:** with a keyboard cover or Bluetooth keyboard attached, the on-screen keyboard stays
  away; without one, it comes up when you type into a text frame or a field. Desktop shortcuts
  work.
- **Clipboard:** text you copy goes to other apps, and Paste takes text copied in other apps.
  Objects copied on the page stay inside DesignCraft.
- **Fonts:** Android's own (Roboto, Noto, including the CJK fonts) and DesignCraft's bundled ones.
  For more, put font files in a **Fonts** folder at the top of Internal storage (with All files
  access) and restart DesignCraft.
- **Crash recovery:** Android may stop an app in the background without warning. DesignCraft
  writes recovery copies of unsaved documents whenever it leaves the screen, and reopens them on
  the next start.
- **Not on Android:** printing to a printer (export a PDF instead) and the `--control`
  automation channel.
- Settings (`ui.json`, `prefs.json`) and recovery live in the app's private folder; uninstalling
  removes them.

## Build it yourself

Needs the Android SDK and NDK, `rustup target add aarch64-linux-android` and
`cargo install cargo-ndk`:

```sh
ANDROID_HOME=~/Android/Sdk ANDROID_NDK_HOME=~/Android/Sdk/ndk/<version> packaging/android/build-apk.sh
adb install -r target/android-apk/designcraft.apk
adb logcat -s DesignCraft   # the app's log
```

The build log of every CI run is also pushed to the `ci-logs` branch (`build.log`, and the
compiler errors alone in `errors.txt`).

## Signing

The APK is signed with `packaging/android/debug.keystore` (password `android`), which is public,
like Android's own debug key. To use a private key instead, add the repository secrets
`ANDROID_KEYSTORE_B64` (the keystore, base64), `ANDROID_KEYSTORE_PASSWORD` and
`ANDROID_KEY_ALIAS`. Android won't install an update signed with a different key over an
installed app: uninstall the debug-signed one first.

## How it's put together

| Part | What it does |
|---|---|
| `apps/designcraft-android` | The `cdylib` Android loads: `android_main` and the host services (`lib.rs`), `tablet.rs` (insets, system bars, keyboard, clipboard sync, going to the background), `touch.rs` (pen, finger and palm routing), `files.rs` (the file browser), `file_access.rs`, `incoming.rs`, `clipboard.rs`, `updater.rs` (update check and card) |
| `crates/ui-egui` | Unchanged UI, plus one host hook any platform can use: `file_asker()` names the command or dialog asking for a file, and `DesignApp::repeat` runs it again once the host has the answer |
| `crates/fonts` | Android's font folders |
| `vendor/winit`, `vendor/android-activity` | winit 0.30.13 with the Android patches (tool type per touch, mouse and touchpad as a mouse) and DesignCraft's macOS termination patch; android-activity with a 64 MiB UI thread stack. See `vendor/README.md` |
| `packaging/android` | Manifest, the Java activity (intents, the updater's HTTP and PackageInstaller install, system bar colours), signing key, `build-apk.sh` |
