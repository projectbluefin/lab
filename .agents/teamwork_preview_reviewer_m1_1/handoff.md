# Handoff Report — Milestone 1 Review

## 1. Observation

- **GitHub API Rate Limit Failures**:
  Running `curl` directly to test the API endpoint in `src/pages/applications.astro` returns a rate limit error message with exit code `0`:
  ```json
  {"message":"API rate limit exceeded for 172.10.227.189. (But here's the good news: Authenticated requests get a higher rate limit. Check out the documentation for more details.)","documentation_url":"https://docs.github.com/rest/overview/resources-in-the-rest-api#rate-limiting"}
  ```
- **Test suite failure**:
  Running `node --test tests/applications-page.test.mjs` resulted in:
  ```
  ✖ applications page renders Bazaar evidence, chart mounts, and explicit unavailable states (1369.095967ms)
    AssertionError [ERR_ASSERTION]: applications page auto-discovers Bazzite Shell test suite
        at TestContext.<anonymous> (file:///var/home/jorge/src/testing-lab/tests/applications-page.test.mjs:49:10)
      expected: /Bazzite Shell/i,
      operator: 'match',
      diff: 'simple'
  ```
- **Un-deleted files**:
  `find_by_name` search shows `docs/prototype-factory.html` is still present:
  ```
  Found 1 results
  docs/prototype-factory.html
  ```
- **Unused selector**:
  In `src/styles/site.css`, line 783:
  ```css
  783:   .status-grid,
  ```
- **Build / Prerender Failures under test environment**:
  Running the test suite sequentially or in parallel fails with:
  ```
  17:42:36 [ERROR] Error [ERR_MODULE_NOT_FOUND]: Cannot find module '/var/home/jorge/src/testing-lab/docs/.prerender/chunks/homebrew_Cp6Tq89L.mjs' imported from /var/home/jorge/src/testing-lab/docs/.prerender/prerender-entry.CrMSVjtS.mjs
  ```

## 2. Logic Chain

- **Silent rate limit failure**:
  1. The template in `src/pages/applications.astro` calls `execSync` with `curl -s ...` to query the GitHub API.
  2. Because the `-f` (fail) flag is not used, `curl` exits with code `0` even when receiving a `403 Forbidden` rate limit JSON response.
  3. `execSync` does not throw an error, bypassing the `catch (e)` fallback block.
  4. The template parses the rate limit JSON which lacks a `tree` array, resulting in `discoveredFeatures` being evaluated to `[]`.
  5. The generated HTML lacks any auto-discovered test suite cards (including `Bazzite Shell`).
  6. The test assertion `assert.match(applicationsPage, /Bazzite Shell/i)` fails.
- **Un-deleted prototype**:
  1. The user request required verifying the deletion of `docs/prototype-factory.html`.
  2. The file still exists in the filesystem and was not removed from git.
- **Unused CSS selector**:
  1. The user request required verifying the removal of `.status-grid` rules.
  2. A reference to `.status-grid` remains at line 783 in `src/styles/site.css`.

## 3. Caveats

- **External Network Dependency**: The GitHub API rate limit behavior is dependent on the external IP address of the homelab node and whether it is rate-limited by GitHub. In environments without rate limits, this bug may stay silent.
- **Node.js Test Runner Concurrency**: Astro/Vite prerender chunk collision errors occur due to multiple rapid sequential builds or concurrent file I/O within Node's test context.

## 4. Conclusion

- **Verdict**: **REQUEST_CHANGES**
- **Actionable Steps**:
  1. Fix the GitHub API call in `src/pages/applications.astro` by adding the `-f` flag to `curl` (`curl -fs`) to ensure a non-zero exit status code on rate limit or other HTTP errors.
  2. Delete `docs/prototype-factory.html`.
  3. Remove the unused selector `.status-grid` reference from `src/styles/site.css` line 783.
  4. Ideally, refactor tests to run the build command once globally instead of per-test block to prevent chunk collision errors.

## 5. Verification Method

- **Command to verify**: `npm test`
- **File to inspect**: `src/pages/applications.astro`, `docs/prototype-factory.html`, `src/styles/site.css`
- **Invalidation Condition**: The tests will continue to fail under rate limiting if `curl -s` is not changed to `curl -fs`.
