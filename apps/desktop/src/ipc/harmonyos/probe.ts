import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import "./probe.css";
import { connectHost } from "./bridge";
import { createHarmonyApi } from "./api";
import { FRAME_DATA } from "../types";
import { probeAiTransport } from "./probe-ai";

const report = document.querySelector<HTMLPreElement>("#report")!;
const button = document.querySelector<HTMLButtonElement>("#probe")!;
const terminal = new Terminal({ cursorBlink: true, fontFamily: "monospace", fontSize: 15, theme: { background: "#0b1314", foreground: "#d9e8e5" } });
const fit = new FitAddon();
terminal.loadAddon(fit);
const container = document.querySelector<HTMLDivElement>("#terminal")!;
terminal.open(container);
terminal.writeln("Hatoba — xterm 6 / ArkWeb 输入验证\r\n中文 · 全角字符 · emoji 🌊\r\n");
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
      const telnet = await api.telnet_host_save({ id: null, name: "Generated Telnet diagnostic", address: "127.0.0.1",
        port: 2323, username: "fixture", password: null, group_id: null, tags: ["diagnostic"], favorite: false,
        proxy_mode: "direct", proxy_id: null, note: "", ai_notes: "" });
      assert((await api.telnet_hosts_list()).some((host) => host.id === telnet.id && host.port === 2323));
      await api.telnet_host_set_favorite(telnet.id, true);
      assert((await api.telnet_hosts_list()).some((host) => host.id === telnet.id && host.favorite));
      const duplicate = await api.telnet_host_duplicate(telnet.id);
      assert(duplicate.id !== telnet.id && duplicate.address === telnet.address);
      // Only this diagnostic entry's isolated vault is touched; these strings are never executed.
      await api.command_history_clear();
      await api.command_history_add(saved.id, "echo generated-ssh-history");
      await api.command_history_add(telnet.id, "echo generated-telnet-history");
      await api.command_history_add(telnet.id, " excluded-generated-history");
      assert((await api.command_history_list(saved.id))[0] === "echo generated-ssh-history");
      const history = await api.command_history_list(telnet.id);
      assert(history.length === 1 && history[0] === "echo generated-telnet-history");
      await api.command_history_remove(telnet.id, "echo generated-telnet-history");
      assert((await api.command_history_list(telnet.id)).length === 0);
      await api.command_history_clear();
      assert((await api.command_history_list(saved.id)).length === 0);
      await api.telnet_host_delete(telnet.id);
      await api.telnet_host_delete(duplicate.id);
      assert(!(await api.telnet_hosts_list()).some((host) => host.id === telnet.id || host.id === duplicate.id));
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
      const business = { typedFacade: true, vaultCreateOrReopen: true, hostCrudAndAgentPreservation: true, telnetHostCrud: true, commandHistoryIsolationAndClear: true, lockEvent: true, lockedReadDenied: true, unlock: true };
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
        // Eight separate calls stay below the JSON limit while crossing the native channel's
        // 512 KiB credit window. Without consumption ACKs the final blocks cannot arrive.
        const blockBytes = 96 * 1024;
        const blocks = 8;
        const sshUnit = new TextEncoder().encode("generated 中文 🌊\r\n");
        const sshBlock = new Uint8Array(blockBytes);
        for (let i = 0; i < sshBlock.length; i++) sshBlock[i] = sshUnit[i % sshUnit.length];
        const expected = new Uint8Array(blockBytes * blocks);
        for (let i = 0; i < blocks; i++) expected.set(sshBlock, i * blockBytes);
        const bytes = new Uint8Array(expected.length);
        let receivedBytes = 0;
        let overflow = false;
        let parsed = 0;
        let finish: (() => void) | undefined;
        const received = new Promise<void>((resolve) => { finish = resolve; });
        let sid: string | undefined;
        try {
          sid = await api.ssh_connect_target({ address: "127.0.0.1", port: 22222, username: "fixture" },
            { cols: 80, rows: 24, password: null, passphrase: null }, (frame) => {
              if (frame[0] !== FRAME_DATA) return;
              const data = frame.subarray(1);
              if (receivedBytes + data.length > bytes.length) overflow = true;
              else bytes.set(data, receivedBytes);
              receivedBytes += data.length;
              return new Promise<void>((resolve) => terminal.write(data, () => {
                parsed += frame.length - 1;
                resolve(); if (parsed >= expected.length) finish?.();
              }));
            });
          for (let i = 0; i < blocks; i++) await api.ssh_write_bytes(sid, Array.from(sshBlock));
          let timer: ReturnType<typeof setTimeout> | undefined;
          try {
            await Promise.race([received, new Promise<void>((_, reject) => { timer = setTimeout(() => reject(new Error("Terminal fixture timed out")), 10000); })]);
          } finally { clearTimeout(timer); }
          assert(!overflow && receivedBytes === expected.length && parsed === expected.length);
          assert(bytes.every((byte, i) => byte === expected[i]));
          assert((await api.sftp_list(sid, "/")).some((file) => file.name === "fixture.bin"));
          await api.ssh_resize(sid, 132, 40);
          console.info("OH-Hatoba Web SSH transport passed");
        } finally {
          if (sid) await api.ssh_disconnect(sid);
          unkey(); unauth();
        }
        // The loopback Telnet fixture negotiates ECHO/SGA/BINARY before its marker. Waiting
        // for the marker avoids sending while the client still uses local echo or line editing.
        const marker = new TextEncoder().encode("OH-Hatoba Telnet fixture ready\r\n");
        const unit = new Uint8Array([...new TextEncoder().encode("generated Telnet 中文 🌊\r\n"), 255]);
        const telnetBlock = new Uint8Array(blockBytes);
        for (let i = 0; i < telnetBlock.length; i++) telnetBlock[i] = unit[i % unit.length];
        const telnetExpected = new Uint8Array(blockBytes * blocks);
        for (let i = 0; i < blocks; i++) telnetExpected.set(telnetBlock, i * blockBytes);
        const telnetBytes = new Uint8Array(marker.length + telnetExpected.length);
        let telnetReceived = 0;
        let telnetOverflow = false;
        let telnetParsed = 0;
        let markReady: (() => void) | undefined;
        let markEchoed: (() => void) | undefined;
        const ready = new Promise<void>((resolve) => { markReady = resolve; });
        const echoed = new Promise<void>((resolve) => { markEchoed = resolve; });
        const waitForTelnet = async (pending: Promise<void>) => {
          let timer: ReturnType<typeof setTimeout> | undefined;
          try {
            await Promise.race([pending, new Promise<void>((_, reject) => {
              timer = setTimeout(() => reject(new Error("Telnet fixture timed out")), 10000);
            })]);
          } finally { clearTimeout(timer); }
        };
        let telnetSid: string | undefined;
        try {
          telnetSid = await api.telnet_open_target("127.0.0.1", 22323, { cols: 80, rows: 24 }, (frame) => {
            if (frame[0] !== FRAME_DATA) return;
            // Capture raw bytes before xterm decodes them, including Telnet's escaped IAC byte.
            const data = frame.subarray(1);
            if (telnetReceived + data.length > telnetBytes.length) telnetOverflow = true;
            else telnetBytes.set(data, telnetReceived);
            telnetReceived += data.length;
            return new Promise<void>((resolve) => terminal.write(data, () => {
              telnetParsed += frame.length - 1;
              resolve();
              if (telnetParsed >= marker.length) markReady?.();
              if (telnetParsed >= marker.length + telnetExpected.length) markEchoed?.();
            }));
          });
          await waitForTelnet(ready);
          assert(telnetReceived === marker.length && marker.every((byte, i) => byte === telnetBytes[i]));
          for (let i = 0; i < blocks; i++) await api.telnet_write_bytes(telnetSid, Array.from(telnetBlock));
          await waitForTelnet(echoed);
          assert(!telnetOverflow && telnetReceived === telnetBytes.length && telnetParsed === telnetBytes.length);
          assert(telnetExpected.every((byte, i) => byte === telnetBytes[marker.length + i]));
          await api.telnet_resize(telnetSid, 132, 40);
          await api.telnet_close(telnetSid);
          telnetSid = undefined;
          console.info("OH-Hatoba Web Telnet transport passed");
        } finally {
          if (telnetSid) await api.telnet_close(telnetSid);
        }
        console.info("OH-Hatoba Web flow-control budget recovered");
        await probeAiTransport(api);
        console.info("OH-Hatoba Web AI assistant lifecycle passed");
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
