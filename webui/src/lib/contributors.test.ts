// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it, vi } from "vitest";
import {
  fetchGitHubContributors,
  selectHumanContributors,
  type GitHubContributor,
} from "./contributors";

function contributor(login: string, type = "User"): GitHubContributor {
  return {
    login,
    type,
    avatar_url: `https://example.com/${login}.png`,
    html_url: `https://github.com/${login}`,
  };
}

describe("selectHumanContributors", () => {
  it("excludes GitHub bot accounts by type and login", () => {
    const result = selectHumanContributors([
      contributor("human"),
      contributor("release-helper", "Bot"),
      contributor("dependabot[bot]"),
    ]);

    expect(result.map(({ login }) => login)).toEqual(["human"]);
  });
});

describe("fetchGitHubContributors", () => {
  it("enriches human contributors with profile names and bios", async () => {
    const human = {
      ...contributor("human"),
      url: "https://api.github.com/users/human",
    };
    const fetcher = vi
      .fn()
      .mockResolvedValueOnce(new Response(JSON.stringify([human])))
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ name: "Human Name", bio: "Kernel developer" })),
      );

    const result = await fetchGitHubContributors(fetcher);

    expect(result[0]).toMatchObject({
      login: "human",
      name: "Human Name",
      bio: "Kernel developer",
    });
  });

  it("keeps the contributor when a profile request fails", async () => {
    const fetcher = vi
      .fn()
      .mockResolvedValueOnce(new Response(JSON.stringify([contributor("human")])))
      .mockRejectedValueOnce(new Error("network error"));

    await expect(fetchGitHubContributors(fetcher)).resolves.toEqual([
      contributor("human"),
    ]);
  });
});
