export type UpdateCheckResult =
  | { kind: "update"; latestVersion: string; releaseUrl: string }
  | { kind: "current"; latestVersion: string }
  | { kind: "no-releases" }
  | { kind: "error"; message: string };

type Fetcher = (input: string | URL | Request, init?: RequestInit) => Promise<Response>;

function parseVersion(version: string): { numbers: number[]; prerelease: string[] | null } | null {
  const match = /^v?(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/.exec(version.trim());
  if (!match) return null;
  return { numbers: match.slice(1, 4).map(Number), prerelease: match[4]?.split(".") ?? null };
}

export function compareVersions(left: string, right: string): number | null {
  const a = parseVersion(left);
  const b = parseVersion(right);
  if (!a || !b) return null;
  for (let i = 0; i < 3; i++) {
    if (a.numbers[i] !== b.numbers[i]) return a.numbers[i] > b.numbers[i] ? 1 : -1;
  }
  if (a.prerelease === null && b.prerelease !== null) return 1;
  if (a.prerelease !== null && b.prerelease === null) return -1;
  if (a.prerelease === null || b.prerelease === null) return 0;
  for (let i = 0; i < Math.max(a.prerelease.length, b.prerelease.length); i++) {
    const ai = a.prerelease[i];
    const bi = b.prerelease[i];
    if (ai === undefined) return -1;
    if (bi === undefined) return 1;
    if (ai === bi) continue;
    const an = /^\d+$/.test(ai);
    const bn = /^\d+$/.test(bi);
    if (an && bn) return Number(ai) > Number(bi) ? 1 : -1;
    if (an !== bn) return an ? -1 : 1;
    return ai > bi ? 1 : -1;
  }
  return 0;
}

export async function checkLatestVersion(
  installedVersion: string,
  repositoryUrl: string,
  fetcher: Fetcher = fetch,
): Promise<UpdateCheckResult> {
  const installed = parseVersion(installedVersion);
  if (!installed) return { kind: "error", message: "The installed version could not be read." };

  let repository: URL;
  try {
    repository = new URL(repositoryUrl);
  } catch {
    return { kind: "error", message: "The repository URL is not configured correctly." };
  }
  const segments = repository.pathname.split("/").filter(Boolean);
  if (repository.protocol !== "https:" || repository.hostname !== "github.com" || segments.length !== 2) {
    return { kind: "error", message: "The repository URL is not configured correctly." };
  }

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 12_000);
  try {
    const [owner, repo] = segments;
    const options: RequestInit = {
      headers: { Accept: "application/vnd.github+json" },
      signal: controller.signal,
    };
    const repositoryResponse = await fetcher(`https://api.github.com/repos/${owner}/${repo}`, options);
    if (repositoryResponse.status === 404) return { kind: "error", message: "The GitHub repository was not found or is not public." };
    if (!repositoryResponse.ok) return { kind: "error", message: `GitHub could not check the repository (HTTP ${repositoryResponse.status}).` };
    const response = await fetcher(`https://api.github.com/repos/${owner}/${repo}/releases/latest`, options);
    if (response.status === 404) return { kind: "no-releases" };
    if (!response.ok) {
      return { kind: "error", message: `GitHub could not check releases (HTTP ${response.status}).` };
    }
    const release: unknown = await response.json();
    if (!release || typeof release !== "object") {
      return { kind: "error", message: "GitHub returned an invalid release response." };
    }
    const data = release as { tag_name?: unknown; html_url?: unknown };
    if (typeof data.tag_name !== "string" || typeof data.html_url !== "string") {
      return { kind: "error", message: "GitHub returned an invalid release response." };
    }
    const releaseUrl = new URL(data.html_url);
    if (releaseUrl.origin !== repository.origin || !releaseUrl.pathname.startsWith(`${repository.pathname}/releases/`)) {
      return { kind: "error", message: "GitHub returned an unexpected release link." };
    }
    const comparison = compareVersions(data.tag_name, installedVersion);
    if (comparison === null) return { kind: "error", message: "The release version uses an unsupported format." };
    return comparison > 0
      ? { kind: "update", latestVersion: data.tag_name, releaseUrl: releaseUrl.href }
      : { kind: "current", latestVersion: data.tag_name };
  } catch {
    return { kind: "error", message: "Could not reach GitHub. Check your internet connection and try again." };
  } finally {
    clearTimeout(timeout);
  }
}
