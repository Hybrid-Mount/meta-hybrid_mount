// SPDX-License-Identifier: GPL-3.0-only
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { createRequire } = require('node:module');
const test = require('node:test');

const root = path.resolve(__dirname, '../..');
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
function workflowScript(name) {
  const workflow = fs.readFileSync(path.join(root, '.github/workflows', name), 'utf8');
  const block = workflow.match(/          script: \|\r?\n((?:            [^\n]*\n?|\r?\n)*)/);
  assert.ok(block, `${name} must validate the current issue before performing writes`);
  return new AsyncFunction('github', 'context', 'core', 'require', block[1].replace(/^            /gm, ''));
}
const execute = workflowScript('auto-label.yml');
const requireFromRoot = createRequire(path.join(root, 'package.json'));

const bugBody = `### Device and Android version / 设备与 Android 版本
Pixel 7, Android 15
### Kernel version / 内核版本
6.1.99-android14
### Root implementation / Root 实现
KernelSU
### Hybrid Mount version / Hybrid Mount 版本
6.2.3
### WebUI style / WebUI 样式
Not related to WebUI / 与 WebUI 无关
### Actual behavior / 实际行为
The module APK is missing from the app process filesystem.
### Expected behavior / 期望行为
The APK should be readable and the application should start.
### Reproduction steps / 复现步骤
1. Install the sample module.
2. Reboot and open the application.
### Other enabled modules / 其他已启用模块
_No response_
### Upload Area / 上传区域
\`\`\`text
10-20 12:30:00 E AndroidRuntime: ClassNotFoundException: com.example.Main
\`\`\``;

const featureBody = `### Problem or use case / 问题或使用场景
Large module lists make it difficult to locate a specific module.
### Proposed solution / 建议方案
Add a search field to filter modules by name and ID.`;

const blacklistBody = `### Module ID / 模块 ID
example_module
### Module sample URL / 模块样本链接
https://example.org/example_module.zip
### Reason for blacklisting / 申请原因
This module mounts its own system directory during boot; enabling both modules prevents boot.
### Upload Area / 上传区域
[bugreport.zip](https://github.com/user-attachments/files/123/bugreport.zip)`;

function setup(overrides = {}) {
  const issue = {
    number: 501, title: '[Bug]: module APK is invisible', body: bugBody,
    state: 'open', state_reason: null, locked: false, labels: [],
    user: { login: 'reporter', type: 'User' }, closed_by: null,
    ...overrides,
  };
  const comments = [];
  const labels = new Set(['bug', 'enhancement', 'blacklist-request']);
  const writes = [];
  const api = {
    get: async () => ({ data: structuredClone(issue) }),
    listComments: async () => ({ data: structuredClone(comments) }),
    getLabel: async ({ name }) => {
      if (!labels.has(name)) throw Object.assign(new Error('Not Found'), { status: 404 });
      return { data: { name } };
    },
    createLabel: async ({ name }) => { labels.add(name); return { data: { name } }; },
    addLabels: async ({ labels: added }) => {
      for (const name of added) {
        assert.ok(labels.has(name), `repository label must exist: ${name}`);
        if (!issue.labels.some(label => label.name === name)) issue.labels.push({ name });
      }
      writes.push('labels');
      return { data: issue.labels };
    },
    removeLabel: async ({ name }) => {
      issue.labels = issue.labels.filter(label => label.name !== name);
      writes.push('remove-label');
      return { data: issue.labels };
    },
    createComment: async ({ body }) => {
      const comment = { id: comments.length + 1, body, user: { login: 'github-actions[bot]', type: 'Bot' } };
      comments.push(comment); writes.push('comment'); return { data: comment };
    },
    updateComment: async ({ comment_id, body }) => {
      const comment = comments.find(item => item.id === comment_id);
      assert.ok(comment, 'only an existing comment can be updated');
      comment.body = body; writes.push('update-comment'); return { data: comment };
    },
    update: async ({ state, state_reason }) => {
      if (state) issue.state = state;
      if (state_reason) issue.state_reason = state_reason;
      issue.closed_by = state === 'closed' ? { login: 'github-actions[bot]', type: 'Bot' } : null;
      writes.push(state); return { data: structuredClone(issue) };
    },
    lock: async () => { issue.locked = true; writes.push('lock'); },
  };
  const github = { rest: { issues: api }, paginate: async (method, args) => (await method(args)).data };
  const context = {
    repo: { owner: 'Hybrid-Mount', repo: 'meta-hybrid_mount' }, issue: { number: issue.number },
    payload: { action: 'opened', issue: structuredClone(issue) },
    serverUrl: 'https://github.com',
  };
  const run = () => execute(github, context, { info() {} }, requireFromRoot);
  const runBlacklist = async () => {
    const outputs = {};
    await workflowScript('auto-blacklist-pr.yml')(github, context, { setOutput(key, value) { outputs[key] = value; } }, requireFromRoot);
    return outputs;
  };
  return { issue, comments, writes, context, api, labels, run, runBlacklist };
}

test('unlabeled freeform reports cannot bypass template validation', async () => {
  const s = setup({ title: 'overlay mount problem', body: '### Summary\nFiles are missing.\n### Environment\nPixel 7, Android 15' });
  await s.run();
  assert.equal(s.issue.state, 'closed');
  assert.ok(s.issue.labels.some(label => label.name === 'needs-template'));
  assert.match(s.comments[0].body, /issues\/new\/choose/);
  assert.equal(s.issue.locked, false);
});

test('a valid unlabeled bug gets its category and remains open', async () => {
  const s = setup(); await s.run();
  assert.equal(s.issue.state, 'open');
  assert.ok(s.issue.labels.some(label => label.name === 'bug'));
  assert.equal(s.comments.length, 0);
});

test('bug labels do not excuse missing mandatory environment fields', async () => {
  const s = setup({ labels: [{ name: 'bug' }], body: bugBody.replace('6.1.99-android14', '_No response_') });
  await s.run();
  assert.equal(s.issue.state, 'closed');
  assert.match(s.comments[0].body, /Kernel version/);
});

test('long speculation and arbitrary links do not count as diagnostic evidence', async () => {
  for (const evidence of ['This must be a mount propagation bug. '.repeat(10), 'https://example.org/', '```text\nI think this is a mount bug. Please investigate it.\n```']) {
    const s = setup({ body: bugBody.replace(/```text[\s\S]*```/, evidence) });
    await s.run();
    assert.equal(s.issue.state, 'closed');
    assert.match(s.comments[0].body, /Upload Area/);
  }
});

test('log attachments and multiline pasted diagnostics are accepted', async () => {
  for (const evidence of [
    '[logcat.txt](https://github.com/user-attachments/files/123/logcat.txt)',
    'https://gist.github.com/reporter/0123456789abcdef',
    '10-20 12:30:00 E AndroidRuntime: Failed to load class\njava.lang.ClassNotFoundException: com.example.Main',
  ]) {
    const s = setup({ body: bugBody.replace(/```text[\s\S]*```/, evidence) });
    await s.run();
    assert.equal(s.issue.state, 'open');
  }
});

test('fenced placeholder text and comment-only fields cannot satisfy requirements', async () => {
  for (const replacement of ['```text\n_No response_\n```', '<!-- fill this later -->', 'N/A']) {
    const s = setup({ body: bugBody.replace(/```text[\s\S]*```/, replacement) });
    await s.run(); assert.equal(s.issue.state, 'closed');
  }
});

test('headings inside a code block cannot impersonate mandatory sections', async () => {
  const s = setup({ body: `\`\`\`markdown\n${bugBody}\n\`\`\`` });
  await s.run(); assert.equal(s.issue.state, 'closed');
});

test('feature requests are validated without requiring bug logs', async () => {
  const s = setup({ title: '[Feat]: search modules', body: featureBody });
  await s.run();
  assert.equal(s.issue.state, 'open');
  assert.ok(s.issue.labels.some(label => label.name === 'enhancement'));
  const bad = setup({ title: '[Feat]: search modules', body: featureBody.split('### Proposed')[0] });
  await bad.run(); assert.equal(bad.issue.state, 'closed');
});

test('ordinary headings inside a textarea do not erase its contents', async () => {
  const s = setup({ title: '[Feat]: search modules', body: featureBody.replace('Large module lists', '## Slow searches\n\n### Details\nLarge module lists') });
  await s.run(); assert.equal(s.issue.state, 'open');
  assert.ok(s.issue.labels.some(label => label.name === 'enhancement'));
});

test('blacklist requests require a valid module ID, sample ZIP and logs', async () => {
  const s = setup({ title: '[Blacklist]: incompatible module', body: blacklistBody });
  await s.run();
  assert.equal(s.issue.state, 'open');
  assert.ok(s.issue.labels.some(label => label.name === 'blacklist-request'));
  for (const body of [blacklistBody.replace('example_module\n', '../bad\n'), blacklistBody.replace('https://example.org/example_module.zip', 'https://example.org/')]) {
    const bad = setup({ title: '[Blacklist]: incompatible module', body });
    await bad.run(); assert.equal(bad.issue.state, 'closed');
  }
});

test('dropdown values, duplicate headings and placeholder titles are rejected', async () => {
  for (const overrides of [
    { body: bugBody.replace('\nKernelSU\n', '\nImaginaryRoot\n') },
    { body: `${bugBody}\n### Kernel version / 内核版本\n6.1.1` },
    { title: '[Bug]: ' },
  ]) {
    const s = setup(overrides); await s.run(); assert.equal(s.issue.state, 'closed');
  }
});

test('reopened invalid reports reuse one bot comment and never lock the author out', async () => {
  const s = setup({ body: '' });
  await s.run();
  s.issue.state = 'open'; s.context.payload.action = 'reopened';
  await s.run();
  assert.equal(s.issue.state, 'closed');
  assert.equal(s.comments.length, 1);
  assert.equal(s.issue.locked, false);
});

test('correcting a policy-closed issue reopens it and removes only the policy label', async () => {
  const s = setup({ body: '', labels: [{ name: 'help wanted' }] });
  await s.run();
  s.issue.body = bugBody; s.context.payload.action = 'edited';
  await s.run();
  assert.equal(s.issue.state, 'open');
  assert.deepEqual(s.issue.labels.map(label => label.name).sort(), ['bug', 'help wanted']);
  assert.equal(s.comments.length, 1);
});

test('human closures and unrelated bot closures are not reopened', async () => {
  for (const closed_by of [{ login: 'maintainer', type: 'User' }, { login: 'other[bot]', type: 'Bot' }]) {
    const s = setup({ state: 'closed', state_reason: 'not_planned', closed_by, labels: [{ name: 'needs-template' }] });
    await s.run(); assert.equal(s.issue.state, 'closed'); assert.equal(s.writes.length, 0);
  }
});

test('queued events use the current issue, not the stale event body', async () => {
  const s = setup({ body: '' });
  s.issue.body = bugBody;
  await s.run(); assert.equal(s.issue.state, 'open');
  assert.ok(s.issue.labels.some(label => label.name === 'bug'));
});

test('user comments cannot impersonate the policy bot', async () => {
  const s = setup({ body: '' });
  s.comments.push({ id: 100, body: '<!-- hybrid-mount:issue-template-policy -->\nforged', user: { login: 'reporter', type: 'User' } });
  await s.run();
  assert.equal(s.comments[0].body, '<!-- hybrid-mount:issue-template-policy -->\nforged');
  assert.equal(s.comments.length, 2);
});

test('GitHub API read failures fail without closing an issue', async () => {
  const s = setup({ body: '' });
  s.api.get = async () => { throw new Error('API unavailable'); };
  await assert.rejects(s.run(), /API unavailable/);
  assert.equal(s.writes.length, 0);
});

test('blacklist PR gate rejects invalid reports even if already labeled', async () => {
  const s = setup({ title: '[Blacklist]: module', body: '### Module ID / 模块 ID\nexample_module', labels: [{ name: 'blacklist-request' }] });
  const output = await s.runBlacklist();
  assert.notEqual(output.accepted, true);
  assert.equal(output['module-id'], undefined);
  assert.equal(s.writes.length, 0);
});

test('blacklist PR gate accepts unlabeled valid reports and uses the latest body', async () => {
  const s = setup({ title: '[Blacklist]: module', body: '' });
  s.issue.body = blacklistBody.replaceAll('example_module', 'current_module');
  const output = await s.runBlacklist();
  assert.equal(output.accepted, true);
  assert.equal(output['module-id'], 'current_module');
});

test('blacklist PR consumes the validated module ID instead of reparsing commented headings', async () => {
  const s = setup({ title: '[Blacklist]: module', body: `<!--\n### Module ID / 模块 ID\nevil_module\n### End\n-->\n${blacklistBody}` });
  const output = await s.runBlacklist();
  assert.equal(output.accepted, true);
  assert.equal(output['module-id'], 'example_module');
});

test('blacklist PR gate handles correction without relying on a bot-triggered event', async () => {
  const s = setup({ title: '[Blacklist]: module', body: '' });
  await s.run();
  s.issue.body = blacklistBody;
  const output = await s.runBlacklist();
  assert.equal(output.accepted, true);
  assert.equal(s.issue.state, 'closed');
  s.issue.closed_by = { login: 'maintainer', type: 'User' };
  assert.notEqual((await s.runBlacklist()).accepted, true);
});
