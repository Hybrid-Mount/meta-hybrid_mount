// SPDX-License-Identifier: GPL-3.0-only

async function findArtifact({ github, context, sourceHash }) {
  if (!/^[a-f0-9]{64}$/.test(sourceHash)) throw new Error('Invalid notifier source hash');
  const name = `notify-linux-x86_64-${sourceHash}`;
  const repositoryId = context.payload.repository.id;
  const artifacts = await github.paginate(github.rest.actions.listArtifactsForRepo, {
    ...context.repo, name, per_page: 100,
  });
  for (const artifact of artifacts) {
    if (artifact.name !== name || artifact.expired !== false
      || artifact.workflow_run?.head_repository_id !== repositoryId) continue;
    const { data: run } = await github.rest.actions.getWorkflowRun({
      ...context.repo, run_id: artifact.workflow_run.id,
    });
    if (run.head_repository?.id !== repositoryId
      || !['push', 'workflow_dispatch'].includes(run.event)
      || !['.github/workflows/build.yml', '.github/workflows/release.yml', '.github/workflows/notify-tool.yml'].includes(run.path)
      || !/^(?:dev|main|v\d+\.\d+\.\d+(?:-(?:alpha|beta|rc)\.\d+)?)$/.test(run.head_branch)) continue;
    return { id: artifact.id, runId: artifact.workflow_run.id };
  }
}

module.exports = { findArtifact };
