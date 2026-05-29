import Anthropic from "@anthropic-ai/sdk";
import { readFileSync } from "fs";
import { join } from "path";
import { fileURLToPath } from "url";
import type { ToolContext } from "../tools/index.js";
import { TOOLS } from "../tools/index.js";

const __dirname = fileURLToPath(new URL(".", import.meta.url));
const SYSTEM_PROMPT_PATH = join(__dirname, "../../../docs/mcp/agent_system_prompt.md");

function loadSystemPrompt(): string {
  try {
    return readFileSync(SYSTEM_PROMPT_PATH, "utf-8");
  } catch {
    return "You are TileGraphAgent. Always use tools to retrieve engineering data. Never infer facts.";
  }
}

function mcpToolsToAnthropicTools(): Anthropic.Tool[] {
  return TOOLS.map((t) => ({
    name: t.definition.name,
    description: t.definition.description,
    input_schema: t.definition.inputSchema as Anthropic.Tool["input_schema"],
  }));
}

export interface AgentTurn {
  role: "user" | "assistant";
  content: string;
  tool_calls?: { name: string; input: unknown; result: unknown }[];
}

export async function runAgentLoop(
  userMessage: string,
  ctx: ToolContext,
  onChunk: (chunk: string) => void,
  maxToolRounds = 8,
): Promise<AgentTurn[]> {
  const client = new Anthropic();
  const systemPrompt = loadSystemPrompt();
  const anthropicTools = mcpToolsToAnthropicTools();

  const turns: AgentTurn[] = [];
  const messages: Anthropic.MessageParam[] = [{ role: "user", content: userMessage }];

  for (let round = 0; round < maxToolRounds; round++) {
    const response = await client.messages.create({
      model: "claude-sonnet-4-6",
      max_tokens: 4096,
      system: systemPrompt,
      tools: anthropicTools,
      messages,
    });

    let assistantText = "";
    const toolUses: Anthropic.ToolUseBlock[] = [];

    for (const block of response.content) {
      if (block.type === "text") {
        assistantText += block.text;
        onChunk(block.text);
      } else if (block.type === "tool_use") {
        toolUses.push(block);
      }
    }

    turns.push({
      role: "assistant",
      content: assistantText,
      tool_calls: [],
    });

    if (toolUses.length === 0 || response.stop_reason === "end_turn") {
      break;
    }

    const toolResults: Anthropic.ToolResultBlockParam[] = [];

    for (const toolUse of toolUses) {
      const tool = TOOLS.find((t) => t.definition.name === toolUse.name);
      let result: unknown;

      if (!tool) {
        result = { error_code: "UNKNOWN_TOOL", message: `Tool '${toolUse.name}' not found` };
      } else {
        const t0 = Date.now();
        try {
          result = await tool.handler(toolUse.input as Record<string, unknown>, ctx);
          await ctx.auditLogger.log({
            tool_name: toolUse.name,
            input: toolUse.input,
            output_summary: JSON.stringify(result).slice(0, 200),
            duration_ms: Date.now() - t0,
          });
        } catch (err) {
          result = {
            error_code: "TOOL_ERROR",
            message: err instanceof Error ? err.message : String(err),
          };
          await ctx.auditLogger.log({
            tool_name: toolUse.name,
            input: toolUse.input,
            output_summary: "TOOL_ERROR",
            duration_ms: Date.now() - t0,
            error: (result as { error_code: string }).error_code,
          });
        }
      }

      turns[turns.length - 1].tool_calls!.push({
        name: toolUse.name,
        input: toolUse.input,
        result,
      });

      toolResults.push({
        type: "tool_result",
        tool_use_id: toolUse.id,
        content: JSON.stringify(result),
      });

      onChunk(`\n[Tool: ${toolUse.name}]\n`);
    }

    messages.push({ role: "assistant", content: response.content });
    messages.push({ role: "user", content: toolResults });
  }

  return turns;
}
