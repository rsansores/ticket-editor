# Releasing

Two packages ship from this repo, always at the **same version** (the editor's
wasm is compiled from `ticket-core`, so a version mismatch silently breaks the
1:1 preview/print parity):

| Package | Registry | Source |
|---|---|---|
| `ticket-core` | crates.io | `crates/ticket-core` |
| `@ticket-editor/vue` | npm | `packages/ticket-editor` |

Publishing is automated by [`.github/workflows/release.yml`](.github/workflows/release.yml),
which uses **OIDC Trusted Publishing** — no npm or crates.io tokens are stored in
this repo. The publish step runs in a protected environment and **waits for your
approval**. Third-party actions are pinned to commit SHAs.

---

## One-time setup

Do this once, before the first automated release.

### 1. Secure the accounts
- Enable **2FA** on both [crates.io](https://crates.io/settings/profile) and
  [npmjs.com](https://www.npmjs.com/settings/~/profile).

### 2. Create the npm org (for the `@ticket-editor` scope)
- npmjs.com → **Add Organization** → name `ticket-editor` → **Free** (unlimited
  public packages). Your user `rsansores` is the owner.

### 3. First publish — by hand (claims the names, lets you eyeball the artifacts)

Trusted Publishing can only be attached to a package that already exists, so the
very first version is manual. From a clean checkout with the Rust + wasm
toolchain installed (`rustup target add wasm32-unknown-unknown`,
`cargo install wasm-bindgen-cli`):

```bash
# --- crate ---
cargo login                       # paste a token from crates.io/settings/tokens
cargo publish -p ticket-core

# --- npm package ---
cd packages/ticket-editor
npm login                         # your npm account (2FA)
pnpm build:wasm && pnpm build     # produces src/wasm + dist (with types)
npm publish --access public       # no --provenance here: it only works from CI (OIDC)
```

> Tip: `cargo publish --dry-run -p ticket-core` and `pnpm pack` let you inspect
> exactly what will ship before you commit.

### 4. Register the Trusted Publishers (removes the need for tokens forever after)

- **crates.io** → your crate → **Settings → Trusted Publishing → Add**:
  - Repository owner: `rsansores`
  - Repository name: `ticket-editor`
  - Workflow filename: `release.yml`
  - Environment: `release`

- **npm** → the package → **Settings → Publishing access → Trusted Publisher →
  GitHub Actions**:
  - Organization/user: `rsansores`
  - Repository: `ticket-editor`
  - Workflow filename: `release.yml`
  - Environment: `release`

### 5. Create the protected `release` environment (the approval gate)

- GitHub repo → **Settings → Environments → New environment** → name `release`.
- Under **Deployment protection rules**, enable **Required reviewers** and add
  **yourself**. Now every automated release pauses until you click **Approve**.

> Until this environment exists with a required reviewer, the pipeline still
> runs but without the human gate — so don't skip this step.

---

## Cutting a release (every time after setup)

1. Make sure `master` is green and holds everything you want to ship.
2. GitHub → **Actions → Release → Run workflow** → enter the version (e.g.
   `0.2.0`) → **Run**.
3. The `verify` job runs tests, clippy, typecheck, and lint.
4. The `release` job then requests approval — you'll get a prompt. **Approve** it.
5. CI bumps `Cargo.toml` + `package.json` to the new version, commits, tags
   `vX.Y.Z`, then publishes the crate and the npm package (with provenance),
   using short-lived OIDC credentials.

That's it. No tokens, one approval click.

---

## Notes & gotchas

- **Versions are immutable.** crates.io cannot be un-published (only yanked); npm
  un-publish is restricted after 72h. Pick the version deliberately.
- **Lockstep is enforced by `scripts/set-version.sh`** — it sets both manifests
  from one input. Don't bump them by hand.
- **The bump commit is pushed to `master`** by the workflow. If you later protect
  `master` with required PRs, allow GitHub Actions to bypass, or the push fails.
- **Parity check before a big release** (optional but recommended): build the wasm
  and confirm the native and wasm renderers still produce identical bytes before
  dispatching (`./scripts/build-wasm.sh`, then the parity scripts under
  `packages/ticket-editor/`).
- **Consumers need no Rust.** The published npm tarball contains the built wasm;
  only maintainers building a release need the Rust toolchain.
