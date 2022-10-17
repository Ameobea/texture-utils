import { sveltekit } from '@sveltejs/kit/vite';
import type { UserConfig } from 'vite';
import { resolve } from 'path';

/** @type {import('vite').Plugin} */
const viteServerConfig = {
  name: 'log-request-middleware',
  configureServer(server) {
    server.middlewares.use((req, res, next) => {
      res.setHeader('Access-Control-Allow-Origin', '*');
      res.setHeader('Access-Control-Allow-Methods', 'GET');
      res.setHeader('Cross-Origin-Opener-Policy', 'same-origin');
      res.setHeader('Cross-Origin-Embedder-Policy', 'require-corp');
      next();
    });
  },
};

const config: UserConfig = {
  plugins: [sveltekit(), viteServerConfig],
  resolve: {
    alias: {
      src: resolve('./src'),
    },
  },
  build: {
    sourcemap: true,
  },
  worker: {
    rollupOptions: {
      output: {
        sourcemap: true,
      },
    },
    format: 'es',
  },
};

export default config;
