/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_GRAPH_JSON_URL?: string;
  readonly VITE_GITHUB_URL?: string;
  readonly VITE_GITHUB_PATH_PREFIX?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

