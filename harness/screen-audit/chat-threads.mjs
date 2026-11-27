export const chatThreads = (
  settled,
  initial = [["main-chat", "기존 시장 대화"]],
) => {
  const histories = new Map();
  const entry = (turn, position, role, text) => ({
    sequence: position,
    at_unix_seconds: "1",
    turn_id: turn,
    role,
    text,
    status: role === "Assistant" ? "Completed" : null,
    cards: [],
    sources: [],
  });
  for (const [id, title] of initial) {
    histories.set(
      id,
      title === null
        ? []
        : [
            entry("fixture-previous", 1, "User", title),
            entry(
              "fixture-previous",
              2,
              "Assistant",
              "저장된 예시 답변입니다.",
            ),
          ],
    );
  }
  const item = (id) => ({
    session_id: id,
    title: histories.get(id)?.[0]?.text ?? null,
  });
  return {
    listChatSessions: () => [...histories.keys()].reverse().map(item),
    createChatSession: ({ request }) => {
      if (!histories.has(request.session_id))
        histories.set(request.session_id, []);
      return item(request.session_id);
    },
    getChatHistory: ({ request }) => ({
      session_id: request.session_id,
      entries: [...(histories.get(request.session_id) ?? [])],
      active_turn: null,
    }),
    sendChatMessage: async ({ request, onChunk }) => {
      const rows = histories.get(request.session_id) ?? [];
      histories.delete(request.session_id);
      histories.set(request.session_id, rows);
      rows.push(
        entry(request.turn_id, rows.length + 1, "User", request.message),
      );
      await settled(request.session_id);
      const text = "예시 응답 · " + request.message;
      rows.push(entry(request.turn_id, rows.length + 1, "Assistant", text));
      onChunk.onmessage({
        turn_id: request.turn_id,
        sequence: 1,
        chunk: { TextDelta: { text } },
      });
      return {
        session_id: request.session_id,
        turn_id: request.turn_id,
        run_id: request.run_id,
        status: "Completed",
        mode: "FreshAnalysis",
        actual_model: null,
        first_token_latency_ms: null,
        resets_at_unix_seconds: null,
        failure_reason: null,
      };
    },
  };
};
