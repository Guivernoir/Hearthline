import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [sveltekit({
    adapter: adapter({ fallback: "index.html" }),
    preprocess: vitePreprocess(),
  })],
  server: {
    proxy: {
      "/api": "http://127.0.0.1:3001",
    },
  },
});
