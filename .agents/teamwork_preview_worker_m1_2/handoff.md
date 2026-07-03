# Handoff Report

## 1. Observation
- **Issue 1: Homepage Heading Outline Violation**:
  - `src/layouts/SiteLayout.astro` previously rendered the site brand title inside a simple `<span>` tag for all pages.
  - Verbatim line 67: `<span class="site-brand__title">Operating System Factory</span>`
- **Issue 2: GitHub API Rate Limit Vulnerability**:
  - `src/pages/applications.astro` verbatim line 136: `const treeRes = execSync('curl -s -H "User-Agent: lab-builder" --max-time 3 https://api.github.com/repos/projectbluefin/testsuite/git/trees/main?recursive=1', { encoding: 'utf8' });`
  - When rate limited, curl exited with status code `0`, returning an empty body, resulting in JSON parse failure or an empty tree list without hitting the catch block cleanly.
- **Issue 3: Build Cache and Chunk Collision Errors**:
  - Sequential builds generated random chunk names based on timestamp. This caused module resolution errors if Astro/Vite cache files were kept.
  - Verbatim error log observed in first run:
    ```
    17:45:12 [ERROR] Error [ERR_MODULE_NOT_FOUND]: Cannot find module '/var/home/jorge/src/testing-lab/docs/.prerender/chunks/index_DOdJ-95x.mjs' imported from /var/home/jorge/src/testing-lab/docs/.prerender/prerender-entry.CrMSVjtS.mjs
    ```
  - `package.json` `"build"` script previously cleaned only output `docs/*` directories, but did not clean `.astro` and `node_modules/.vite`.
- **Issue 4: Unused Selector and Files**:
  - `docs/prototype-factory.html` was present in the Git index as untracked/deleted change.
  - `src/styles/site.css` line 783 had `.status-grid,` selector inside `@media (max-width: 900px)`.

## 2. Logic Chain
- **For Issue 1**:
  - Added a check `const isHomepage = normalizedPath === baseUrl || normalizedPath === '/' || normalizedPath === '/index.html';` in `SiteLayout.astro`.
  - Conditioned tag rendering based on `isHomepage`:
    - `true` -> render as `<h1>`
    - `false` -> render as `<span>`
  - This ensures that only the homepage has the brand title in an `<h1>`, while subpages use `<span>` for the brand title, preserving exactly one `<h1>` per page.
- **For Issue 2**:
  - Changed `curl -s` to `curl -fs` in the shell execution. The `-f` flag tells curl to exit with a non-zero status on HTTP errors.
  - Added the check `if (!treeData || !treeData.tree || !Array.isArray(treeData.tree))` inside the `try` block, and explicitly threw an error.
  - This ensures any invalid payload or failure correctly transfers execution to the `catch` block and uses local mock data fallback.
- **For Issue 3**:
  - Updated the `"build"` script inside `package.json` to delete `.astro` and `node_modules/.vite`.
  - Verified that manual and sequential runs of `npm run build` now consistently succeed.
- **For Issue 4**:
  - Deleted `docs/prototype-factory.html` file.
  - Removed `.status-grid,` from `src/styles/site.css` responsive media query block.

## 3. Caveats
- No caveats. The issues were well-defined and solved using standard Astro/CSS/shell practices.

## 4. Conclusion
- All requested tasks have been successfully completed:
  1. Header dynamically renders `<h1 class="site-brand__title">` on the homepage and `<span class="site-brand__title">` on subpages.
  2. GitHub API call uses `curl -fs` and throws on missing/non-array `treeData.tree` properties to ensure clean fallback triggers.
  3. `package.json` build cleanup deletes `.astro` and `node_modules/.vite` build cache directories.
  4. `docs/prototype-factory.html` is deleted.
  5. Unused `.status-grid` selector is removed from `src/styles/site.css`.

## 5. Verification Method
- Execute the following commands in the workspace root directory:
  - `npm run check`: To verify there are no compilation errors or type warnings.
  - `npm run build`: To verify that clean builds succeed sequentially without chunk errors.
  - `npm test`: To execute the test suite (all 13 tests must pass green).
