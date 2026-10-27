import test from "node:test";
import assert from "node:assert/strict";
import { checkLatestVersion, compareVersions } from "./update-check.ts";

const repo = "https://github.com/eeliyaa/Squeeze";
const response = (status, body) => ({ status, ok: status >= 200 && status < 300, json: async () => body });

test("compares stable and prerelease versions", () => {
  assert.equal(compareVersions("v1.2.0", "1.1.9"), 1);
  assert.equal(compareVersions("1.2.0-rc.1", "1.2.0"), -1);
  assert.equal(compareVersions("1.2.0", "1.2.0"), 0);
});

test("returns the release when GitHub has a newer version", async () => {
  let call = 0;
  const result = await checkLatestVersion("0.1.0", repo, async (url) => {
    call++;
    if (call === 1) {
      assert.equal(url, "https://api.github.com/repos/eeliyaa/Squeeze");
      return response(200, {});
    }
    assert.equal(url, "https://api.github.com/repos/eeliyaa/Squeeze/releases/latest");
    return response(200, { tag_name: "v0.2.0", html_url: "https://github.com/eeliyaa/Squeeze/releases/tag/v0.2.0" });
  });
  assert.deepEqual(result, { kind: "update", latestVersion: "v0.2.0", releaseUrl: "https://github.com/eeliyaa/Squeeze/releases/tag/v0.2.0" });
});

test("reports current only after a successful latest-release response", async () => {
  let call = 0;
  const result = await checkLatestVersion("0.2.0", repo, async () => ++call === 1 ? response(200, {}) : response(200, {
    tag_name: "v0.2.0", html_url: "https://github.com/eeliyaa/Squeeze/releases/tag/v0.2.0",
  }));
  assert.deepEqual(result, { kind: "current", latestVersion: "v0.2.0" });
});

test("distinguishes a repository with no published releases", async () => {
  let call = 0;
  assert.deepEqual(await checkLatestVersion("0.1.0", repo, async () => ++call === 1 ? response(200, {}) : response(404, {})), { kind: "no-releases" });
});

test("reports offline/API failures instead of claiming the app is current", async () => {
  const offline = await checkLatestVersion("0.1.0", repo, async () => { throw new TypeError("fetch failed"); });
  let call = 0;
  const apiError = await checkLatestVersion("0.1.0", repo, async () => ++call === 1 ? response(200, {}) : response(503, {}));
  assert.equal(offline.kind, "error");
  assert.equal(apiError.kind, "error");
});
