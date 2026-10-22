// A virtual HTTPS origin served entirely from packaged rawfiles. No listener or remote server.
export const LOCAL_ORIGIN = 'https://hatoba.invalid';
export const LOCAL_ENTRY = `${LOCAL_ORIGIN}/harmonyos-probe.html`;

export interface LocalAsset {
  path: string;
  mime: string;
}

export function isLocalNavigation(url: string): boolean {
  return url === LOCAL_ENTRY || url.startsWith(`${LOCAL_ENTRY}#`);
}

export function resolveLocalAsset(url: string, method: string): LocalAsset | undefined {
  if (method !== 'GET' || !url.startsWith(`${LOCAL_ORIGIN}/`)) return undefined;
  const path = url.slice(LOCAL_ORIGIN.length + 1);
  // Reject encoding, queries, traversal and alternate separators before touching the resource manager.
  if (!/^[A-Za-z0-9_./-]+$/.test(path) || path.split('/').some((part: string) => !part || part === '.' || part === '..')) return undefined;
  if (path !== 'harmonyos-probe.html' && !path.startsWith('assets/')) return undefined;
  const extension = path.slice(path.lastIndexOf('.') + 1);
  let mime: string;
  switch (extension) {
    case 'html': mime = 'text/html'; break;
    case 'js': case 'mjs': mime = 'text/javascript'; break;
    case 'css': mime = 'text/css'; break;
    case 'json': mime = 'application/json'; break;
    case 'svg': mime = 'image/svg+xml'; break;
    case 'png': mime = 'image/png'; break;
    case 'jpg': case 'jpeg': mime = 'image/jpeg'; break;
    case 'webp': mime = 'image/webp'; break;
    case 'ico': mime = 'image/x-icon'; break;
    case 'woff': mime = 'font/woff'; break;
    case 'woff2': mime = 'font/woff2'; break;
    case 'ttf': mime = 'font/ttf'; break;
    case 'wasm': mime = 'application/wasm'; break;
    default: return undefined;
  }
  return { path: `web/${path}`, mime: mime };
}
