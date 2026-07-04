# AUR packaging

`PKGBUILD` here is the source-of-truth template for the `jinja-lsp-bin` AUR
package. It installs the pre-built `x86_64`/`aarch64` Linux binary from the
matching GitHub Release — no Rust toolchain required on the user's machine.

`pkgver` and both `sha256sums_*` fields are placeholders (`0.1.0` / `'SKIP'`).
The release workflow (`.github/workflows/release.yml`, `aur-publish` job)
renders real values from the just-published release for every tagged version
— do not hand-edit `pkgver`/checksums here; edit metadata (`pkgdesc`, `url`,
`license`, …) only.

## One-time setup (maintainer)

Publishing requires an AUR account with the `jinja-lsp-bin` package name
reserved, plus repo secrets so CI can push to it:

1. Create an account at <https://aur.archlinux.org> if you don't have one.
2. Generate a dedicated SSH keypair for CI (don't reuse your personal key):
   ```bash
   ssh-keygen -t ed25519 -f aur_ci_key -N "" -C "jinja-lsp CI"
   ```
3. Add the **public** key (`aur_ci_key.pub`) to your AUR account under
   *My Account → SSH Public Key*.
4. Reserve the package: clone `ssh://aur@aur.archlinux.org/jinja-lsp-bin.git`
   (an empty repo — AUR creates it lazily on first push) and push a copy of
   `aur/PKGBUILD` plus a generated `.SRCINFO` (`makepkg --printsrcinfo > .SRCINFO`)
   once, so the package exists before CI's first automated update.
5. Add these secrets to the GitHub repo (*Settings → Secrets and variables →
   Actions*, environment `release`):
   - `AUR_USERNAME` — your AUR account username
   - `AUR_EMAIL` — the email associated with your AUR account
   - `AUR_SSH_PRIVATE_KEY` — the **private** half of the keypair from step 2

Once these are set, every push of a `vX.Y.Z` tag updates the AUR package
automatically as the last step of the release pipeline (`aur-publish` job in
`.github/workflows/release.yml`) — plain `git`/`ssh` against
`aur.archlinux.org`, no third-party GitHub Action, mirroring `../babel-lsp`'s
release workflow. The private key is written to `~/.ssh/aur_ed25519` for the
duration of that job only.

## Local testing

```bash
cd aur
# fill in a real pkgver + sha256sums (see the release workflow step for how),
# then:
makepkg --printsrcinfo > .SRCINFO
makepkg -si   # build + install locally, on Arch or in an archlinux container
```
