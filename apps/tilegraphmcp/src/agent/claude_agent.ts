import type { ToolContext } from "../tools/index.js";
import { TOOLS } from "../tools/index.js";

// ── DeepSeek / OpenAI-compatible types ──────────────────────────────────────

interface OAIFunction {
  name: string;
  description: string;
  parameters: Record<string, unknown>;
}

interface OAITool {
  type: "function";
  function: OAIFunction;
}

interface OAIToolCall {
  id: string;
  type: "function";
  function: { name: string; arguments: string };
}

interface OAIMessage {
  role: "system" | "user" | "assistant" | "tool";
  content: string | null;
  tool_calls?: OAIToolCall[];
  tool_call_id?: string;   // required when role === "tool"
  name?: string;
}

interface OAIRequest {
  model: string;
  messages: OAIMessage[];
  tools?: OAITool[];
  tool_choice?: "auto" | "none";
}

interface OAIChoice {
  finish_reason: string;
  message: {
    role: "assistant";
    content: string | null;
    tool_calls?: OAIToolCall[];
  };
}

interface OAIResponse {
  choices: OAIChoice[];
}

// ─────────────────────────────────────────────────────────────────────────────

const DEEPSEEK_URL = "https://api.deepseek.com/chat/completions";

// DeepSeek V3 on DeepSeek — cheapest option with solid tool-calling support
export const DEFAULT_MODEL = "deepseek-v4-flash";

async function chatCompletion(
  apiKey: string,
  request: OAIRequest,
): Promise<OAIResponse> {
  const resp = await fetch(DEEPSEEK_URL, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${apiKey}`,
      "HTTP-Referer": "https://tilegraphmcp.workers.dev",
      "X-Title": "TileGraphAgent",
    },
    body: JSON.stringify(request),
  });

  if (!resp.ok) {
    const body = await resp.text();
    throw new Error(`DeepSeek API error ${resp.status}: ${body}`);
  }

  return resp.json() as Promise<OAIResponse>;
}

function buildTools(): OAITool[] {
  return TOOLS.map((t) => ({
    type: "function",
    function: {
      name: t.definition.name,
      description: t.definition.description,
      parameters: t.definition.inputSchema as Record<string, unknown>,
    },
  }));
}

export interface AgentTurn {
  role: "user" | "assistant";
  content: string;
  tool_calls?: { name: string; input: unknown; result: unknown }[];
}

const DEFAULT_SYSTEM_PROMPT =
  "You are TileGraphAgent. Always use tools to retrieve engineering data. Never infer facts.";

export async function runAgentLoop(
  userMessage: string,
  ctx: ToolContext,
  onChunk: (chunk: string) => void,
  systemPrompt = DEFAULT_SYSTEM_PROMPT,
  apiKey?: string,
  model = DEFAULT_MODEL,
  maxToolRounds = 8,
): Promise<AgentTurn[]> {
  const key =
    apiKey ??
    (typeof process !== "undefined" ? process.env.DEEPSEEK_API_KEY : undefined) ??
    "";
  if (!key) throw new Error("DEEPSEEK_API_KEY is not set");

  const oaiTools = buildTools();
  const turns: AgentTurn[] = [];
  const messages: OAIMessage[] = [
    { role: "system", content: systemPrompt },
    { role: "user", content: userMessage },
  ];

  for (let round = 0; round < maxToolRounds; round++) {
    const response = await chatCompletion(key, {
      model,
      messages,
      tools: oaiTools,
      tool_choice: "auto",
    });

    const choice = response.choices[0];
    if (!choice) throw new Error("DeepSeek returned no choices");

    const assistantMsg = choice.message;
    const assistantText = assistantMsg.content ?? "";
    const toolCalls = assistantMsg.tool_calls ?? [];

    if (assistantText) onChunk(assistantText);

    turns.push({ role: "assistant", content: assistantText, tool_calls: [] });

    // Append the raw assistant message so the model sees its own tool_calls
    messages.push({
      role: "assistant",
      content: assistantMsg.content,
      tool_calls: toolCalls.length ? toolCalls : undefined,
    });

    if (toolCalls.length === 0 || choice.finish_reason === "stop") break;

    for (const tc of toolCalls) {
      const toolName = tc.function.name;
      const toolInput = JSON.parse(tc.function.arguments || "{}") as Record<string, unknown>;
      const tool = TOOLS.find((t) => t.definition.name === toolName);
      let result: unknown;
      const t0 = Date.now();

      if (!tool) {
        result = { error_code: "UNKNOWN_TOOL", message: `Tool '${toolName}' not found` };
      } else {
        try {
          result = await tool.handler(toolInput, ctx);
          await ctx.auditLogger.log({
            tool_name: toolName,
            input: toolInput,
            output_summary: JSON.stringify(result).slice(0, 200),
            duration_ms: Date.now() - t0,
          });
        } catch (err) {
          result = {
            error_code: "TOOL_ERROR",
            message: err instanceof Error ? err.message : String(err),
          };
          await ctx.auditLogger.log({
            tool_name: toolName,
            input: toolInput,
            output_summary: "TOOL_ERROR",
            duration_ms: Date.now() - t0,
            error: (result as { error_code: string }).error_code,
          });
        }
      }

      turns[turns.length - 1].tool_calls!.push({ name: toolName, input: toolInput, result });
      onChunk(`\n[Tool: ${toolName}]\n`);

      messages.push({
        role: "tool",
        tool_call_id: tc.id,
        name: toolName,
        content: JSON.stringify(result),
      });
    }
  }

  return turns;
}
