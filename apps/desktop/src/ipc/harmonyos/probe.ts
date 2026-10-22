import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import "./probe.css";
import { connectHost } from "./bridge";
import { createHarmonyApi } from "./api";
import { FRAME_DATA } from "../types";

const report = document.querySelector<HTMLPreElement>("#report")!;
const button = document.querySelector<HTMLButtonElement>("#probe")!;
const terminal = new Terminal({ cursorBlink: true, fontFamily: "monospace", fontSize: 15, theme: { background: "#0b1314", foreground: "#d9e8e5" } });
const fit = new FitAddon();
terminal.loadAddon(fit);
const container = document.querySelector<HTMLDivElement>("#terminal")!;
terminal.open(container);
terminal.writeln("OH-Hatoba — xterm 6 / ArkWeb 输入验证\r\n中文 · 全角字符 · emoji 🌊\r\n");
terminal.onData((data) => terminal.write(data === "\r" ? "\r\n" : data === "\x7f" ? "\b \b" : data));
new ResizeObserver(() => fit.fit()).observe(container);
document.querySelector("#keyboard")!.addEventListener("click", () => terminal.focus());
fit.fit();

void connectHost().then((bridge) => {
  button.disabled = false;
  button.textContent = "运行原生验证";
  const run = async () => {
    button.disabled = true;
    try {
      const native = await bridge.invoke("host_probe");
      report.textContent = JSON.stringify(native, null, 2);
      console.info("OH-Hatoba P0 probe rendered");
      // The native host gives this entry a separate vault directory. These generated fixtures
      // exercise the same typed facade and Rust commands used by the production application.
      const api = await createHarmonyApi(bridge);
      const assert = (condition: boolean) => { if (!condition) throw new Error("Runtime check failed"); };
      assert((await api.app_info()).platform === "harmonyos");
      const password = "OH-Hatoba generated diagnostic vault only";
      const status = await api.vault_status();
      if (status.state === "uninitialized") await api.vault_create(password);
      else if (status.state === "locked") await api.vault_unlock(password);
      const saved = await api.host_save({ id: null, name: "Generated diagnostic", address: "127.0.0.1", port: 22,
        username: "fixture", auth_kind: "agent", password: null, key_id: null, group_id: null, tags: ["diagnostic"],
        favorite: false, jump_host_id: null, proxy_mode: "direct", proxy_id: null, note: "", ai_notes: "", env: [] });
      assert((await api.host_get(saved.id)).auth_kind === "agent");
      await api.host_delete(saved.id);
      let locked = false;
      const unlisten = await api.listen("vault://locked", () => { locked = true; });
      await api.vault_lock();
      assert((await api.vault_status()).state === "locked" && locked);
      let refused = false;
      try { await api.hosts_list(); } catch { refused = true; }
      assert(refused);
      await api.vault_unlock(password);
      assert((await api.vault_status()).state === "unlocked");
      unlisten();
      const business = { typedFacade: true, vaultCreateOrReopen: true, hostCrudAndAgentPreservation: true, lockEvent: true, lockedReadDenied: true, unlock: true };
      report.textContent = JSON.stringify({ native, business }, null, 2);
      console.info("OH-Hatoba shared business smoke rendered");
      const network = await bridge.invoke("host_network_probe") as { enabled?: boolean; checks?: Record<string, boolean> };
      if (network.checks) {
        report.textContent = JSON.stringify({ native, business, network }, null, 2);
        assert(Object.values(network.checks).length >= 9 && Object.values(network.checks).every(Boolean));
        const unkey = await api.listen("ssh://hostkey-prompt", (prompt) => {
          void api.hostkey_respond(prompt.request_id, prompt.host === "127.0.0.1" && prompt.port === 22222);
        });
        const unauth = await api.listen("ssh://auth-prompt", (prompt) => {
          void api.auth_prompt_respond(prompt.request_id, prompt.prompts.map(() => "generated-diagnostic-only"));
        });
        const expected = new TextEncoder().encode("generated 中文 🌊\r\n".repeat(4096));
        const bytes: number[] = [];
        let parsed = 0;
        let finish: (() => void) | undefined;
        const received = new Promise<void>((resolve) => { finish = resolve; });
        let sid: string | undefined;
        try {
          sid = await api.ssh_connect_target({ address: "127.0.0.1", port: 22222, username: "fixture" },
            { cols: 80, rows: 24, password: null, passphrase: null }, (frame) => {
              if (frame[0] !== FRAME_DATA) return;
              bytes.push(...frame.subarray(1));
              return new Promise<void>((resolve) => terminal.write(frame.subarray(1), () => {
                parsed += frame.length - 1;
                resolve(); if (parsed >= expected.length) finish?.();
              }));
            });
          await api.ssh_write_bytes(sid, Array.from(expected));
          let timer: ReturnType<typeof setTimeout> | undefined;
          try {
            await Promise.race([received, new Promise<void>((_, reject) => { timer = setTimeout(() => reject(new Error("Terminal fixture timed out")), 10000); })]);
          } finally { clearTimeout(timer); }
          assert(bytes.length === expected.length && bytes.every((byte, i) => byte === expected[i]));
          assert((await api.sftp_list(sid, "/")).some((file) => file.name === "fixture.bin"));
          await api.ssh_resize(sid, 132, 40);
          console.info("OH-Hatoba Web SSH transport passed");
        } finally {
          if (sid) await api.ssh_disconnect(sid);
          unkey(); unauth();
        }
        console.info("OH-Hatoba network probe rendered");
      }
    } catch {
      report.textContent += "\n业务运行验证失败，请查看构建与设备日志。";
      console.info("OH-Hatoba diagnostic failed");
    } finally {
      button.disabled = false;
    }
  };
  button.addEventListener("click", () => void run());
  void run();
}).catch(() => {
  button.textContent = "未连接原生宿主";
  report.textContent = "未收到鸿蒙消息端口。请在 HAP 中运行此验证台；普通浏览器仅可检查终端显示与输入。";
});
