// SPDX-License-Identifier: Apache-2.0

export interface GitHubContributor {
  login: string;
  avatar_url: string;
  html_url: string;
  url?: string;
  name?: string | null;
  bio?: string | null;
  type?: string;
}

type ContributorFetch = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>;

const CONTRIBUTORS_URL =
  "https://api.github.com/repos/Hybrid-Mount/meta-hybrid_mount/contributors";

let contributorsRequest: Promise<GitHubContributor[]> | null = null;

export function selectHumanContributors(
  contributors: GitHubContributor[],
  limit = 20,
): GitHubContributor[] {
  return contributors
    .filter((contributor) => {
      const isBotType = contributor.type?.toLowerCase() === "bot";
      const hasBotName = contributor.login.toLowerCase().includes("bot");
      return !isBotType && !hasBotName;
    })
    .slice(0, limit);
}

async function enrichContributor(
  contributor: GitHubContributor,
  fetcher: ContributorFetch,
): Promise<GitHubContributor> {
  const profileUrl =
    contributor.url ??
    `https://api.github.com/users/${encodeURIComponent(contributor.login)}`;

  try {
    const response = await fetcher(profileUrl);
    if (!response.ok) return contributor;

    const profile = (await response.json()) as Partial<GitHubContributor>;
    return {
      ...contributor,
      name: profile.name ?? contributor.name,
      bio: profile.bio ?? contributor.bio,
    };
  } catch {
    return contributor;
  }
}

export async function fetchGitHubContributors(
  fetcher: ContributorFetch = fetch,
  limit = 20,
): Promise<GitHubContributor[]> {
  const response = await fetcher(CONTRIBUTORS_URL);
  if (!response.ok) throw new Error(String(response.status));

  const contributors = selectHumanContributors(
    (await response.json()) as GitHubContributor[],
    limit,
  );

  return Promise.all(
    contributors.map((contributor) => enrichContributor(contributor, fetcher)),
  );
}

export function loadGitHubContributors(): Promise<GitHubContributor[]> {
  if (!contributorsRequest) {
    contributorsRequest = fetchGitHubContributors().catch((error: unknown) => {
      contributorsRequest = null;
      throw error;
    });
  }

  return contributorsRequest;
}
