import { defineConfig } from 'astro/config';

export default defineConfig({
  output: 'static',
  outDir: './docs',
  site: 'https://qa.projectbluefin.io',
  trailingSlash: 'always',
  vite: {
    build: {
      emptyOutDir: false,
    },
  },
});
