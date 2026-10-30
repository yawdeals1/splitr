# Download Splitr

Splitr splits one PDF into several files by chapter, invoice, student or any condition you pick. Your files stay on your computer.

<!-- Maintainers: the source repo is private, so release.yml publishes each release as a draft in
     the public repo yawdeals1/splitr-releases. Publish the draft there before sharing this page;
     a draft is not shown on the Releases page to anyone outside the repo. That repo's README.md
     is a copy of this page and its user-guide.md a copy of docs/user-guide.md: when either
     changes here, copy it there too (links to download.md become README.md). -->

**[Download Splitr from the Releases page](https://github.com/yawdeals1/splitr-releases/releases)**

On the Releases page, the newest version is at the top. Open its **Assets** and pick the file for your computer:

| Your computer | Download | Notes |
| --- | --- | --- |
| Windows 10 or 11 (64-bit) | `Splitr_<version>_x64-setup.exe` | Recommended. `Splitr_<version>_x64_en-US.msi` also works, for example for managed installs |
| Mac with Apple Silicon (M1 or later) | `Splitr_<version>_aarch64.dmg` | macOS 11 or later |
| Mac with an Intel processor | `Splitr_<version>_x64.dmg` | macOS 11 or later. To check your Mac, open the Apple menu > About This Mac |
| Linux (Ubuntu 22.04 or similar) | `Splitr_<version>_amd64.AppImage` | Runs without installing |
| Debian or Ubuntu | `Splitr_<version>_amd64.deb` | Installs like other apps |

Then follow the steps for your system below. When it's installed, the [user guide](user-guide.md) walks you through your first split.

## Why your computer warns you

Splitr's installers are not code-signed yet. Signing costs money every year and is planned if Splitr is shared more widely. Until then, Windows and macOS show a warning the first time you open it. The steps below get past that warning safely. Only do this for files you downloaded from the official release page linked above.

## Windows

1. Double-click `Splitr_<version>_x64-setup.exe`.
2. If you see **Windows protected your PC** (Microsoft Defender SmartScreen), click **More info**, check that the app name says `Splitr_<version>_x64-setup.exe`, then click **Run anyway**.
3. Follow the installer. If your PC doesn't have the Microsoft Edge WebView2 runtime, the installer adds it. This needs an internet connection.
4. Open Splitr from the Start menu.

If your browser blocks the download itself, choose **Keep** (Edge: **...** > **Keep** > **Show more** > **Keep anyway**).

To uninstall, go to **Settings > Apps > Installed apps**, find Splitr, then choose **Uninstall**.

## macOS

1. Open the `.dmg` file and drag **Splitr** into **Applications**.
2. Open Splitr from Applications. macOS says it can't check the app for malicious software, or that Apple could not verify it. Click **Done** or **OK**. Don't move it to the Bin.
3. Open **System Settings > Privacy & Security**, scroll to **Security**, and next to the message about Splitr click **Open Anyway**. Enter your password if asked.
4. Open Splitr again and click **Open**. From now on it opens normally.

On macOS 14 and earlier you can also Control-click Splitr in Applications, choose **Open**, then click **Open** again.

If macOS says **"Splitr" is damaged and can't be opened**, the app isn't broken. macOS says this about unsigned apps that still carry the download quarantine flag. Open **Terminal** and run this command, then open Splitr again:

```sh
xattr -dr com.apple.quarantine /Applications/Splitr.app
```

To uninstall, drag Splitr from Applications to the Bin.

## Linux

**AppImage**

1. Make the file executable: right-click it, choose **Properties > Permissions**, and turn on **Allow executing file as program**. Or run `chmod +x Splitr_<version>_amd64.AppImage`.
2. Double-click it, or run `./Splitr_<version>_amd64.AppImage`.

If it doesn't start on Ubuntu 22.04 or later, install FUSE 2 with `sudo apt install libfuse2` (on Ubuntu 24.04: `sudo apt install libfuse2t64`).

**Debian or Ubuntu package**

```sh
sudo apt install ./Splitr_<version>_amd64.deb
```

Splitr then appears in your applications. To uninstall: `sudo apt remove splitr`.

## Privacy

- Splitr opens and splits PDFs on your computer. It never changes your original file.
- Custom and Fixed splits, and the bookmark, text pattern and blank page methods that some presets use, work fully offline. Splitr only goes online for AI, and only when you've turned AI on.
- The optional Smart (AI) mode sends only short text excerpts of each page to the AI service you choose, with your own API key, and only after you agree. You can turn it off in Settings.
- Your API keys are kept in your system's secure storage (Windows Credential Manager, macOS Keychain or Linux Secret Service), never in a plain file.
- Splitr collects no analytics.

## System requirements

| System | Minimum |
| --- | --- |
| Windows | Windows 10 (64-bit) or later |
| macOS | macOS 11 or later, Apple Silicon or Intel |
| Linux | 64-bit with WebKitGTK 4.1, such as Ubuntu 22.04 or later |

8 GB of memory is recommended for PDFs over 500 pages.
