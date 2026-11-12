import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { LocalPrefs } from "@/ipc/types";

const host = vi.hoisted(() => ({ harmony: false }));
vi.mock("@/ipc/api", async (importActual) => ({
  ...(await importActual<typeof import("@/ipc/api")>()),
  isHarmonyOS: () => host.harmony,
}));
vi.stubGlobal("matchMedia", () => ({ matches: false, addEventListener() {}, removeEventListener() {} }));

const { DEFAULT_PREFS, useApp } = await import("@/app/store");
const { useVaultData } = await import("@/app/data");
const { t } = await import("@/i18n");
const { ProxiesPane } = await import("./ProxiesPane");

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let unmount: (() => void) | null = null;
const saved = vi.fn(async (prefs: LocalPrefs) => useApp.setState({ prefs }));
beforeEach(() => {
  host.harmony = false;
  saved.mockClear();
  useApp.setState({ prefs: { ...DEFAULT_PREFS }, setPrefs: saved });
  useVaultData.setState({
    proxies: [
      {
        id: "generated-proxy",
        name: "Generated proxy",
        kind: "socks5",
        address: "127.0.0.1",
        port: 1080,
        username: "",
        has_password: false,
        host_ids: [],
        updated_at: 0,
      },
    ],
  });
});
afterEach(() => {
  act(() => unmount?.());
  unmount = null;
  document.body.innerHTML = "";
});

async function render() {
  const container = document.createElement("div");
  document.body.appendChild(container);
  const root = createRoot(container);
  await act(async () => root.render(<ProxiesPane />));
  unmount = () => root.unmount();
  const button = container.querySelector<HTMLButtonElement>(`[aria-label="${t("settings.proxy.app")}"]`)!;
  return { container, button };
}

describe("proxy defaults by host", () => {
  it("keeps the system proxy option on desktop", async () => {
    const { container, button } = await render();
    expect(button.textContent).toContain(t("settings.proxy.system"));
    expect(container.textContent).toContain(t("settings.proxy.app.hint"));
  });

  it("shows direct connections on HarmonyOS and still allows choosing a saved proxy", async () => {
    host.harmony = true;
    const { container, button } = await render();
    expect(button.textContent).toContain(t("settings.proxy.none"));
    expect(container.textContent).toContain(t("settings.proxy.app.manualHint"));
    await act(async () => button.click());
    const options = [...document.querySelectorAll<HTMLButtonElement>('[role="menu"] button')];
    expect(options.some((option) => option.textContent?.includes(t("settings.proxy.system")))).toBe(false);
    const proxy = options.find((option) => option.textContent?.includes("Generated proxy"))!;
    expect(proxy).toBeDefined();
    await act(async () => proxy.click());
    expect(saved).toHaveBeenLastCalledWith(
      expect.objectContaining({
        app_proxy_id: "generated-proxy",
        default_proxy_id: null,
      }),
    );
    expect(button.textContent).toContain("Generated proxy");
  });
});
