import { defineConfig } from 'astro/config';

export default defineConfig({
  output: 'static',
  outDir: './docs',
  site: 'https://projectbluefin.github.io',
  base: '/testing-lab/',
  trailingSlash: 'always',
  vite: {
    build: {
      emptyOutDir: false,
    },
  },
});
