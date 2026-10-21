// SPDX-License-Identifier: GPL-3.0-only
// Keep these field names and dropdown values aligned with ISSUE_TEMPLATE/*.yml.
const upload = 'Upload Area / 上传区域';
const templates = [
  {
    prefix: '[Bug]:', label: 'bug', color: 'd73a4a',
    required: [
      'Device and Android version / 设备与 Android 版本',
      'Kernel version / 内核版本',
      'Root implementation / Root 实现',
      'Hybrid Mount version / Hybrid Mount 版本',
      'WebUI style / WebUI 样式',
      'Actual behavior / 实际行为',
      'Expected behavior / 期望行为',
      'Reproduction steps / 复现步骤', upload,
    ],
    choices: {
      'Root implementation / Root 实现': ['KernelSU', 'KernelSU Next', 'APatch', 'Other / 其他'],
      'WebUI style / WebUI 样式': ['Not related to WebUI / 与 WebUI 无关', 'Miuix', 'Material Design 3'],
    },
  },
  {
    prefix: '[Feat]:', label: 'enhancement', color: 'a2eeef',
    required: ['Problem or use case / 问题或使用场景', 'Proposed solution / 建议方案'],
  },
  {
    prefix: '[Blacklist]:', label: 'blacklist-request', color: 'd93f0b',
    required: ['Module ID / 模块 ID', 'Module sample URL / 模块样本链接', 'Reason for blacklisting / 申请原因', upload],
  },
];

const marker = '<!-- hybrid-mount:issue-template-policy -->';
const policyLabel = 'needs-template';
const botLogin = 'github-actions[bot]';
const sectionNames = new Set([
  ...templates.flatMap(template => template.required),
  'Other enabled modules / 其他已启用模块',
  'Alternatives considered / 考虑过的替代方案',
  'Additional context / 补充信息',
]);

function clean(value) {
  return value.replace(/<!--[\s\S]*?-->/g, '').trim();
}

function hasContent(value) {
  const text = clean(value).replace(/[`*_~]/g, '').trim();
  return text.length > 0 && !/^(?:no response|n\/?a|none|null|unknown|tbd|todo|待补充|待填写|未提供|未知|无|暂无|[?.…\s-]+)$/i.test(text);
}

// Headings inside fenced logs are data, never form fields. Ignore HTML comments too.
function parseSections(body) {
  const sections = new Map();
  let current;
  let fence;
  for (const line of clean(body).replace(/\r\n?/g, '\n').split('\n')) {
    const delimiter = line.match(/^ {0,3}(`{3,}|~{3,})(.*)$/);
    if (fence) {
      if (current) current.push(line);
      if (delimiter && delimiter[1][0] === fence[0] && delimiter[1].length >= fence.length && !delimiter[2].trim()) fence = undefined;
      continue;
    }
    if (delimiter) {
      fence = delimiter[1];
      if (current) current.push(line);
      continue;
    }
    const heading = line.match(/^ {0,3}(#{1,3})\s+(.+?)\s*#*\s*$/);
    if (heading && heading[1] === '###' && sectionNames.has(heading[2].trim())) {
      const name = heading[2].trim();
      current = [];
      if (!sections.has(name)) sections.set(name, []);
      sections.get(name).push(current);
    } else if (current) {
      current.push(line);
    }
  }
  return sections;
}

function urls(value) {
  return [...value.matchAll(/https?:\/\/[^\s<>()[\]`]+/gi)].flatMap(match => {
    try { return [new URL(match[0])]; } catch { return []; }
  });
}

function hasEvidence(value) {
  const text = clean(value);
  const attachment = /\.(?:txt|log|old|logcat|dmesg|dump|crash|trace|bugreport|tar|gz|tgz|bz2|xz|lz4|zst|zip|7z|rar)$/i;
  if (urls(text).some(url => attachment.test(url.pathname)
    || /^(?:gist\.github\.com|gist\.githubusercontent\.com|pastebin\.com|paste\.rs)$/i.test(url.hostname) && url.pathname.split('/').filter(Boolean).length > 0)) return true;
  // Fences alone do not turn prose into diagnostics. Recognize logcat, kernel,
  // stack traces, mount tables and Hybrid Mount state without asserting authenticity.
  const blocks = text.matchAll(/^ {0,3}(`{3,}|~{3,})[^\n]*\n([\s\S]*?)^ {0,3}\1\s*$/gm);
  for (const block of blocks) if (hasDiagnosticOutput(block[2])) return true;
  const lines = text.split('\n').filter(line => hasContent(line));
  return lines.length >= 2 && hasDiagnosticOutput(text);
}

function hasDiagnosticOutput(value) {
  const text = clean(value);
  return hasContent(text) && /^(?:\s*(?:\d{2}-\d{2}\s+)?\d{2}:\d{2}:\d{2}|\s*\[\s*\d+\.\d+\]|\s*[EFWDI]\/|\s*(?:\[[^\]\n]+\]\s*)?(?:ERROR|WARN|INFO|DEBUG|TRACE|FATAL)\b|.*\b[\w.$]*(?:Exception|Error):|.*\bavc:\s*denied|\s*\d+\s+\d+\s+\d+:\d+\s+.* - (?:overlay|tmpfs|ext4|erofs|f2fs)\s|\S+\s+\/(?:system|vendor|product|system_ext|odm)\b.*\s(?:overlay|tmpfs|ext4|erofs)\s|\s*"(?:mount_stats|overlayfs_mounts|vfs_error|failed_stage)"\s*:|\s*(?:ls|cat|stat|mount|readlink):.*(?:No such file|Permission denied|Operation not permitted))/m.test(text);
}

function validateIssue(issue) {
  const title = clean(issue.title || '');
  const sections = parseSections(issue.body || '');
  const template = templates.find(item => title.toLowerCase().startsWith(item.prefix.toLowerCase()))
    || templates.find(item => sections.has(item.required[0]));
  if (!template) return { errors: ['Use one of the repository issue templates / 请使用仓库提供的 Issue 模板。'] };
  const errors = [];
  const fields = {};
  if (!title.toLowerCase().startsWith(template.prefix.toLowerCase()) || !hasContent(title.slice(template.prefix.length))) {
    errors.push(`Title must start with ${template.prefix} followed by a summary / 标题须包含此前缀及问题摘要。`);
  }
  for (const field of template.required) {
    const entries = sections.get(field) || [];
    const value = entries.length === 1 ? clean(entries[0].join('\n')) : '';
    fields[field] = value;
    if (entries.length > 1) errors.push(`${field}: duplicate section / 章节重复。`);
    else if (!hasContent(value)) errors.push(`${field}: missing or placeholder-only / 缺失、为空或仅有占位内容。`);
    else if (template.choices?.[field] && !template.choices[field].includes(value)) errors.push(`${field}: select an option from the form / 请选择表单中的选项。`);
    else if (field === upload && !hasEvidence(value)) errors.push(`${field}: provide a log/archive link or pasted diagnostic output / 请提供日志或报告附件链接，或粘贴日志代码块；普通描述和任意网页链接不算日志。`);
    else if (field === 'Module ID / 模块 ID' && !/^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(value)) errors.push(`${field}: invalid module ID / 模块 ID 格式无效。`);
    else if (field === 'Module sample URL / 模块样本链接' && !urls(value).some(url => /\.zip$/i.test(url.pathname))) errors.push(`${field}: provide a direct ZIP URL / 请提供模块 ZIP 直链。`);
  }
  return { template, errors, fields };
}

async function ensureLabel(github, repo, name, color) {
  try {
    await github.rest.issues.getLabel({ ...repo, name });
  } catch (error) {
    if (error.status !== 404) throw error;
    try {
      await github.rest.issues.createLabel({ ...repo, name, color });
    } catch (createError) {
      // Two different issues may be the first to need this repository label.
      if (createError.status !== 422) throw createError;
      await github.rest.issues.getLabel({ ...repo, name });
    }
  }
}

async function inspectIssue({ github, context }) {
  const args = { ...context.repo, issue_number: context.issue.number };
  // Read current state after entering the per-issue workflow queue. Event bodies
  // may predate an edit or a maintainer closing the issue.
  const { data: issue } = await github.rest.issues.get(args);
  if (issue.pull_request || issue.locked) return;
  const labels = new Set(issue.labels.map(label => typeof label === 'string' ? label : label.name));
  const possiblyPolicyClosed = issue.state === 'closed' && issue.state_reason === 'not_planned'
    && issue.closed_by?.login === botLogin && labels.has(policyLabel);
  if (issue.state !== 'open' && !possiblyPolicyClosed) return;

  const result = validateIssue(issue);
  let comment;
  if (result.errors.length || labels.has(policyLabel)) {
    const comments = await github.paginate(github.rest.issues.listComments, { ...args, per_page: 100 });
    comment = comments.find(item => item.user?.login === botLogin && item.user.type === 'Bot' && item.body?.startsWith(marker));
  }
  // Never reopen a human closure or infer ownership from a label alone.
  if (issue.state === 'closed' && !comment) return;
  return { args, issue, labels, result, comment, possiblyPolicyClosed };
}

async function acceptedBlacklistIssue({ github, context }) {
  const state = await inspectIssue({ github, context });
  if (state && !state.result.errors.length && state.result.template.label === 'blacklist-request') {
    return { ...state.issue, validatedModuleId: state.result.fields['Module ID / 模块 ID'] };
  }
}

async function enforceIssuePolicy({ github, context, core }) {
  const state = await inspectIssue({ github, context });
  if (!state) return;
  const { args, issue, labels, result, comment, possiblyPolicyClosed } = state;
  const chooser = `${context.serverUrl || 'https://github.com'}/${context.repo.owner}/${context.repo.repo}/issues/new/choose`;

  async function setComment(body) {
    if (!comment) {
      await github.rest.issues.createComment({ ...args, body });
    } else if (comment.body !== body) {
      await github.rest.issues.updateComment({ ...context.repo, comment_id: comment.id, body });
    }
  }
  async function addLabel(name, color) {
    if (labels.has(name)) return;
    await ensureLabel(github, context.repo, name, color);
    await github.rest.issues.addLabels({ ...args, labels: [name] });
  }

  if (result.errors.length) {
    await addLabel(policyLabel, 'fbca04');
    await setComment(`${marker}\nThis issue was closed because required template information is missing or invalid.\n此 Issue 因模板必填信息缺失或无效而自动关闭。\n\n${result.errors.map(error => `- ${error}`).join('\n')}\n\n[Choose a template / 选择模板](${chooser}). Edit this issue's title/body to match the template; once validation passes, this bot will reopen issues it closed. Do not create a duplicate.\n请按对应模板编辑当前 Issue 的标题和正文，补全后由本机器人关闭的 Issue 会自动重新打开，无需重复提交。\n\nThis check verifies structure and evidence format only; it does not establish the cause or authenticity of a report.\n此检查仅验证结构与证据格式，不代表已确认问题根因或报告真实性。`);
    if (issue.state === 'open') await github.rest.issues.update({ ...args, state: 'closed', state_reason: 'not_planned' });
    core.info(`Issue #${issue.number}: ${result.errors.length} template validation error(s)`);
    return;
  }

  await addLabel(result.template.label, result.template.color);
  if (possiblyPolicyClosed) await github.rest.issues.update({ ...args, state: 'open', state_reason: 'reopened' });
  if (labels.has(policyLabel)) {
    if (comment) await setComment(`${marker}\nRequired template information is now complete. The template check passed; technical claims still require investigation.\n模板必填信息已补全，检查通过；技术结论仍需进一步核实。`);
    await github.rest.issues.removeLabel({ ...args, name: policyLabel });
  }
}

module.exports = { validateIssue, enforceIssuePolicy, acceptedBlacklistIssue };
