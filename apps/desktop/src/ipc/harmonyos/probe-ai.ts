import type { HatobaApi } from "../api";
import type { AiProviderInput, AiTurnContext, AiTurnEvent, AiTurnEndReason } from "../types";

const RESPONSE = "generated 中文 🌊";
const CANCEL_RESPONSE = "generated cancel ready";

function check(condition: boolean): void {
  if (!condition) throw new Error("AI diagnostic check failed");
}

async function within(pending: Promise<void>, milliseconds = 10000): Promise<void> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    await Promise.race([
      pending,
      new Promise<void>((_, reject) => {
        timer = setTimeout(() => reject(new Error("AI diagnostic timed out")), milliseconds);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

function captureTurn(conversations: Set<string>) {
  let firstText: (() => void) | undefined;
  let end: (() => void) | undefined;
  const textReceived = new Promise<void>((resolve) => { firstText = resolve; });
  const finished = new Promise<void>((resolve) => { end = resolve; });
  const state = {
    text: "",
    storedText: "",
    requestStarted: false,
    failed: false,
    completed: false,
    ended: null as AiTurnEndReason | null,
    endings: 0,
    lateEvents: false,
  };
  const receive = (event: AiTurnEvent): void => {
    if (state.ended !== null) state.lateEvents = true;
    switch (event.kind) {
      case "opened": conversations.add(event.conversation_id); break;
      case "request_started": state.requestStarted = true; break;
      case "text": state.text += event.delta; firstText?.(); break;
      case "entry":
        if (event.entry.role === "assistant") state.storedText = event.entry.text;
        break;
      case "done": state.completed = event.finish === "stop"; break;
      case "error":
      case "tool_call": state.failed = true; break;
      case "turn_ended":
        state.ended = event.reason;
        state.endings++;
        end?.();
        break;
    }
  };
  return { state, receive, textReceived, finished };
}

/** Called only by the network diagnostic entry, whose native host opens an isolated vault. */
export async function probeAiTransport(api: HatobaApi): Promise<void> {
  const input: AiProviderInput = {
    id: null,
    name: "Generated Web AI diagnostic",
    protocol: "chat_completions",
    base_url: "http://127.0.0.1:28080/v1",
    api_key: "",
    auth_header: "authorization",
    models: ["generated-fixture", "generated-fixture-cancel"].map((id) => ({
      id, name: id, context_window: 32768, max_output_tokens: 1024,
      efforts: [], adaptive_thinking: false,
    })),
  };
  const models = await api.ai_provider_models(input);
  check(models.some((model) => model.id === "generated-fixture"));
  check(models.some((model) => model.id === "generated-fixture-cancel"));
  const provider = await api.ai_provider_save(input);
  const conversations = new Set<string>();
  try {
    check(!provider.has_api_key);
    const context: AiTurnContext = {
      provider_id: provider.id,
      model_id: "generated-fixture",
      effort: null,
      host_id: null,
      target: null,
      local: null,
      tab: false,
      telnet: false,
      session_id: null,
      disabled_mcp_servers: (await api.mcp_servers_list()).map((server) => server.id),
      permission_mode: "manual",
      screen: null,
    };
    const turn = captureTurn(conversations);
    const sent = await api.ai_send({
      conversation_id: null,
      text: "Generated diagnostic request; return the fixed fixture response.",
      context,
    }, turn.receive);
    conversations.add(sent.conversation.id);
    await within(turn.finished);
    check(turn.state.requestStarted && turn.state.completed && !turn.state.failed);
    check(turn.state.endings === 1 && turn.state.ended === "completed");
    check(turn.state.text === RESPONSE && turn.state.storedText === RESPONSE);
    const stored = await api.ai_conversation_get(sent.conversation.id);
    check(!stored.running && stored.entries.some((entry) => entry.role === "assistant" && entry.text === RESPONSE));

    // This fixture model emits one chunk and leaves its HTTP stream open. Stopping after
    // that chunk exercises cancellation in the shared assistant, not an already finished turn.
    const cancelling = captureTurn(conversations);
    const waiting = await api.ai_send({
      conversation_id: null,
      text: "Generated diagnostic cancellation request.",
      context: { ...context, model_id: "generated-fixture-cancel" },
    }, cancelling.receive);
    conversations.add(waiting.conversation.id);
    await within(cancelling.textReceived);
    check(cancelling.state.text === CANCEL_RESPONSE && cancelling.state.ended === null);
    check((await api.ai_conversation_get(waiting.conversation.id)).running);
    await within(api.ai_stop(waiting.conversation.id), 5000);
    await within(cancelling.finished, 5000);
    check(cancelling.state.requestStarted && !cancelling.state.failed && !cancelling.state.completed);
    check(cancelling.state.endings === 1 && cancelling.state.ended === "stopped");
    check(cancelling.state.storedText === CANCEL_RESPONSE);
    const stopped = await api.ai_conversation_get(waiting.conversation.id);
    check(!stopped.running && stopped.entries.some((entry) => entry.role === "assistant" && entry.text === CANCEL_RESPONSE));
    await new Promise<void>((resolve) => setTimeout(resolve, 250));
    check(!turn.state.lateEvents && !cancelling.state.lateEvents);
  } finally {
    // Delete only records created by this invocation. No conversation or model content is logged.
    const cleanup = await Promise.allSettled([...conversations].map(async (id) => {
      await api.ai_stop(id);
      await api.ai_conversation_delete(id);
    }));
    await api.ai_provider_delete(provider.id);
    check(cleanup.every((result) => result.status === "fulfilled"));
    check(!(await api.ai_conversations_list()).some((conversation) => conversations.has(conversation.id)));
    check(!(await api.ai_providers_list()).some((saved) => saved.id === provider.id));
  }
}
