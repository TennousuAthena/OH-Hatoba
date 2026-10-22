# OH-Hatoba development

This is a history-preserving downstream of scarletkc/Hatoba. Read `README.ohos.md` and
`platform/harmonyos/docs/upstream-updates.md` before changing platform boundaries.

- Keep React business UI, vault formats, SSH algorithms, AI workflows and sync protocol shared.
  Platform implementations live in `platform/harmonyos`, `scripts/harmonyos` and the isolated
  `apps/desktop/src/ipc/harmonyos` transport. Do not copy business modules into an OHOS tree.
- `createCommandApi` in the existing IPC adapter is the single command/argument mapping. Its
  generated inputs come from upstream bindings. Never hand-edit `*.generated.*` files or build
  a second command table. Regenerate and review with `node scripts/harmonyos/contracts.mjs --write`.
- Preserve the desktop/mock hosts. A production Harmony build must fail visibly when its native
  host is missing, and must never fall back to mock data or demo file paths.
- The OHOS Rust target reports `target_os="linux"`, `target_env="ohos"`. Do not import desktop
  Linux crates or test `target_os="ohos"`.
- Minimum API20, ARM64, both `2in1` and `tablet`. New SDK APIs need compatibility guards and
  device evidence; successful compilation is not successful device validation.
- Keep synced configurations for unsupported local processes intact. Device capabilities are
  local; no stdio MCP, local shell, desktop updater or SSH-agent discovery in the common v1.
- Do not print or commit signing material, credentials, user vaults, transcripts or terminal
  contents. Base build profiles stay unsigned. Local signing data belongs in ignored files.
- Run contract checks, host-boundary checks, TypeScript checks, IPC tests and native probe tests
  after changing a boundary. Record target/SDK and distinguish host tests from device tests.
- An upstream version bump requires an explicit candidate commit, compatibility audit, reviewed
  generated diff, desktop regressions and OHOS validation. Do not advance the baseline JSON to
  silence failed checks. Do not force-push shared history or create external PRs unasked.
