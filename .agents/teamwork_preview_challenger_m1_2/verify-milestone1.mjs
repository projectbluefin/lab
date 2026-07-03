import { readFileSync, existsSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const repoRoot = '/var/home/jorge/src/testing-lab';
const docsDir = join(repoRoot, 'docs');

const failures = [];

function recordFailure(category, details) {
  failures.push({ category, details });
  console.error(`❌ [${category}] ${details}`);
}

// 1. Navigation Active State Highlighting logic test
function testNavigationLogic() {
  console.log('Testing Navigation Active State Logic...');

  function isActive(itemHref, baseUrl, pathname) {
    const normalizedPath = pathname.replace(/\/index\.html$/, '/');
    return itemHref === baseUrl
      ? normalizedPath === baseUrl || normalizedPath === `${baseUrl}index.html`
      : normalizedPath === itemHref || normalizedPath === itemHref.slice(0, -1) || normalizedPath.startsWith(itemHref);
  }

  const cases = [
    ['/', '/', '/', true],
    ['/', '/', '/index.html', true],
    ['/', '/', '/bluefin', false],
    ['/bluefin/', '/', '/bluefin', true],
    ['/bluefin/', '/', '/bluefin/', true],
    ['/bluefin/', '/', '/bluefin/index.html', true],
    ['/about/', '/', '/about', true],
    ['/about/', '/', '/about/', true],
    ['/about/', '/', '/about/index.html', true],
    ['/bluefin/', '/', '/bluefin/subpage', true],
    ['/bluefin/', '/', '/bluefin-something', false],
    
    ['/subpath/', '/subpath/', '/subpath/', true],
    ['/subpath/', '/subpath/', '/subpath/index.html', true],
    ['/subpath/bluefin/', '/subpath/', '/subpath/bluefin', true],
    ['/subpath/bluefin/', '/subpath/', '/subpath/bluefin/', true],
    ['/subpath/bluefin/', '/subpath/', '/subpath/bluefin/subpage', true],
    ['/subpath/bluefin/', '/subpath/', '/subpath/bluefin-something', false],
  ];

  let passed = true;
  for (const [itemHref, baseUrl, pathname, expected] of cases) {
    const result = isActive(itemHref, baseUrl, pathname);
    if (result !== expected) {
      recordFailure('Navigation Logic', `itemHref='${itemHref}', baseUrl='${baseUrl}', pathname='${pathname}'. Expected: ${expected}, Got: ${result}`);
      passed = false;
    }
  }
  if (passed) console.log('✓ Navigation Active State Logic assertions passed!');
}

function parseAttributes(tagStr) {
  const attrs = {};
  const regex = /([\w-]+)(?:=(?:"([^"]*)"|'([^']*)'|(\S+)))?/g;
  let match;
  const cleanTagStr = tagStr.replace(/^<\w+\s+/, '').replace(/\/?>$/, '');
  while ((match = regex.exec(cleanTagStr)) !== null) {
    const name = match[1].toLowerCase();
    const value = match[2] ?? match[3] ?? match[4] ?? true;
    attrs[name] = value;
  }
  return attrs;
}

function findNavLinks(htmlContent) {
  const navMatch = htmlContent.match(/<nav\b[^>]*>([\s\S]*?)<\/nav>/i);
  if (!navMatch) return [];

  const navContent = navMatch[1];
  const linkRegex = /<a\b[^>]*>([\s\S]*?)<\/a>/gi;
  let match;
  const links = [];
  while ((match = linkRegex.exec(navContent)) !== null) {
    const fullTag = match[0];
    const text = match[1].trim();
    const openingTag = fullTag.match(/<a\b[^>]*>/i)[0];
    const attrs = parseAttributes(openingTag);
    links.push({ text, attrs, fullTag });
  }
  return links;
}

// 2. Parse built HTML files for specific checks
function verifyCompiledPages() {
  console.log('Verifying Compiled Pages in docs/ ...');

  const compiledFiles = [
    'docs/index.html',
    'docs/upstream/index.html',
    'docs/bluefin/index.html',
    'docs/tests/index.html',
    'docs/applications/index.html',
    'docs/homebrew/index.html',
    'docs/adoption/index.html',
    'docs/userspace/index.html',
    'docs/about/index.html',
  ];

  const routeToNavLinkText = {
    '/index.html': 'Overview',
    '/about/index.html': 'About',
    '/bluefin/index.html': 'Bluefin',
    '/upstream/index.html': 'Upstream',
    '/tests/index.html': 'Tests',
    '/applications/index.html': 'Applications',
    '/homebrew/index.html': 'Homebrew',
    '/adoption/index.html': 'Adoption',
    '/userspace/index.html': 'Userspace',
  };

  for (const relativePath of compiledFiles) {
    const file = join(repoRoot, relativePath);
    const relativeDocPath = relativePath.replace('docs', '');
    
    if (!existsSync(file)) {
      recordFailure('Build Output', `Expected compiled page is missing: ${relativePath}`);
      continue;
    }

    const content = readFileSync(file, 'utf8');
    console.log(`Analyzing: ${relativeDocPath}`);

    // --- Heading Outline check (exactly one <h1> tag) ---
    const h1Matches = content.match(/<h1\b[^>]*>/gi) || [];
    if (h1Matches.length !== 1) {
      recordFailure('Heading Outline', `${relativeDocPath} has ${h1Matches.length} <h1> tags (expected exactly 1).`);
    }

    // --- Keyboard Accessibility check (Skip link) ---
    const hasSkipLink = /<a\s+[^>]*href=["']#main-content["'][^>]*class=["']skip-link["'][^>]*>Skip to content<\/a>/i.test(content);
    if (!hasSkipLink) {
      recordFailure('Keyboard Accessibility', `${relativeDocPath} is missing skip link syntax targeting #main-content.`);
    } else {
      const bodyIndex = content.indexOf('<body');
      const skipLinkIndex = content.search(/class=["']skip-link["']/);
      const mainIndex = content.indexOf('id="main-content"');
      
      if (skipLinkIndex <= bodyIndex) {
        recordFailure('Keyboard Accessibility', `Skip link is not inside <body> in ${relativeDocPath}`);
      }
      if (skipLinkIndex >= mainIndex) {
        recordFailure('Keyboard Accessibility', `Skip link is placed after main-content in ${relativeDocPath}`);
      }
    }

    const hasMainContent = /<main\s+[^>]*id=["']main-content["'][^>]*tabindex=["']-1["']/i.test(content);
    if (!hasMainContent) {
      recordFailure('Keyboard Accessibility', `${relativeDocPath} is missing <main id="main-content" tabindex="-1"> wrapper.`);
    }

    // --- SEO Metadata check (Open Graph and Twitter) ---
    const expectedMeta = [
      { name: 'og:title', pattern: /<meta\s+[^>]*property=["']og:title["']/i },
      { name: 'og:description', pattern: /<meta\s+[^>]*property=["']og:description["']/i },
      { name: 'og:type', pattern: /<meta\s+[^>]*property=["']og:type["']\s+[^>]*content=["']website["']/i },
      { name: 'og:url', pattern: /<meta\s+[^>]*property=["']og:url["']/i },
      { name: 'twitter:card', pattern: /<meta\s+[^>]*name=["']twitter:card["']\s+[^>]*content=["']summary_large_image["']/i },
      { name: 'twitter:title', pattern: /<meta\s+[^>]*name=["']twitter:title["']/i },
      { name: 'twitter:description', pattern: /<meta\s+[^>]*name=["']twitter:description["']/i },
    ];

    for (const meta of expectedMeta) {
      if (!meta.pattern.test(content)) {
        recordFailure('SEO Metadata', `${relativeDocPath} is missing required meta tag: ${meta.name}`);
      }
    }

    // --- Navigation Active Highlight check on statically built HTML files ---
    const activeText = routeToNavLinkText[relativeDocPath];
    if (activeText) {
      const navLinks = findNavLinks(content);
      if (navLinks.length === 0) {
        recordFailure('Navigation Highlighting', `No navigation links found in ${relativeDocPath}`);
        continue;
      }

      let foundActive = false;
      for (const link of navLinks) {
        const isCurrentRouteLink = link.text === activeText;
        const classes = link.attrs.class ? link.attrs.class.split(/\s+/) : [];
        const isActiveClass = classes.includes('is-active');
        const hasAriaCurrent = link.attrs['aria-current'] === 'page';

        if (isCurrentRouteLink) {
          if (!isActiveClass) {
            recordFailure('Navigation Highlighting', `Link for '${activeText}' in ${relativeDocPath} is missing 'is-active' class.`);
          }
          if (!hasAriaCurrent) {
            recordFailure('Navigation Highlighting', `Link for '${activeText}' in ${relativeDocPath} is missing 'aria-current="page"'.`);
          }
          foundActive = true;
        } else {
          if (isActiveClass) {
            recordFailure('Navigation Highlighting', `Link for '${link.text}' in ${relativeDocPath} should NOT have 'is-active' class.`);
          }
          if (hasAriaCurrent) {
            recordFailure('Navigation Highlighting', `Link for '${link.text}' in ${relativeDocPath} should NOT have 'aria-current="page"'.`);
          }
        }
      }
      if (!foundActive) {
        recordFailure('Navigation Highlighting', `Active navigation link for text '${activeText}' not found in ${relativeDocPath}`);
      }
    }
  }

  console.log('✓ Compiled pages verification completed.');
}

// Helper to recursively get files matching a pattern
function getFilesRecursive(dir, ext) {
  let results = [];
  try {
    const list = readdirSync(dir);
    for (const file of list) {
      const filePath = join(dir, file);
      const stat = statSync(filePath);
      if (stat && stat.isDirectory()) {
        results = results.concat(getFilesRecursive(filePath, ext));
      } else if (file.endsWith(ext)) {
        results.push(filePath);
      }
    }
  } catch (e) {
    // Ignore missing directories
    console.error(`getFilesRecursive error: ${e.message}`);
  }
  return results;
}

// 3. CSS file verification
function verifyCSS() {
  console.log('Verifying Compiled CSS Styles...');
  const cssFiles = getFilesRecursive(docsDir, '.css');
  if (cssFiles.length === 0) {
    recordFailure('Compiled CSS', 'No CSS files found in docs/.');
    return;
  }

  console.log(`Found ${cssFiles.length} CSS files to inspect.`);

  let hasStatusVars = false;
  let hasFailedPillColor = false;
  let hasFailedValue = false;

  for (const file of cssFiles) {
    const content = readFileSync(file, 'utf8');

    if (content.includes('--status-passed') && 
        content.includes('--status-failed') && 
        content.includes('--status-pending') && 
        content.includes('--status-unavailable')) {
      hasStatusVars = true;
    }

    if (content.includes('--status-failed:') && content.match(/--status-failed:\s*#fb7185/i)) {
      hasFailedValue = true;
    }

    if (content.includes('.pill--failed') && 
        (content.includes('var(--status-failed)') || content.includes('#fb7185'))) {
      hasFailedPillColor = true;
    }
  }

  if (!hasStatusVars) {
    recordFailure('Compiled CSS', 'Semantic status variables (--status-passed/failed/pending/unavailable) are missing from :root in all CSS files.');
  }
  if (!hasFailedValue) {
    recordFailure('Compiled CSS', '--status-failed does not resolve to color #fb7185.');
  }
  if (!hasFailedPillColor) {
    recordFailure('Compiled CSS', '.pill--failed does not resolve to color #fb7185 or var(--status-failed).');
  }

  if (hasStatusVars && hasFailedValue && hasFailedPillColor) {
    console.log('✓ CSS status variables and failing color assertions passed!');
  }
}

// 4. File Cleanup verification
function verifyFileCleanup() {
  console.log('Verifying dead prototype file deletion...');
  
  const deadFiles = [
    join(repoRoot, 'src/components/UnavailablePanel.astro'),
    join(repoRoot, 'docs/prototype-factory.html'),
    join(repoRoot, 'flatcar-clone-prototype.py'),
  ];

  let cleanupPassed = true;
  for (const file of deadFiles) {
    const filename = file.replace(repoRoot, '');
    if (existsSync(file)) {
      recordFailure('File Cleanup', `dead file ${filename} still exists in the repository!`);
      cleanupPassed = false;
    } else {
      console.log(`✓ Confirmed deleted: ${filename}`);
    }
  }

  if (cleanupPassed) {
    console.log('✓ Prototype file cleanup verification passed!');
  }
}

try {
  testNavigationLogic();
  verifyCompiledPages();
  verifyCSS();
  verifyFileCleanup();
  
  console.log('\n=========================================');
  if (failures.length === 0) {
    console.log('ALL MILESTONE 1 VERIFICATIONS PASSED!');
  } else {
    console.log(`VERIFICATION COMPLETED WITH ${failures.length} FAILURES!`);
  }
  console.log('=========================================');
} catch (error) {
  console.error('\n❌ VERIFICATION SCRIPT CRASHED!');
  console.error(error.stack || error.message);
  process.exit(1);
}
