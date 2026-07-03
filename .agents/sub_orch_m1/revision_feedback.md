# Revision Feedback for Milestone 1

We received `REQUEST_CHANGES` verdicts from our Reviewers and heading outline violations from our Challengers. Please address the following issues in the next iteration:

## 1. Homepage Heading Outline Violation
- **Issue**: The homepage (`index.html`) completely lacks any `<h1>` header tag.
- **Fix**: Update `src/layouts/SiteLayout.astro` to dynamically render the site brand title (`Operating System Factory`) as an `<h1>` when on the homepage (`/` or `/index.html`), and as a `<span>` on all other pages. This ensures every page has exactly one `<h1>` tag (either the site brand title on the homepage, or the page-specific heading on subpages).

## 2. GitHub API Rate Limit Vulnerability
- **Issue**: In `src/pages/applications.astro`, the GitHub API call uses `curl -s` without `-f`. When rate limits are hit, it exits with 0 and returns an empty list, which silent-fails and breaks test assertions.
- **Fix**:
  1. Add `-f` (so `curl -fs`) to ensure a non-zero exit code on HTTP errors.
  2. In the frontmatter, check if `treeData.tree` exists and is an array. If not, explicitly throw an Error so that it falls back to the local mock/cached data.

## 3. Build Cache and Chunk Collision Errors
- **Issue**: Repetitive/sequential builds during test execution fail with `ERR_MODULE_NOT_FOUND` because Astro/Vite cache old chunk hashes but the build timestamp in `SiteLayout.astro` makes builds non-deterministic.
- **Fix**: Update the build script clean phase in `package.json` to also delete `.astro` and `node_modules/.vite` directories before building. For example:
  ```json
  "build": "rm -rf docs/.prerender docs/_astro docs/applications docs/tests docs/upstream docs/bluefin docs/adoption docs/homebrew docs/about .astro node_modules/.vite && astro build"
  ```

## 4. File Deletion and Unused Selector Cleanups
- **Issue**: `docs/prototype-factory.html` was not deleted. Unused selector `.status-grid` still remains at line 783 in `src/styles/site.css`.
- **Fix**:
  - Delete `docs/prototype-factory.html`.
  - Remove `.status-grid` from `src/styles/site.css` line 783.
