# Installing Postlane

## macOS

Download the `.dmg` from the [Releases](https://github.com/postlane/desktop/releases/latest) page.
Open it, drag Postlane to Applications, and launch normally.

The app is notarized by Apple — no Gatekeeper prompt or quarantine warning should appear.
If you get "damaged and can't be opened" despite this, run:

```
xattr -cr /Applications/Postlane.app
```

then try launching again. This removes any quarantine flag applied by your browser.

## Linux

Download the `.AppImage` from the [Releases](https://github.com/postlane/desktop/releases/latest) page.

```bash
chmod +x postlane_VERSION_amd64.AppImage
./postlane_VERSION_amd64.AppImage
```

### Verifying the signature

Each release ships a `.AppImage.asc` GPG signature and a `.AppImage.cosign.bundle` Sigstore attestation.

**GPG:**
```bash
# Import the key once
gpg --keyserver hkps://keys.openpgp.org --recv-keys FC2CFF33AB10A0E8
# Or import from https://postlane.dev/pgp

# Verify
gpg --verify postlane_VERSION_amd64.AppImage.asc postlane_VERSION_amd64.AppImage
```

**Sigstore (cosign):**
```bash
cosign verify-blob postlane_VERSION_amd64.AppImage \
  --bundle postlane_VERSION_amd64.AppImage.cosign.bundle \
  --certificate-identity-regexp 'https://github.com/postlane/desktop' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

### .deb (Debian/Ubuntu)

A `.deb` package is also available. Install with:

```bash
sudo dpkg -i postlane_VERSION_amd64.deb
```

The `.deb` is GPG-signed. Verify with `dpkg-sig --verify postlane_VERSION_amd64.deb` after importing the key above.

## Windows

Download the `_x64-setup.exe` from the [Releases](https://github.com/postlane/desktop/releases/latest) page and run it.

**SmartScreen warning:** Windows may show "Windows protected your PC" the first time you run the installer.
Click **More info**, then **Run anyway**. This is expected until the app has enough usage history to
build SmartScreen reputation. The installer is signed for auto-update integrity but not yet
with a Microsoft-trusted EV certificate.
