import { sveltekit } from '@sveltejs/kit/vite';
import type { UserConfig } from 'vite';
import { resolve } from 'path';

const config: UserConfig = {
  plugins: [sveltekit()],
  resolve: {
    alias: {
      src: resolve('./src'),
    },
  },
};

export default config;
