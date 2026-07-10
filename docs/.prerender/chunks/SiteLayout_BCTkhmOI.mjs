import { C as createComponent, S as createAstro, _ as addAttribute, a as Fragment, c as renderSlot, d as renderTemplate, g as renderHead, i as renderComponent } from "./server_Dx5UOJVp.mjs";
//#region \0rolldown/runtime.js
var __defProp = Object.defineProperty;
var __exportAll = (all, no_symbols) => {
	let target = {};
	for (var name in all) __defProp(target, name, {
		get: all[name],
		enumerable: true
	});
	if (!no_symbols) __defProp(target, Symbol.toStringTag, { value: "Module" });
	return target;
};
//#endregion
//#region src/layouts/SiteLayout.astro
createAstro("https://factory.projectbluefin.io");
var $$SiteLayout = createComponent(($$result, $$props, $$slots) => {
	const Astro2 = $$result.createAstro($$props, $$slots);
	Astro2.self = $$SiteLayout;
	const { title, description, includeDashboardAssets = false } = Astro2.props;
	const baseUrl = "/";
	const navItems = [
		{
			id: "overview",
			label: "Overview",
			href: `${baseUrl}`
		},
		{
			id: "images",
			label: "Images",
			href: `${baseUrl}images/`
		},
		{
			id: "builds",
			label: "Builds",
			href: `${baseUrl}builds/`
		},
		{
			id: "tests",
			label: "Tests",
			href: `${baseUrl}tests/`
		},
		{
			id: "applications",
			label: "Applications",
			href: `${baseUrl}applications/`
		},
		{
			id: "adoption",
			label: "Adoption",
			href: `${baseUrl}adoption/`
		},
		{
			id: "userspace",
			label: "Userspace",
			href: `${baseUrl}userspace/`
		},
		{
			id: "about",
			label: "About",
			href: `${baseUrl}about/`
		}
	];
	const normalizedPath = Astro2.url.pathname.replace(/\/index\.html$/, "/");
	const currentYear = (/* @__PURE__ */ new Date()).getFullYear();
	const buildTimestamp = (/* @__PURE__ */ new Date()).toISOString();
	const isHomepage = normalizedPath === baseUrl || normalizedPath === "/" || normalizedPath === "/index.html";
	return renderTemplate`<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta name="description"${addAttribute(description, "content")}><meta name="color-scheme" content="dark"><title>${title} · Project Bluefin Operating System Factory</title><link rel="preconnect" href="https://fonts.googleapis.com"><link rel="preconnect" href="https://fonts.gstatic.com" crossorigin><link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap" rel="stylesheet"><link rel="icon" type="image/svg+xml"${addAttribute(`${baseUrl}favicon.svg`, "href")}><meta property="og:title"${addAttribute(title, "content")}><meta property="og:description"${addAttribute(description, "content")}><meta property="og:type" content="website"><meta property="og:url"${addAttribute(Astro2.url.href, "content")}><meta name="twitter:card" content="summary_large_image"><meta name="twitter:title"${addAttribute(title, "content")}><meta name="twitter:description"${addAttribute(description, "content")}>${includeDashboardAssets && renderTemplate`${renderComponent($$result, "Fragment", Fragment, {}, { "default": ($$result2) => renderTemplate`<link rel="stylesheet"${addAttribute(`${baseUrl}assets/factory-dashboard.css`, "href")}><script${addAttribute(`${baseUrl}assets/factory-dashboard.js`, "src")} defer data-cfasync="false"><\/script>` })}`}${renderHead($$result)}</head><body><a href="#main-content" class="skip-link">Skip to content</a><header class="site-header"><div class="site-header__inner"><a class="site-brand"${addAttribute(baseUrl, "href")}><span class="site-brand__eyebrow">Project Bluefin</span>${isHomepage ? renderTemplate`<h1 class="site-brand__title">Operating System Factory</h1>` : renderTemplate`<span class="site-brand__title">Operating System Factory</span>`}</a><nav class="site-nav" aria-label="Factory sections">${navItems.map((item) => {
		const isActive = item.href === baseUrl ? normalizedPath === baseUrl || normalizedPath === `${baseUrl}index.html` : normalizedPath === item.href || normalizedPath === item.href.slice(0, -1) || normalizedPath.startsWith(item.href);
		return renderTemplate`<a${addAttribute(["site-nav__link", isActive && "is-active"], "class:list")}${addAttribute(item.href, "href")}${addAttribute(isActive ? "page" : void 0, "aria-current")}>${item.label}</a>`;
	})}</nav></div></header><main id="main-content" tabindex="-1" class="site-shell">${renderSlot($$result, $$slots["default"])}</main><footer class="site-footer"><div class="site-footer__inner"><p>&copy; ${currentYear} Project Bluefin. All rights reserved.</p><p><a href="https://github.com/projectbluefin/lab" target="_blank" rel="noopener noreferrer">GitHub Repository</a></p><p class="build-timestamp">Built: ${buildTimestamp}</p></div></footer></body></html>`;
}, "/var/home/jorge/src/lab/src/layouts/SiteLayout.astro", void 0);
//#endregion
export { __exportAll as n, $$SiteLayout as t };
