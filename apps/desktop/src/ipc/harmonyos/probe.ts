import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import "./probe.css";
import { connectHost } from "./bridge";

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
      report.textContent = JSON.stringify(await bridge.invoke("host_probe"), null, 2);
      console.info("OH-Hatoba P0 probe rendered");
    } catch {
      report.textContent = "原生验证失败，请查看构建与设备日志。";
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
