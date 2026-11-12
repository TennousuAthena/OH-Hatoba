import { act, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.stubGlobal("matchMedia", () => ({ matches: false, addEventListener() {}, removeEventListener() {} }));

const { useApp } = await import("./store");
const { WindowChrome, WindowControls } = await import("./TitleBar");

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let unmount: (() => void) | null = null;
afterEach(() => {
  act(() => unmount?.());
  unmount = null;
  document.body.innerHTML = "";
});

async function render(children: ReactNode) {
  const host = document.createElement("div");
  document.body.appendChild(host);
  const root = createRoot(host);
  await act(async () => root.render(children));
  unmount = () => root.unmount();
  return host;
}

describe("window controls by host", () => {
  it.each(["windows", "linux"] as const)("keeps the three desktop caption controls on %s", async (platform) => {
    useApp.setState((state) => ({ info: { ...state.info, platform } }));
    const host = await render(<WindowControls />);
    expect(host.querySelectorAll("button")).toHaveLength(3);
  });

  it("shows no desktop caption buttons on HarmonyOS tablets or 2in1 devices", async () => {
    useApp.setState((state) => ({ info: { ...state.info, platform: "harmonyos" } }));
    const host = await render(<WindowControls />);
    expect(host.childNodes).toHaveLength(0);
  });

  it("leaves no desktop title strip over HarmonyOS onboarding and unlock screens", async () => {
    useApp.setState((state) => ({ info: { ...state.info, platform: "harmonyos" } }));
    const host = await render(<WindowChrome />);
    expect(host.childNodes).toHaveLength(0);
  });
});
