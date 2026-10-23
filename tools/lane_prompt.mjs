// Render the existing canonical workflow's prompt/schema without executing a workflow or calling a model.
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {dirname, resolve} from 'node:path';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const request = JSON.parse(readFileSync(0, 'utf8'));
let source = readFileSync(resolve(root, '.claude/workflows/fc4-lane.js'), 'utf8');
const marker = '// A restart starts a fresh agent';
if (!source.includes(marker)) throw new Error('Workflow prompt boundary changed; update renderer tests');
source = source.split(marker)[0].replace('export const meta', 'const meta');
const rendered = new Function('args', source + '\nreturn {prompt, schema: RESULT};')(request.args);
if (request.args.kind === 're') {
  rendered.prompt += '\n\nREA admission: only the retained, admitted exact-target client may perform research. The transport exposes procedure_pseudo_code and batch_decompile only. Never open/close/reset targets, add servers or fall back to another REA process. A tool list or keeper process is not target readiness. If the transport reports target failure, preserve its actual cause and stop blocked; do not retry a worker round. Claude RE workflow startup is unsupported and must fail admission.\n';
}
if (request.provider === 'codex') {
  rendered.prompt = rendered.prompt.replace('- REA: you have only mcp__rea__procedure_pseudo_code and mcp__rea__batch_decompile, for pseudocode the export and decomp/ lack. All lanes share ONE REA Ghidra session: expect ~3 minutes if you are the first to query; on an error fall back to re_query.py and decomp/ and say so in your report (docs/re-workspace.md "Querying in parallel lanes").',
    '- REA uses the owned retained client admitted by the host. Only procedure_pseudo_code{procedure} and batch_decompile{addresses} are exposed. No document/provider/path override. Preserve live failures and UNKNOWN truthfully. Canonical export/re_query/instruction research is allowed independently of live pseudocode success, within the supplied evidence scope.');
  // Provider-specific transport instructions only; the behaviour brief and clean-room rules are shared.
  rendered.prompt = rendered.prompt
    .replace('- Shell: the Bash tool is Git Bash. Write Windows paths with forward slashes (C:/Users/...) or in single quotes: an unquoted backslash is eaten. PowerShell: `powershell -NoProfile -File <script>.ps1`.',
      '- Shell: exec_command uses PowerShell on Windows. Use forward-slash paths. For Bash syntax, run the quiet q.py wrapper. Use apply_patch for edits; cap shell output and avoid long inline scripts.')
    .replace('- Your cwd is the main checkout: never edit, commit, merge or `cd` there.',
      '- Your cwd is your lane worktree. Never edit, commit, merge or `cd` in the main checkout.')
    .replace('Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>',
      'Co-Authored-By: Codex <noreply@openai.com>')
    .replace('ctx.py <lane> --budget <budget>', 'codexguard.py status --manifest <MANIFEST>')
    .replaceAll('--fresh', '');
  rendered.prompt += '\n\n## Codex transport\nThe coordinator has created and reserved this worktree. Reuse it. Use only local shell/edit tools and the explicitly configured REA tools. Do not spawn agents, workflows or model CLI processes. Waiting: a command that runs longer than a few seconds (qbuild/fc4verify, cargo, ctest, regress, qwait, lanes.py wait, a game request) gets yield_time_ms of at least 60000 on exec_command; never call write_stdin, wait or sleep only to see whether a command has finished (22% of lane calls on 2026-10-21/22 were such checks). Files: create and change every file with apply_patch, never through the shell (no here-strings, Set-Content, Out-File, WriteAllText or echo redirects); a multi-line script is a file in <WT>/build-runs/ created with apply_patch and run by path. The shared lane guard enforces every supported tool call; when denied, write only the permitted handoff and commit it, then return the required structured result. No tools or MCP servers may be added.\n';
  rendered.prompt += `- <MANIFEST> = ${request.manifest}\n`;
  rendered.prompt += '- Host admission: the dispatcher and supervisor completed the full host guard, dependency, key, role and source checks before this isolated worker. Consume the registered immutable manifest host_admission provenance; do not rerun lanes.py check, spawn-admission, REA installation discovery or coordinator preflight from detached/scoped refs. Missing global npm files or Git refs here do not negate the host assertion. No reinstall, permission expansion or replacement server. Normal per-call STOP, role, runtime, budget and call guards still apply. Admission and binary metadata do not prove live procedure success.\n';
  rendered.prompt += '- This worktree intentionally has a detached HEAD. Commit normally; do not switch branches, update refs or run Git maintenance. The supervisor publishes your commits to the reserved lane branch after this round. Use the actual HEAD for the result and queue tip. Shared Git refs/config/hooks are read-only.\n';
  const worktree = `${request.args.main || 'C:/Users/tlori/FreeCity4'}/.claude/worktrees/${request.args.name}`;
  rendered.prompt += `- Before any other tool, run exactly this single shell command to initialize measured context accounting: git -C "${worktree}" rev-parse --show-toplevel\n`;
  // Codex strict JSON schemas require every property in required, and no extra properties.
  rendered.schema.required = Object.keys(rendered.schema.properties);
  rendered.schema.additionalProperties = false;
}
process.stdout.write(JSON.stringify(rendered));
