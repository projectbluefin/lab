## 2026-07-01T21:44:47Z
You are the Worker agent. Your working directory is /var/home/jorge/src/testing-lab/.agents/teamwork_preview_worker_m1_2.
Your task is to fix the issues discovered during the review of the Milestone 1 implementation.

Please read the revision feedback here:
/var/home/jorge/src/testing-lab/.agents/sub_orch_m1/revision_feedback.md

Please implement the following changes:
1. Update `src/layouts/SiteLayout.astro` to dynamically render the site brand title (`Operating System Factory`) as an `<h1>` only on the homepage (`/` or `/index.html`), and as a `<span>` on all other pages. This ensures the homepage has exactly one `<h1>` tag, and all other pages also have exactly one `<h1>` (their page heading).
2. Fix the GitHub API call in `src/pages/applications.astro`:
   - Change `curl -s` to `curl -fs` to ensure that it fails with a non-zero exit status code on HTTP error (e.g. rate limit).
   - In the page frontmatter script, check if `treeData.tree` exists and is an array. If not, explicitly throw an error (e.g., `throw new Error("Rate limit or invalid response")`) to trigger the `catch` block fallback.
3. Clean the build cache:
   - In `package.json`, update the `"build"` script to delete `.astro` and `node_modules/.vite` in its cleanup sequence before running `astro build`.
4. Delete `docs/prototype-factory.html`.
5. Remove the unused selector `.status-grid` at line 783 in `src/styles/site.css`.

After making these changes, run `npm run build` and `npm test` and `npm run check` to verify that all 13 tests pass green and there are no compilation errors or warnings.

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A Forensic Auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Document the commands run, tests executed, and write your handoff report to `/var/home/jorge/src/testing-lab/.agents/teamwork_preview_worker_m1_2/handoff.md`. Report back when complete.
