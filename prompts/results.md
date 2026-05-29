Both builds clean. Everything is in place. Here's a summary of what was
implemented:

---

MCP Server changes:

- src/tools/index.ts — exported TOOLS array (was const, now export const)
- src/agent/claude_agent.ts — new module; runs the full Claude agent loop,
  executes tool calls through existing handlers, streams text/tool-feedback via
  onChunk, returns AgentTurn[]
- src/index.ts — added POST /chat endpoint that sets SSE headers, streams
  chunks as data: {...}\n\n events, emits a done event with turn count and tool
  call names

Viewer changes:

- src/agent/claude_client.ts — new SSE client that posts to /chat and parses
  data: lines from the response stream
- src/main.ts — wired the Ask button and Enter key to sendAgentMessage; text
  streams into the assistant div in real time; [N turns, tools: ...] meta line
  appended on completion
- index.html — added .msg-meta CSS style

Integration test:

- tests/integration/mock_neo4j.ts — in-memory Neo4j stub returning fixed data
  for LINE-1001
- tests/integration/tool_chain.test.ts — skips without ANTHROPIC_API_KEY; spies
  on all tool handlers to capture call order; asserts search_object_by_tag
  precedes graph tools and get_tile_feature_mapping precedes viewer tools
